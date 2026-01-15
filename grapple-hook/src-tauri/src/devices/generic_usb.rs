use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use bounded_static::ToBoundedStatic;
use futures::{SinkExt, StreamExt};
use grapple_frc_msgs::{
    binmarshal::{
        AsymmetricCow, BitView, BitWriter, BufferBitWriter, Demarshal, Marshal, MarshalUpdate,
    },
    bridge::BridgedCANMessage,
    grapple::{fragments::FragmentReassembler, GrappleMessageId, TaggedGrappleMessage},
    ManufacturerMessage,
};
use log::{info, warn};
use serde_json::json;
use tokio::sync::{mpsc, Mutex};
use tokio_serial::{SerialPort, SerialStream, UsbPortInfo};
use tokio_util::codec::Framed;

use crate::codecs::usb_codec::GrappleUsbCodec;

use super::{
    device_manager::{DeviceManager, DeviceManagerRequest, DeviceManagerResponse},
    provider::{DeviceProvider, ProviderInfo},
};

pub struct GenericUSBInner {
    address: String,
    running: AtomicBool,
    device_manager: DeviceManager,

    stop_signal_tx: mpsc::Sender<()>,
    stop_signal_rx: Mutex<mpsc::Receiver<()>>,

    send_rx: Mutex<mpsc::Receiver<TaggedGrappleMessage<'static>>>,
}

pub struct GenericUSB {
    inner: Arc<GenericUSBInner>,
}

impl GenericUSB {
    pub fn new(address: String) -> Self {
        let (send_tx, send_rx) = mpsc::channel(100);
        let (stop_signal_tx, stop_signal_rx) = mpsc::channel(5);

        let mut sends = HashMap::new();
        sends.insert("USB".to_owned(), send_tx);

        Self {
            inner: Arc::new(GenericUSBInner {
                address,
                running: AtomicBool::new(false),
                device_manager: DeviceManager::new(sends),
                stop_signal_tx,
                stop_signal_rx: Mutex::new(stop_signal_rx),
                send_rx: Mutex::new(send_rx),
            }),
        }
    }

    async fn do_loop(
        mut framed: Framed<SerialStream, GrappleUsbCodec>,
        inner: Arc<GenericUSBInner>,
    ) -> anyhow::Result<()> {
        let mut send_rx = inner
            .send_rx
            .try_lock()
            .map_err(|_| anyhow::anyhow!("This RootDevice is already running!"))?;
        let mut stop_signal_rx = inner.stop_signal_rx.try_lock()?;

        let (mut reassemble_rx, _) = FragmentReassembler::new(1000, 1024).split();
        let mut device_manager_interval = tokio::time::interval(Duration::from_millis(500));

        loop {
            tokio::select! {
              msg = framed.next() => match msg {
                Some(Ok(msg)) => {
                  let manufacturer_msg = ManufacturerMessage::read(&mut BitView::new(&msg.data[..]), msg.id);
                  match manufacturer_msg {
                    Ok(ManufacturerMessage::Grapple(grpl_msg)) => {
                      let mut storage = Vec::new();
                      if let Ok(Some((gid, grpl_unfragmented))) = reassemble_rx.defragment(0, &msg.id, grpl_msg, &mut storage) {
                        inner.device_manager.on_message("USB".to_owned(), gid, TaggedGrappleMessage::new(msg.id.device_id, grpl_unfragmented.to_static())).await?;
                      }
                    },
                    _ => ()
                  }
                },
                Some(Err(e)) => anyhow::bail!(e),
                None => ()
              },
              msg = send_rx.recv() => match msg {
                Some(mut tagged) => {
                  let mut payload = [0u8; 1024];
                  let mut writer = BufferBitWriter::new(&mut payload);
                  let mut id = GrappleMessageId::new(tagged.device_id);

                  tagged.msg.update(&mut id);
                  tagged.msg.write(&mut writer, id.clone()).ok();

                  let mut msgs = vec![
                    (id.into(), writer.slice().to_vec())
                  ];

                  for msg in msgs {
                    framed.send(BridgedCANMessage { id: msg.0, timestamp: 0, data: AsymmetricCow(Cow::Borrowed((&msg.1[..]).into())) }).await?;
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

    async fn do_start(inner: Arc<GenericUSBInner>) -> anyhow::Result<()> {
        info!("Connecting...");

        let mut port =
            tokio_serial::SerialStream::open(&tokio_serial::new(inner.address.clone(), 115200))?;
        port.set_baud_rate(1200)?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        port.set_baud_rate(115200)?;

        // let stream = tokio::time::timeout(Duration::from_millis(3000), TcpStream::connect(ROBORIO_ADDRESS.to_owned() + ":8006")).await.map_err(|_| anyhow::anyhow!("Connection Timed Out!"))??;
        let framed = Framed::new(port, GrappleUsbCodec);

        info!("Connected!");

        tokio::task::spawn(async move {
            inner
                .running
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let r = Self::do_loop(framed, inner.clone()).await;
            inner
                .running
                .store(false, std::sync::atomic::Ordering::Relaxed);
            inner.device_manager.reset().await;
            match r {
                Ok(_) => info!("GenericUSB runner stopped gracefully"),
                Err(e) => warn!("GenericUSB runner stopped with error: {}", e),
            }
        });

        Ok(())
    }
}

#[async_trait::async_trait]
impl DeviceProvider for GenericUSB {
    async fn connect(&self) -> anyhow::Result<()> {
        Self::do_start(self.inner.clone()).await?;
        Ok(())
    }

    async fn disconnect(&self) -> anyhow::Result<()> {
        self.inner.stop_signal_tx.send(()).await.ok();
        Ok(())
    }

    async fn info(&self) -> anyhow::Result<ProviderInfo> {
        Ok(ProviderInfo {
            ty: "Generic-USB".to_owned(),
            description: "Grapple USB Device".to_owned(),
            address: self.inner.address.clone(),
            connected: self
                .inner
                .running
                .load(std::sync::atomic::Ordering::Relaxed),
        })
    }

    async fn call(&self, _req: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        Ok(json!({}))
    }

    async fn device_manager_call(
        &self,
        req: DeviceManagerRequest,
    ) -> anyhow::Result<DeviceManagerResponse> {
        self.inner.device_manager.rpc_process(req).await
    }
}
