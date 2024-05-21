use grapple_frc_msgs::{grapple::{device_info::GrappleModelId, GrappleDeviceMessage, TaggedGrappleMessage}, DEVICE_ID_BROADCAST};
use grapple_hook_macros::rpc;
use tokio::sync::RwLock;

use crate::rpc::RpcBase;
use super::{SendWrapper, SharedInfo, GrappleDevice, Device, GrappleDeviceRequest, GrappleDeviceResponse, VersionGatedDevice, RootDevice, start_field_upgrade, FirmwareValidatingDevice};

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct FlexiCanStatus {
  // last_update: Option<powerful_panda::StatusFrame>
}

pub struct FlexiCan {
  sender: SendWrapper,
  info: SharedInfo,

  grapple_device: GrappleDevice,

  status: RwLock<FlexiCanStatus>
}

impl FlexiCan {
  pub fn new(sender: SendWrapper, info: SharedInfo) -> Self {
    Self {
      sender: sender.clone(),
      info: info.clone(),

      grapple_device: GrappleDevice::new(sender.clone(), info.clone()),

      status: RwLock::new(FlexiCanStatus { })
    }
  }
}

impl VersionGatedDevice for FlexiCan {
  fn validate_version(version: Option<String>) -> anyhow::Result<()> {
    // Self::require_version(version, ">= 2024.2.0, < 2024.3.0")
    Ok(())
  }

  fn firmware_url() -> Option<String> {
    // Some("https://github.com/GrappleRobotics/LaserCAN/releases".to_owned())
    None
  }
}

#[async_trait::async_trait]
impl RootDevice for FlexiCan {
  fn device_class(&self) -> &'static str {
    "FlexiCAN"
  }
}

#[async_trait::async_trait]
impl Device for FlexiCan {
  async fn handle(&self, msg: TaggedGrappleMessage<'static>) -> anyhow::Result<()> {
    if msg.device_id == DEVICE_ID_BROADCAST || Some(msg.device_id) == self.info.read().await.device_id {
      match msg.clone().msg {
        GrappleDeviceMessage::Broadcast(bcast) => match bcast {
          _ => ()
        },
        _ => ()
      }
    }
    
    self.grapple_device.handle(msg.clone()).await?;
    Ok(())
  }
}

impl FirmwareValidatingDevice for FlexiCan {
  fn validate_firmware(_info: &super::DeviceInfo, buf: &[u8]) -> anyhow::Result<()> {
    if &buf[0x200..0x204] == &[0xBEu8, 0xBAu8, 0xFEu8, 0xCAu8] && buf[0x20c] == (GrappleModelId::FlexiCAN as u8) {
      Ok(())
    } else {
      anyhow::bail!("Invalid Firmware File. Are you sure this is the correct firmware?")
    }
  }
}

#[rpc]
impl FlexiCan {
  async fn start_field_upgrade(&self) -> anyhow::Result<()> {
    let serial = self.info.read().await.require_serial()?;
    start_field_upgrade(&self.sender, serial).await
  }

  async fn grapple(&self, msg: GrappleDeviceRequest) -> anyhow::Result<GrappleDeviceResponse> {
    self.grapple_device.rpc_process(msg).await
  }

  async fn status(&self) -> anyhow::Result<FlexiCanStatus> {
    Ok(self.status.read().await.clone())
  }
}
