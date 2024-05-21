use std::{sync::{atomic::AtomicBool, Arc}, time::Duration, collections::HashMap, borrow::Cow};

use bounded_static::ToBoundedStatic;
use grapple_frc_msgs::{binmarshal::{BitView, Demarshal, LengthTaggedPayload, LengthTaggedPayloadOwned}, grapple::{fragments::FragmentReassembler, TaggedGrappleMessage}, ManufacturerMessage, bridge::BridgedCANMessage};
use futures_util::{SinkExt, StreamExt};
use log::{info, warn};
use rust_embed::RustEmbed;
use tokio::{sync::{mpsc, Mutex}, net::TcpStream};
use tokio_util::codec::Framed;

use crate::{devices::{device_manager::{DeviceManager, DeviceManagerRequest, DeviceManagerResponse}, provider::{DeviceProvider, ProviderInfo}}, codecs::tcp_can_bridge::GrappleTcpCanBridgeCodec, ssh::SSHSession};

const ROBORIO_ADDRESS: &'static str = "172.22.11.2";

#[derive(RustEmbed)]
#[folder="../../GrappleHook-RoboRIO-Daemon/build/exe/grappleHookRoboRioDaemon/release/"]
struct Daemon;

pub struct RoboRioDaemonInner {
  running: AtomicBool,
  device_manager: DeviceManager,

  stop_signal_tx: mpsc::Sender<()>,
  stop_signal_rx: Mutex<mpsc::Receiver<()>>,
  can_send_rx: Mutex<mpsc::Receiver<TaggedGrappleMessage<'static>>>,
}

pub struct RoboRioDaemon {
  inner: Arc<RoboRioDaemonInner>
}

impl RoboRioDaemon {
  pub fn new() -> Self {
    let (can_send_tx, can_send_rx) = mpsc::channel(100);
    let (stop_signal_tx, stop_signal_rx) = mpsc::channel(5);

    let mut sends = HashMap::new();
    sends.insert("CAN".to_owned(), can_send_tx);

    Self {
      inner: Arc::new(
        RoboRioDaemonInner {
          running: AtomicBool::new(false),
          device_manager: DeviceManager::new(sends),
          stop_signal_tx, stop_signal_rx: Mutex::new(stop_signal_rx),
          can_send_rx: Mutex::new(can_send_rx),
        }
      )
    }
  }
  
  async fn do_loop(mut framed: Framed<TcpStream, GrappleTcpCanBridgeCodec>, inner: Arc<RoboRioDaemonInner>) -> anyhow::Result<()> {
    let mut can_send_rx = inner.can_send_rx.try_lock().map_err(|_| anyhow::anyhow!("This RootDevice is already running!"))?;
    let mut stop_signal_rx = inner.stop_signal_rx.try_lock()?;

    let (mut reassemble_rx, mut reassemble_tx) = FragmentReassembler::new(1000, 8).split();
    let mut device_manager_interval = tokio::time::interval(Duration::from_millis(500));

    loop {
      tokio::select! {
        msg = framed.next() => match msg {
          Some(Ok(msg)) => {
            let id2 = Into::<grapple_frc_msgs::grapple::GrappleMessageId>::into(msg.id);
            // println!("{:?}", id2);
            let manufacturer_msg = ManufacturerMessage::read(&mut BitView::new(&msg.data.0[..]), msg.id);
            match manufacturer_msg {
              Ok(ManufacturerMessage::Grapple(grpl_msg)) => {
                let mut storage = Vec::new();
                if let Ok(Some(grpl_unfragmented)) = reassemble_rx.defragment(msg.timestamp as i64, &msg.id, grpl_msg, &mut storage) {
                  inner.device_manager.on_message("CAN".to_owned(), msg.id.clone().into(), TaggedGrappleMessage::new(msg.id.device_id, grpl_unfragmented.to_static())).await?;
                }
              },
              _ => ()
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
            reassemble_tx.maybe_fragment(device_id, msg, &mut |id, buf| {
              msgs.push(BridgedCANMessage { id, timestamp: 0, data: Cow::<LengthTaggedPayload<u8>>::Owned(LengthTaggedPayloadOwned::new(buf.to_vec())).into() });
            }).ok();

            for msg in msgs {
              framed.send(msg).await?;
            }
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

  async fn deploy() -> anyhow::Result<()> {
    info!("Deploy...");
    let session = SSHSession::connect(&(ROBORIO_ADDRESS.to_owned() + ":22"), "admin", "").await?;

    let file = Daemon::get("grappleHookRoboRioDaemon").ok_or(anyhow::anyhow!("Embedded File Error"))?;
    session.copy(file.data.to_vec(), "/tmp/grapple-hook-daemon").await?;
    
    tokio::spawn(async move {
      session.run("frcKillRobot.sh -t; killall grapple-hook-daemon; frcKillRobot.sh -t; /tmp/grapple-hook-daemon > /tmp/grapple-hook-daemon.log 2>&1").await.ok();
    });

    info!("Deploy Successful!");

    Ok(())
  }

  pub async fn revert_to_robot_code() -> anyhow::Result<()> {
    info!("Reverting to user code...");
    let session = SSHSession::connect("172.22.11.2:22", "admin", "").await?;
    session.run("killall grapple-hook-daemon; frcKillRobot.sh -t -r").await.ok();
    info!("Reverted to user code!");
    Ok(())
  }

  async fn do_start(inner: Arc<RoboRioDaemonInner>) -> anyhow::Result<()> {
    info!("Connecting...");

    Self::deploy().await?;

    let stream = tokio::time::timeout(Duration::from_millis(3000), TcpStream::connect(ROBORIO_ADDRESS.to_owned() + ":8006")).await.map_err(|_| anyhow::anyhow!("Connection Timed Out!"))??;
    let framed = Framed::new(stream, GrappleTcpCanBridgeCodec);

    info!("Connected!");

    tokio::task::spawn(async move {
      inner.running.store(true, std::sync::atomic::Ordering::Relaxed);
      let r = Self::do_loop(framed, inner.clone()).await;
      inner.running.store(false, std::sync::atomic::Ordering::Relaxed);
      tokio::time::timeout(tokio::time::Duration::from_secs(10), Self::revert_to_robot_code()).await.ok();
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
      description: "RoboRIO".to_owned(),
      address: ROBORIO_ADDRESS.to_owned(),
      connected: self.inner.running.load(std::sync::atomic::Ordering::Relaxed)
    })
  }

  async fn device_manager_call(&self, req: DeviceManagerRequest) -> anyhow::Result<DeviceManagerResponse> {
    self.inner.device_manager.rpc_process(req).await
  }
}