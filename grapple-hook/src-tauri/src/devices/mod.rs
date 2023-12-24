pub mod device_manager;
pub mod provider;
pub mod provider_manager;
pub mod roborio;
pub mod lasercan;

use std::{sync::Arc, time::Duration, collections::{LinkedList, HashMap}};

use grapple_frc_msgs::{Validate, grapple::{device_info::GrappleModelId, GrappleDeviceMessage, firmware::GrappleFirmwareMessage, TaggedGrappleMessage, GrappleMessageId}, Message, DEVICE_ID_BROADCAST, ManufacturerMessage, binmarshal::{LengthTaggedVec, BinMarshal}, MessageId};
use grapple_hook_macros::rpc;
use log::info;
use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc, RwLock, Notify, oneshot};
use uuid::Uuid;

use crate::rpc::RpcBase;

use self::device_manager::RepliesWaiting;

#[derive(Clone)]
pub struct SendWrapper(mpsc::Sender<TaggedGrappleMessage>, RepliesWaiting);

impl SendWrapper {
  async fn send(&self, msg: TaggedGrappleMessage) -> anyhow::Result<()> {
    msg.msg.validate()?;
    self.0.send(msg).await?;
    Ok(())
  }

  async fn request(&self, mut msg: TaggedGrappleMessage, timeout_ms: usize) -> anyhow::Result<TaggedGrappleMessage> {
    let mut id = GrappleMessageId::new(msg.device_id);
    msg.msg.update(&mut id);

    let mut complement_id = id.clone();
    complement_id.ack_flag = true;
    let complement_id_u32: u32 = Into::<MessageId>::into(complement_id).into();

    let uuid = Uuid::new_v4();

    let (tx, rx) = oneshot::channel();
    {
      let mut hm = self.1.write().await;
      if !hm.contains_key(&complement_id_u32) {
        hm.insert(complement_id_u32, HashMap::new());
      }
      hm.get_mut(&complement_id_u32).unwrap().insert(uuid, tx);
    }
    self.send(msg).await?;

    match tokio::time::timeout(Duration::from_millis(timeout_ms as u64), rx).await {
      Ok(result) => result.map_err(|e| anyhow::anyhow!(e)),
      Err(_) => {
        // Timed out - remove it from the replies waiting
        let mut hm = self.1.write().await;
        hm.get_mut(&complement_id_u32).map(|x| x.remove(&uuid));
        anyhow::bail!("Timed out waiting for response")
      },
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum DeviceType {
  Grapple(GrappleModelId),
  RoboRIO,
  Unknown
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeviceInfo {
  pub device_type: DeviceType,
  pub firmware_version: Option<String>,
  pub serial: Option<u32>,
  pub is_dfu: bool,
  pub is_dfu_in_progress: bool,
  pub name: Option<String>,
  pub device_id: Option<u8>
}

impl DeviceInfo {
  pub fn require_serial(&self) -> anyhow::Result<u32> {
    return self.serial.ok_or(anyhow::anyhow!("No Serial Number for Device!"))
  }

  pub fn require_device_id(&self) -> anyhow::Result<u8> {
    return self.device_id.ok_or(anyhow::anyhow!("No Device ID for Device!"))
  }
}

#[async_trait::async_trait]
pub trait Device : RpcBase {
  async fn handle(&self, msg: TaggedGrappleMessage) -> anyhow::Result<()> { Ok(()) }
}

pub type SharedInfo = Arc<RwLock<DeviceInfo>>;

/* GRAPPLE DEVICE */

pub struct GrappleDevice {
  sender: SendWrapper,
  info: SharedInfo
}

impl GrappleDevice {
  pub fn new(sender: SendWrapper, info: SharedInfo) -> Self {
    Self { sender, info }
  }
}

#[rpc]
impl GrappleDevice {
  async fn blink(&self) -> anyhow::Result<()> {
    self.sender.send(TaggedGrappleMessage::new(
      DEVICE_ID_BROADCAST,
      grapple_frc_msgs::grapple::GrappleDeviceMessage::Broadcast(
        grapple_frc_msgs::grapple::GrappleBroadcastMessage::DeviceInfo(grapple_frc_msgs::grapple::device_info::GrappleDeviceInfo::Blink {
          serial: self.info.read().await.require_serial()?
        })
      )
    )).await
  }

  async fn set_id(&self, id: u8) -> anyhow::Result<()>  {
    self.sender.send(TaggedGrappleMessage::new(
      DEVICE_ID_BROADCAST,
      grapple_frc_msgs::grapple::GrappleDeviceMessage::Broadcast(
        grapple_frc_msgs::grapple::GrappleBroadcastMessage::DeviceInfo(grapple_frc_msgs::grapple::device_info::GrappleDeviceInfo::SetId {
          serial: self.info.read().await.require_serial()?, new_id: id
        })
      )
    )).await
  }

  async fn set_name(&self, name: String) -> anyhow::Result<()>  {
    self.sender.send(TaggedGrappleMessage::new(
      DEVICE_ID_BROADCAST,
      grapple_frc_msgs::grapple::GrappleDeviceMessage::Broadcast(
        grapple_frc_msgs::grapple::GrappleBroadcastMessage::DeviceInfo(grapple_frc_msgs::grapple::device_info::GrappleDeviceInfo::SetName {
          serial: self.info.read().await.require_serial()?,
          name
        })
      )
    )).await
  }

  async fn commit_to_eeprom(&self) -> anyhow::Result<()>  {
    self.sender.send(TaggedGrappleMessage::new(
      DEVICE_ID_BROADCAST,
      grapple_frc_msgs::grapple::GrappleDeviceMessage::Broadcast(
        grapple_frc_msgs::grapple::GrappleBroadcastMessage::DeviceInfo(grapple_frc_msgs::grapple::device_info::GrappleDeviceInfo::CommitConfig {
          serial: self.info.read().await.require_serial()?,
        })
      )
    )).await
  }
}

impl Device for GrappleDevice {}

/* FIRMWARE UPGRADE DEVICE */

pub struct FirmwareUpgradeDevice {
  sender: SendWrapper,
  info: SharedInfo,
  progress: Arc<RwLock<Option<f64>>>,
  ack: Arc<Notify>
}

impl FirmwareUpgradeDevice {
  pub fn new(sender: SendWrapper, info: SharedInfo) -> Self {
    Self { sender, info, progress: Arc::new(RwLock::new(None)), ack: Arc::new(Notify::new()) }
  }

  pub async fn field_upgrade_worker(sender: SendWrapper, id: u8, data: &[u8], progress: Arc<RwLock<Option<f64>>>, ack: Arc<Notify>) -> anyhow::Result<()> {
    *progress.write().await = Some(0.0);
    let chunks = data.chunks(8);
    let nchunks = chunks.len();
    for (i, chunk) in chunks.enumerate() {
      info!("Chunk {}", i);
      let mut c = [0u8; 8];
      c[0..chunk.len()].copy_from_slice(chunk);

      sender.send(TaggedGrappleMessage::new(
        id,
        GrappleDeviceMessage::FirmwareUpdate(
          GrappleFirmwareMessage::UpdatePart(c)
        )
      )).await?;
      tokio::time::timeout(Duration::from_millis(1000), ack.notified()).await?;
      *progress.write().await = Some((i + 1) as f64 / (nchunks as f64) * 100.0);
    }

    *progress.write().await = Some(100.0);
    sender.send(TaggedGrappleMessage::new(
      id,
      GrappleDeviceMessage::FirmwareUpdate(
        GrappleFirmwareMessage::UpdateDone
      )
    )).await?;
    *progress.write().await = None;

    Ok(())
  }
}

#[rpc]
impl FirmwareUpgradeDevice {
  async fn start_field_upgrade(&self) -> anyhow::Result<()> {
    self.sender.send(TaggedGrappleMessage::new(
      DEVICE_ID_BROADCAST,
      GrappleDeviceMessage::FirmwareUpdate(
        GrappleFirmwareMessage::StartFieldUpgrade { serial: self.info.read().await.require_serial()? }
      )
    )).await
  }

  async fn do_field_upgrade(&self, data: Vec<u8>) -> anyhow::Result<()> {
    let sender = self.sender.clone();
    let progress = self.progress.clone();
    let id = self.info.read().await.require_device_id()?;
    let notify = self.ack.clone();

    tokio::task::spawn(async move {
      let data = data;
      Self::field_upgrade_worker(sender, id, &data[..], progress, notify).await.ok();
    });
    Ok(())
  }

  async fn progress(&self) -> anyhow::Result<Option<f64>> {
    Ok(self.progress.read().await.clone())
  }
}

#[async_trait::async_trait]
impl Device for FirmwareUpgradeDevice {
  async fn handle(&self, msg: TaggedGrappleMessage) -> anyhow::Result<()> {
    if msg.device_id == DEVICE_ID_BROADCAST || Some(msg.device_id) == self.info.read().await.device_id {
      match msg.clone().msg {
        GrappleDeviceMessage::FirmwareUpdate(fw) => match fw {
          GrappleFirmwareMessage::UpdatePartAck => {
            self.ack.notify_one();
          },
          _ => ()
        },
        _ => ()
      }
    }
    Ok(())
  }
}

// pub trait RpcDevice : Device + Rpc<RpcT = DeviceRPC> + RpcWithState<State = DeviceState> {}

// #[async_trait::async_trait]
// pub trait BasicDevice : Sync {
//   fn capabilities(&self) -> Vec<Capability>;

//   async fn info(&self) -> DeviceInfo;
//   async fn set_info(&self, info: DeviceInfo);

//   async fn send_now(&self, message: Message) -> anyhow::Result<()>;
//   async fn handle_msg(&self, msg: Message) -> anyhow::Result<()>;

//   async fn rpc_specific(&self, data: serde_json::Value) -> RpcResult;
//   async fn state_specific(&self) -> serde_json::Value;
// }

// // TODO: Split out into different classes, composed based on capabilities
// // SpiderLan > FirmwareUpgradableDevice > GrappleDevice > BasicDevice

// #[async_trait::async_trait]
// pub trait Device : BasicDevice {
//   async fn handle(&self, message: Message) -> anyhow::Result<()>;

//   async fn send(&self, mut message: Message) -> anyhow::Result<()> {
//     message.update().map_err(|e| anyhow::anyhow!("Update Error: {}", e))?;
//     message.validate().map_err(|e| anyhow::anyhow!("Validation Error: {}", e))?;
//     self.send_now(message).await?;
//     Ok(())
//   }
//   async fn blink(&self) -> anyhow::Result<()> { anyhow::bail!("Blink is unsupported on this device") }
//   async fn set_id(&self, id: u8) -> anyhow::Result<()> { anyhow::bail!("Set ID is unsupported on this device") }
//   async fn set_name(&self, name: String) -> anyhow::Result<()> { anyhow::bail!("Set Name is unsupported on this device") }
//   async fn commit(&self) -> anyhow::Result<()> { anyhow::bail!("Commit is unsupported on this device") }
//   async fn start_field_upgrade(&self) -> anyhow::Result<()> { anyhow::bail!("Start Field Upgrade is unsupported on this device") }
//   async fn do_field_upgrade(&self, data: Vec<u8>) -> anyhow::Result<()> { anyhow::bail!("Do Field Upgrade is unsupported on this device") }
// }

// #[macro_export]
// macro_rules! grapple_device_impl {
//   ($cls:ident) => {
//     #[async_trait::async_trait]
//     impl crate::devices::Device for $cls {
//       async fn handle(&self, message: Message) -> anyhow::Result<()> {
//         BasicDevice::handle_msg(self, message).await
//       }

//       async fn blink(&self) -> anyhow::Result<()> {
//         if let Some(serial) = self.info().await.serial {
//           self.send(Message::new(
//             grapple_frc_msgs::DEVICE_ID_BROADCAST,
//             ManufacturerMessage::Grapple(grapple_frc_msgs::grapple::GrappleDeviceMessage::Broadcast(grapple_frc_msgs::grapple::GrappleBroadcastMessage::DeviceInfo(
//               grapple_frc_msgs::grapple::device_info::GrappleDeviceInfo::Blink { serial }
//             )))
//           )).await?;
//           Ok(())
//         } else {
//           anyhow::bail!("Can't Blink a non-Grapple Device!")
//         }
//       }

//       async fn set_id(&self, id: u8) -> anyhow::Result<()> {
//         if let Some(serial) = self.info().await.serial {
//           self.send(Message::new(
//             grapple_frc_msgs::DEVICE_ID_BROADCAST,
//             ManufacturerMessage::Grapple(grapple_frc_msgs::grapple::GrappleDeviceMessage::Broadcast(grapple_frc_msgs::grapple::GrappleBroadcastMessage::DeviceInfo(
//               grapple_frc_msgs::grapple::device_info::GrappleDeviceInfo::SetId { serial, new_id: id }
//             )))
//           )).await?;
//           Ok(())
//         } else {
//           anyhow::bail!("Can't Set ID of a non-Grapple Device!")
//         }
//       }

//       async fn set_name(&self, name: String) -> anyhow::Result<()> {
//         if let Some(serial) = self.info().await.serial {
//           self.send(Message::new(
//             grapple_frc_msgs::DEVICE_ID_BROADCAST,
//             ManufacturerMessage::Grapple(grapple_frc_msgs::grapple::GrappleDeviceMessage::Broadcast(grapple_frc_msgs::grapple::GrappleBroadcastMessage::DeviceInfo(
//               grapple_frc_msgs::grapple::device_info::GrappleDeviceInfo::SetName { serial, name_len: name.len() as u8, name: name.as_bytes().to_vec() }
//             )))
//           )).await?;
//           Ok(())
//         } else {
//           anyhow::bail!("Can't Set Name of a non-Grapple Device!")
//         }
//       }

//       async fn commit(&self) -> anyhow::Result<()> {
//         if let Some(serial) = self.info().await.serial {
//           self.send(Message::new(
//             grapple_frc_msgs::DEVICE_ID_BROADCAST,
//             ManufacturerMessage::Grapple(grapple_frc_msgs::grapple::GrappleDeviceMessage::Broadcast(grapple_frc_msgs::grapple::GrappleBroadcastMessage::DeviceInfo(
//               grapple_frc_msgs::grapple::device_info::GrappleDeviceInfo::CommitConfig { serial }
//             )))
//           )).await?;
//           Ok(())
//         } else {
//           anyhow::bail!("Can't Commit to EEPROM for a non-Grapple Device!")
//         }
//       }

//       async fn start_field_upgrade(&self) -> anyhow::Result<()> {
//         if let Some(serial) = self.info().await.serial {
//           self.send(Message::new(
//             grapple_frc_msgs::DEVICE_ID_BROADCAST,
//             ManufacturerMessage::Grapple(grapple_frc_msgs::grapple::GrappleDeviceMessage::FirmwareUpdate(
//               grapple_frc_msgs::grapple::firmware::GrappleFirmwareMessage::StartFieldUpgrade { serial }
//             ))
//           )).await?;
//           Ok(())
//         } else {
//           anyhow::bail!("Can't Commit to EEPROM for a non-Grapple Device!")
//         }
//       }
//     }
//   }
// }

// #[macro_export]
// macro_rules! device_rpc_impl {
//   ($cls:ident) => {
//     #[async_trait::async_trait]
//     impl crate::rpc::Rpc for $cls  {
//       type RpcT = crate::devices::DeviceRPC;
//       async fn rpc(&self, data: Self::RpcT) -> crate::rpc::RpcResult {
//         use crate::devices::Device;
//         match data {
//           crate::devices::DeviceRPC::Blink => { self.blink().await?; crate::rpc::to_rpc_result(()) },
//           crate::devices::DeviceRPC::SetId(id) => { self.set_id(id).await?; crate::rpc::to_rpc_result(()) },
//           crate::devices::DeviceRPC::SetName(name) => { self.set_name(name).await?; crate::rpc::to_rpc_result(()) },
//           crate::devices::DeviceRPC::Commit => { self.commit().await?; crate::rpc::to_rpc_result(()) },
//           crate::devices::DeviceRPC::StartFieldUpgrade => { self.start_field_upgrade().await?; crate::rpc::to_rpc_result(()) },
//           crate::devices::DeviceRPC::DoFieldUpgrade(data) => { self.do_field_upgrade(data).await?; crate::rpc::to_rpc_result(()) },
//           crate::devices::DeviceRPC::Specific(msg) => self.rpc_specific(msg).await
//         }
//       }
//     }

//     #[async_trait::async_trait]
//     impl crate::rpc::RpcWithState for $cls {
//       type State = crate::devices::DeviceState;
//       async fn state(&self) -> Self::State {
//         crate::devices::DeviceState {
//           info: self.info().await,
//           capabilities: self.capabilities(),
//           specific: self.state_specific().await
//         }
//       }
//     }
//   }
// }

// impl<T> RpcDevice for T where T: Device + Rpc<RpcT = DeviceRPC> + RpcWithState<State = DeviceState> {}
