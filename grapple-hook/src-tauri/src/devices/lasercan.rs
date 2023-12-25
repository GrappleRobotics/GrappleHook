use grapple_frc_msgs::{grapple::{Request, errors::{GrappleError, CowStr}, lasercan::{LaserCanStatusFrame, LaserCanMessage, LaserCanRoi}, GrappleDeviceMessage, TaggedGrappleMessage}, DEVICE_ID_BROADCAST, Message, ManufacturerMessage, request_factory};
use grapple_hook_macros::rpc;
use tokio::sync::RwLock;

use crate::rpc::RpcBase;
use super::{SendWrapper, SharedInfo, GrappleDevice, FirmwareUpgradeDevice, Device, FirmwareUpgradeDeviceRequest, GrappleDeviceRequest, GrappleDeviceResponse, FirmwareUpgradeDeviceResponse, VersionGatedDevice, RootDevice};

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

impl VersionGatedDevice for LaserCan {
  fn validate_version(version: Option<String>) -> anyhow::Result<()> {
    Self::require_version(version, ">= 2024.0.0, < 2024.1.0")
  }

  fn firmware_url() -> Option<String> {
    Some("https://github.com/GrappleRobotics/LaserCAN/releases".to_owned())
  }
}

#[async_trait::async_trait]
impl RootDevice for LaserCan {
  fn device_class(&self) -> &'static str {
    "LaserCAN"
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
