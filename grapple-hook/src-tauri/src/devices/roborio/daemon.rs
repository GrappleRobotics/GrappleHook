use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use bounded_static::ToBoundedStatic;
use futures_util::{SinkExt, StreamExt};
use grapple_frc_msgs::{
    binmarshal::{BitView, Demarshal, LengthTaggedPayload, LengthTaggedPayloadOwned},
    bridge::BridgedCANMessage,
    grapple::{fragments::FragmentReassembler, TaggedGrappleMessage},
    ManufacturerMessage, MessageId,
};
use grapple_hook_macros::rpc;
use log::{info, warn};
use rust_embed::RustEmbed;
use tokio::{
    net::TcpStream,
    sync::{mpsc, Mutex},
};
use tokio_util::codec::Framed;

use crate::{
    canlog::{CanLog, CanLogRequest, CanLogResponse},
    rpc::RpcBase,
};

use crate::{
    codecs::tcp_can_bridge::GrappleTcpCanBridgeCodec,
    devices::{
        device_manager::{DeviceManager, DeviceManagerRequest, DeviceManagerResponse},
        provider::{DeviceProvider, ProviderInfo},
    },
    ssh::SSHSession,
};

const ROBORIO_ADDRESS: &'static str = "172.22.11.2";

#[derive(RustEmbed)]
#[folder = "../../GrappleHook-RoboRIO-Daemon/build/exe/grappleHookRoboRioDaemon/release/"]
struct Daemon;

pub struct RoboRioDaemonInner {
    running: AtomicBool,
    device_manager: DeviceManager,

    stop_signal_tx: mpsc::Sender<()>,
    stop_signal_rx: Mutex<mpsc::Receiver<()>>,
    can_send_rx: Mutex<mpsc::Receiver<TaggedGrappleMessage<'static>>>,
    can_send_raw_rx: Mutex<mpsc::Receiver<(MessageId, Vec<u8>)>>,

    do_deploy: AtomicBool,
    address: Mutex<String>,

    canlog: CanLog,
}

pub struct RoboRioDaemon {
    inner: Arc<RoboRioDaemonInner>,
}

impl RoboRioDaemon {
    pub fn new() -> Self {
        let (can_send_tx, can_send_rx) = mpsc::channel(100);
        let (can_send_raw_tx, can_send_raw_rx) = mpsc::channel(100);
        let (stop_signal_tx, stop_signal_rx) = mpsc::channel(5);

        let mut sends = HashMap::new();
        sends.insert("CAN".to_owned(), can_send_tx);

        Self {
            inner: Arc::new(RoboRioDaemonInner {
                running: AtomicBool::new(false),
                device_manager: DeviceManager::new(sends),
                stop_signal_tx,
                stop_signal_rx: Mutex::new(stop_signal_rx),
                can_send_rx: Mutex::new(can_send_rx),
                can_send_raw_rx: Mutex::new(can_send_raw_rx),
                do_deploy: AtomicBool::new(true),
                address: Mutex::new(ROBORIO_ADDRESS.to_owned()),
                canlog: CanLog::new(512, can_send_raw_tx),
            }),
        }
    }

    async fn do_loop(
        mut framed: Framed<TcpStream, GrappleTcpCanBridgeCodec>,
        inner: Arc<RoboRioDaemonInner>,
    ) -> anyhow::Result<()> {
        let mut can_send_rx = inner
            .can_send_rx
            .try_lock()
            .map_err(|_| anyhow::anyhow!("This RootDevice is already running!"))?;
        let mut can_send_raw_rx = inner
            .can_send_raw_rx
            .try_lock()
            .map_err(|_| anyhow::anyhow!("This RootDevice is already running!"))?;
        let mut stop_signal_rx = inner.stop_signal_rx.try_lock()?;

        let (mut reassemble_rx, mut reassemble_tx) = FragmentReassembler::new(1000, 8).split();
        let mut device_manager_interval = tokio::time::interval(Duration::from_millis(500));

        loop {
            tokio::select! {
              msg = framed.next() => match msg {
                Some(Ok(msg)) => {
                  // let id2 = Into::<grapple_frc_msgs::grapple::GrappleMessageId>::into(msg.id);
                  let mut already_logged = false;
                  let manufacturer_msg = ManufacturerMessage::read(&mut BitView::new(&msg.data.0[..]), msg.id);
                  match manufacturer_msg {
                    Ok(ManufacturerMessage::Grapple(grpl_msg)) => {
                      let mut storage = Vec::new();
                      if let Ok(Some((gid, grpl_unfragmented))) = reassemble_rx.defragment(msg.timestamp as i64, &msg.id, grpl_msg, &mut storage) {
                        inner.canlog.on_message(&msg, Some(&grpl_unfragmented)).await;
                        already_logged = true;

                        inner.device_manager.on_message("CAN".to_owned(), gid, TaggedGrappleMessage::new(msg.id.device_id, grpl_unfragmented.to_static())).await?;
                      }
                    },
                    _ => ()
                  }

                  if !already_logged {
                    inner.canlog.on_message(&msg, None).await;
                  }
                },
                Some(Err(e)) => anyhow::bail!(e),
                None => ()
              },
              msg = can_send_rx.recv() => match msg {
                Some(msg) => {
                  // Need to send something on the CAN bus
                  let TaggedGrappleMessage { device_id, msg } = msg;

                  let mut msgs = vec![];
                  reassemble_tx.maybe_fragment(device_id, msg.clone(), &mut |id, buf| {
                    msgs.push(BridgedCANMessage { id, timestamp: 0, data: Cow::<LengthTaggedPayload<u8>>::Owned(LengthTaggedPayloadOwned::new(buf.to_vec())).into() });
                  }).ok();

                  let len = msgs.len();
                  for (i, cur_msg) in msgs.into_iter().enumerate() {
                    inner.canlog.on_message(&cur_msg, (i == len - 1).then(|| &msg)).await;

                    framed.send(cur_msg).await?;
                  }
                },
                None => ()
              },
              msg = can_send_raw_rx.recv() => match msg {
                  Some((id, data)) => {
                    let msg = BridgedCANMessage { id, timestamp: 0, data: Cow::<LengthTaggedPayload<u8>>::Owned(LengthTaggedPayloadOwned::new(data)).into() };
                    inner.canlog.on_message(&msg, None).await;
                    framed.send(msg).await?;
                  },
                  None => ()
              },
              sig = stop_signal_rx.recv() => match sig {
                Some(()) => {
                  break;
                },
                None => ()
              },
              _ = device_manager_interval.tick() => {
                inner.device_manager.on_tick().await?;
              }
            }
        }

        Ok(())
    }

    async fn deploy(addr: String) -> anyhow::Result<()> {
        info!("Deploy...");
        let session = SSHSession::connect(&(addr + ":22"), "admin", "").await?;

        let file = Daemon::get("grappleHookRoboRioDaemon")
            .ok_or(anyhow::anyhow!("Embedded File Error"))?;
        session
            .copy(file.data.to_vec(), "/tmp/grapple-hook-daemon")
            .await?;

        tokio::spawn(async move {
            session.run("frcKillRobot.sh -t; killall grapple-hook-daemon; frcKillRobot.sh -t; /tmp/grapple-hook-daemon > /tmp/grapple-hook-daemon.log 2>&1").await.ok();
        });

        info!("Deploy Successful!");

        Ok(())
    }

    pub async fn revert_to_robot_code(addr: String) -> anyhow::Result<()> {
        info!("Reverting to user code...");
        let session = SSHSession::connect(&(addr + ":22"), "admin", "").await?;
        session
            .run("killall grapple-hook-daemon; frcKillRobot.sh -t -r")
            .await
            .ok();
        info!("Reverted to user code!");
        Ok(())
    }

    async fn do_start(inner: Arc<RoboRioDaemonInner>) -> anyhow::Result<()> {
        info!("Connecting...");

        let will_deploy = inner.do_deploy.load(std::sync::atomic::Ordering::Relaxed);
        let addr = inner.address.lock().await.clone();

        if will_deploy {
            Self::deploy(addr.clone()).await?;
        }

        let stream = tokio::time::timeout(
            Duration::from_millis(3000),
            TcpStream::connect(ROBORIO_ADDRESS.to_owned() + ":8006"),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Connection Timed Out!"))??;
        let framed = Framed::new(stream, GrappleTcpCanBridgeCodec);

        info!("Connected!");

        tokio::task::spawn(async move {
            inner
                .running
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let r = Self::do_loop(framed, inner.clone()).await;
            inner
                .running
                .store(false, std::sync::atomic::Ordering::Relaxed);
            if will_deploy {
                tokio::time::timeout(
                    tokio::time::Duration::from_secs(10),
                    Self::revert_to_robot_code(addr.clone()),
                )
                .await
                .ok();
            }
            inner.device_manager.reset().await;
            match r {
                Ok(_) => info!("RoboRioDaemon runner stopped gracefully"),
                Err(e) => warn!("RoboRioDaemon runner stopped with error: {}", e),
            }
        });

        Ok(())
    }
}

#[async_trait::async_trait]
impl DeviceProvider for RoboRioDaemon {
    async fn connect(&self) -> anyhow::Result<()> {
        Self::do_start(self.inner.clone()).await
    }

    async fn disconnect(&self) -> anyhow::Result<()> {
        self.inner.stop_signal_tx.send(()).await.ok();
        Ok(())
    }

    async fn info(&self) -> anyhow::Result<ProviderInfo> {
        Ok(ProviderInfo {
            ty: "RoboRIO".to_owned(),
            description: format!("RoboRIO"),
            address: self.inner.address.lock().await.clone(),
            connected: self
                .inner
                .running
                .load(std::sync::atomic::Ordering::Relaxed),
        })
    }

    async fn device_manager_call(
        &self,
        req: DeviceManagerRequest,
    ) -> anyhow::Result<DeviceManagerResponse> {
        self.inner.device_manager.rpc_process(req).await
    }

    async fn call(&self, req: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        self.rpc_call(req).await
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RoboRIOStatus {
    pub using_daemon: bool,
}

#[rpc]
impl RoboRioDaemon {
    async fn status(&self) -> anyhow::Result<RoboRIOStatus> {
        Ok(RoboRIOStatus {
            using_daemon: self
                .inner
                .do_deploy
                .load(std::sync::atomic::Ordering::Relaxed),
        })
    }

    async fn set_use_daemon(&self, use_daemon: bool) -> anyhow::Result<()> {
        self.inner
            .do_deploy
            .store(use_daemon, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn set_address(&self, address: String) -> anyhow::Result<()> {
        let mut addr = self.inner.address.lock().await;
        *addr = address;
        Ok(())
    }

    async fn canlog_call(&self, req: CanLogRequest) -> anyhow::Result<CanLogResponse> {
        self.inner.canlog.rpc_process(req).await
    }
}
