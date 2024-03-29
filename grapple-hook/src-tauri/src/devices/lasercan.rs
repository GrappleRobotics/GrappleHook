use grapple_frc_msgs::{grapple::{Request, errors::GrappleError, lasercan::{LaserCanMessage, LaserCanRoi, LaserCanMeasurement, LaserCanRangingMode, LaserCanTimingBudget}, GrappleDeviceMessage, TaggedGrappleMessage, device_info::GrappleModelId}, DEVICE_ID_BROADCAST, request_factory};
use grapple_hook_macros::rpc;
use tokio::sync::RwLock;

use crate::rpc::RpcBase;
use super::{SendWrapper, SharedInfo, GrappleDevice, Device, GrappleDeviceRequest, GrappleDeviceResponse, VersionGatedDevice, RootDevice, start_field_upgrade, FirmwareValidatingDevice};

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct LaserCanStatus {
  last_update: Option<LaserCanMeasurement>
}

pub struct LaserCan {
  sender: SendWrapper,
  info: SharedInfo,

  grapple_device: GrappleDevice,

  status: RwLock<LaserCanStatus>
}

impl LaserCan {
  pub fn new(sender: SendWrapper, info: SharedInfo) -> Self {
    Self {
      sender: sender.clone(),
      info: info.clone(),

      grapple_device: GrappleDevice::new(sender.clone(), info.clone()),

      status: RwLock::new(LaserCanStatus { last_update: None })
    }
  }
}

impl VersionGatedDevice for LaserCan {
  fn validate_version(version: Option<String>) -> anyhow::Result<()> {
    Self::require_version(version, ">= 2024.2.0, < 2024.3.0")
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
  async fn handle(&self, msg: TaggedGrappleMessage<'static>) -> anyhow::Result<()> {
    if msg.device_id == DEVICE_ID_BROADCAST || Some(msg.device_id) == self.info.read().await.device_id {
      match msg.clone().msg {
        GrappleDeviceMessage::Broadcast(bcast) => match bcast {
          _ => ()
        },
        GrappleDeviceMessage::DistanceSensor(sensor) => match sensor {
          LaserCanMessage::Measurement(measurement) => {
            self.status.write().await.last_update = Some(measurement);
          },
          _ => ()
        },
        _ => ()
      }
    }
    
    self.grapple_device.handle(msg.clone()).await?;
    Ok(())
  }
}

impl FirmwareValidatingDevice for LaserCan {
  fn validate_firmware(_info: &super::DeviceInfo, buf: &[u8]) -> anyhow::Result<()> {
    if &buf[0x150..0x154] == &[0xBEu8, 0xBAu8, 0xFEu8, 0xCAu8] && buf[0x15c] == (GrappleModelId::LaserCan as u8) {
      Ok(())
    } else {
      anyhow::bail!("Invalid Firmware File. Are you sure this is the correct firmware?")
    }
  }
}

#[rpc]
impl LaserCan {
  async fn start_field_upgrade(&self) -> anyhow::Result<()> {
    let serial = self.info.read().await.require_serial()?;
    start_field_upgrade(&self.sender, serial).await
  }

  async fn set_range(&self, mode: LaserCanRangingMode) -> anyhow::Result<()> {
    let id = self.info.read().await.require_device_id()?;
    let (encode, decode) = request_factory!(data, GrappleDeviceMessage::DistanceSensor(LaserCanMessage::SetRange(data)));

    let msg = self.sender.request(TaggedGrappleMessage::new(id, encode(mode)), 2000).await?;
    decode(msg.msg)??;
    Ok(())
  }

  async fn set_roi(&self, roi: LaserCanRoi) -> anyhow::Result<()> {
    let id = self.info.read().await.require_device_id()?;
    let (encode, decode) = request_factory!(data, GrappleDeviceMessage::DistanceSensor(LaserCanMessage::SetRoi(data)));

    let msg = self.sender.request(TaggedGrappleMessage::new(id, encode(roi)), 500).await?;
    decode(msg.msg)??;
    Ok(())
  }

  async fn set_timing_budget(&self, budget: LaserCanTimingBudget) -> anyhow::Result<()> {
    let id = self.info.read().await.require_device_id()?;
    let (encode, decode) = request_factory!(data, GrappleDeviceMessage::DistanceSensor(LaserCanMessage::SetTimingBudget(data)));

    let msg = self.sender.request(TaggedGrappleMessage::new(id, encode(budget)), 500).await?;
    decode(msg.msg)??;
    Ok(())
  }

  async fn grapple(&self, msg: GrappleDeviceRequest) -> anyhow::Result<GrappleDeviceResponse> {
    self.grapple_device.rpc_process(msg).await
  }

  async fn status(&self) -> anyhow::Result<LaserCanStatus> {
    Ok(self.status.read().await.clone())
  }
}
