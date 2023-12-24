use grapple_frc_msgs::{grapple::{Request, errors::{GrappleError, CowStr}, lasercan::{LaserCanStatusFrame, LaserCanMessage, LaserCanRoi}, GrappleDeviceMessage, TaggedGrappleMessage}, DEVICE_ID_BROADCAST, Message, ManufacturerMessage, request_factory};
use grapple_hook_macros::rpc;
use tokio::sync::RwLock;

use crate::rpc::RpcBase;
use super::{SendWrapper, SharedInfo, GrappleDevice, FirmwareUpgradeDevice, Device, FirmwareUpgradeDeviceRequest, GrappleDeviceRequest, GrappleDeviceResponse, FirmwareUpgradeDeviceResponse};

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct LaserCanStatus {
  last_update: Option<LaserCanStatusFrame>
}

pub struct LaserCan {
  sender: SendWrapper,
  info: SharedInfo,

  grapple_device: GrappleDevice,
  firmware_upgrade_device: FirmwareUpgradeDevice,

  status: RwLock<LaserCanStatus>
}

impl LaserCan {
  pub fn new(sender: SendWrapper, info: SharedInfo) -> Self {
    Self {
      sender: sender.clone(),
      info: info.clone(),

      grapple_device: GrappleDevice::new(sender.clone(), info.clone()),
      firmware_upgrade_device: FirmwareUpgradeDevice::new(sender.clone(), info.clone()),

      status: RwLock::new(LaserCanStatus { last_update: None })
    }
  }
}

#[async_trait::async_trait]
impl Device for LaserCan {
  async fn handle(&self, msg: TaggedGrappleMessage) -> anyhow::Result<()> {
    if msg.device_id == DEVICE_ID_BROADCAST || Some(msg.device_id) == self.info.read().await.device_id {
      match msg.clone().msg {
        GrappleDeviceMessage::Broadcast(bcast) => match bcast {
          _ => ()
        },
        GrappleDeviceMessage::DistanceSensor(sensor) => match sensor {
          LaserCanMessage::Status(status) => {
            self.status.write().await.last_update = Some(status);
          },
          _ => ()
        },
        _ => ()
      }
    }
    
    self.grapple_device.handle(msg.clone()).await?;
    self.firmware_upgrade_device.handle(msg).await?;
    Ok(())
  }
}

#[rpc]
impl LaserCan {
  async fn set_range(&self, long: bool) -> anyhow::Result<()> {
    let id = self.info.read().await.require_device_id()?;
    let (encode, decode) = request_factory!(data, GrappleDeviceMessage::DistanceSensor(LaserCanMessage::SetRange(data)));

    let msg = self.sender.request(TaggedGrappleMessage::new(id, encode(long)), 500).await?;
    decode(msg.msg)??;
    Ok(())
  }

  async fn set_roi(&self, roi: LaserCanRoi) -> anyhow::Result<()> {
    // TODO: Validation
    let id = self.info.read().await.require_device_id()?;
    let (encode, decode) = request_factory!(data, GrappleDeviceMessage::DistanceSensor(LaserCanMessage::SetRoi(data)));

    let msg = self.sender.request(TaggedGrappleMessage::new(id, encode(roi)), 500).await?;
    decode(msg.msg)??;
    Ok(())
  }

  async fn set_timing_budget(&self, budget: u8) -> anyhow::Result<()> {
    let id = self.info.read().await.require_device_id()?;
    let (encode, decode) = request_factory!(data, GrappleDeviceMessage::DistanceSensor(LaserCanMessage::SetTimingBudget(data)));

    let msg = self.sender.request(TaggedGrappleMessage::new(id, encode(budget)), 500).await?;
    decode(msg.msg)??;
    Ok(())
  }

  async fn grapple(&self, msg: GrappleDeviceRequest) -> anyhow::Result<GrappleDeviceResponse> {
    self.grapple_device.rpc_process(msg).await
  }

  async fn firmware(&self, msg: FirmwareUpgradeDeviceRequest) -> anyhow::Result<FirmwareUpgradeDeviceResponse> {
    self.firmware_upgrade_device.rpc_process(msg).await
  }

  async fn status(&self) -> anyhow::Result<LaserCanStatus> {
    Ok(self.status.read().await.clone())
  }
}

// use frc_can::grapple::{Grapple, GrappleLaserCan, GrappleLaserCanRoi};

// use super::device_manager::{CanProviderT, DeviceSpecificDataClass};

// #[derive(Debug, Clone, serde::Serialize)]
// pub struct LaserCanData {
//   device_id: u8,
//   status: u8,
//   distance: u16,
//   ambient: u16,
//   ranging_long: bool,
//   budget_ms: u8,
//   roi: GrappleLaserCanRoi
// }

// impl LaserCanData {
//   pub fn new(device_id: u8) -> Self {
//     Self {
//       device_id,
//       status: 1,
//       distance: 0,
//       ambient: 0,
//       ranging_long: false,
//       budget_ms: 0,
//       roi: GrappleLaserCanRoi { x: 8, y: 8, w: 16, h: 16 }
//     }
//   }
// }

// #[async_trait::async_trait]
// impl DeviceSpecificDataClass for LaserCanData {
//   async fn handle(&mut self, msg: &Grapple) {
//     match msg {
//       Grapple::LaserCan(lc) => match lc {
//         GrappleLaserCan::Status { device_id, status, distance_mm, ambient, long, budget_ms, roi } if *device_id == self.device_id => {
//           self.status = *status;
//           self.distance = *distance_mm;
//           self.ambient = *ambient;
//           self.ranging_long = *long;
//           self.budget_ms = *budget_ms;
//           self.roi = roi.clone();
//         },
//         _ => (),
//       },
//       _ => ()
//     }
//   }
// }