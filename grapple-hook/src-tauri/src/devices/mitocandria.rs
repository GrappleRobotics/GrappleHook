use std::thread::current;

use grapple_frc_msgs::{grapple::{device_info::GrappleModelId, errors::GrappleError, mitocandria::{self, MitocandriaAdjustableChannelRequest, MitocandriaSwitchableChannelRequest}, GrappleDeviceMessage, Request, TaggedGrappleMessage}, request_factory, DEVICE_ID_BROADCAST};
use grapple_hook_macros::rpc;
use tokio::sync::RwLock;

use crate::{rpc::RpcBase, updates::{most_recent_update_available, LightReleaseResponse}};
use super::{check_for_new_firmware_release_rpc_target, start_field_upgrade, Device, FirmwareValidatingDevice, GrappleDevice, GrappleDeviceRequest, GrappleDeviceResponse, HasFirmwareUpdateURLDevice, RootDevice, SendWrapper, SharedInfo, VersionGatedDevice};

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct MitocandriaStatus {
  last_update: Option<mitocandria::MitocandriaStatusFrame>
}

pub struct Mitocandria {
  sender: SendWrapper,
  info: SharedInfo,

  grapple_device: GrappleDevice,

  status: RwLock<MitocandriaStatus>
}

impl Mitocandria {
  pub fn new(sender: SendWrapper, info: SharedInfo) -> Self {
    Self {
      sender: sender.clone(),
      info: info.clone(),

      grapple_device: GrappleDevice::new(sender.clone(), info.clone()),

      status: RwLock::new(MitocandriaStatus { last_update: None })
    }
  }
}

impl HasFirmwareUpdateURLDevice for Mitocandria {
  fn firmware_url() -> Option<String> {
    Some("https://github.com/GrappleRobotics/Binaries/releases".to_owned())
  }
}

#[async_trait::async_trait]
impl VersionGatedDevice for Mitocandria {
  fn validate_version(version: Option<String>) -> anyhow::Result<()> {
    Self::require_version(version, ">= 2025.0.0, < 2025.1.0")
  }

  async fn check_for_new_firmware_release(current_version: &str) -> Option<LightReleaseResponse>{
    let regex = regex::Regex::new("^([a-zA-Z0-9_]+)-v?(.+)$").unwrap();
    let current = semver::Version::parse(&current_version).ok()?;

    most_recent_update_available(
      "https://api.github.com/repos/GrappleRobotics/Binaries/releases",
      |release| {
        match release.tag_name.to_string() {
          x if x.starts_with("mitocandria") => {
            if let Some(captures) = regex.captures(x.as_str()) {
              let vers = semver::Version::parse(captures.get(2).map(|x| x.as_str()).unwrap_or("")).ok();
              println!("{:?}", vers);
              if let Some(vers) = vers {
                vers > current && Self::validate_version(Some(vers.to_string())).is_ok()
              } else {
                false
              }
            } else {
              false
            }
          },
          _ => false
        }
      }
    ).await.ok().flatten()
  }
}

#[async_trait::async_trait]
impl RootDevice for Mitocandria {
  fn device_class(&self) -> &'static str {
    "MitoCANdria"
  }
}

#[async_trait::async_trait]
impl Device for Mitocandria {
  async fn handle(&self, msg: TaggedGrappleMessage<'static>) -> anyhow::Result<()> {
    if msg.device_id == DEVICE_ID_BROADCAST || Some(msg.device_id) == self.info.read().await.device_id {
      match msg.clone().msg {
        GrappleDeviceMessage::Broadcast(bcast) => match bcast {
          _ => ()
        },
        GrappleDeviceMessage::PowerDistributionModule(pdm) => match pdm {
          mitocandria::MitocandriaMessage::StatusFrame(status) => {
            self.status.write().await.last_update = Some(status);
          },
          _ => ()
        }
        _ => ()
      }
    }
    
    self.grapple_device.handle(msg.clone()).await?;
    Ok(())
  }
}

impl FirmwareValidatingDevice for Mitocandria {
  fn validate_firmware(_info: &super::DeviceInfo, buf: &[u8]) -> anyhow::Result<()> {
    if &buf[0x200..0x204] == &[0xBEu8, 0xBAu8, 0xFEu8, 0xCAu8] && buf[0x20c] == (GrappleModelId::MitoCANdria as u8) {
      Ok(())
    } else {
      anyhow::bail!("Invalid Firmware File. Are you sure this is the correct firmware?")
    }
  }
}

#[rpc]
impl Mitocandria {
  async fn start_field_upgrade(&self) -> anyhow::Result<()> {
    let serial = self.info.read().await.require_serial()?;
    start_field_upgrade(&self.sender, serial).await
  }

  async fn set_switchable_channel(&self, channel: MitocandriaSwitchableChannelRequest) -> anyhow::Result<()> {
    let id = self.info.read().await.require_device_id()?;
    let (encode, decode) = request_factory!(data, GrappleDeviceMessage::PowerDistributionModule(
      mitocandria::MitocandriaMessage::ChannelRequest(mitocandria::MitocandriaChannelRequest::SetSwitchableChannel(data))
    ));

    let msg = self.sender.request(TaggedGrappleMessage::new(id, encode(channel)), 300, 5).await?;
    decode(msg.msg)??;
    Ok(())
  }

  async fn set_adjustable_channel(&self, channel: MitocandriaAdjustableChannelRequest) -> anyhow::Result<()> {
    let id = self.info.read().await.require_device_id()?;
    let (encode, decode) = request_factory!(data, GrappleDeviceMessage::PowerDistributionModule(
      mitocandria::MitocandriaMessage::ChannelRequest(mitocandria::MitocandriaChannelRequest::SetAdjustableChannel(data))
    ));

    let msg = self.sender.request(TaggedGrappleMessage::new(id, encode(channel)), 300, 5).await?;
    decode(msg.msg)??;
    Ok(())
  }

  async fn grapple(&self, msg: GrappleDeviceRequest) -> anyhow::Result<GrappleDeviceResponse> {
    self.grapple_device.rpc_process(msg).await
  }

  async fn status(&self) -> anyhow::Result<MitocandriaStatus> {
    Ok(self.status.read().await.clone())
  }

  async fn check_for_new_firmware(&self) -> anyhow::Result<Option<LightReleaseResponse>> {
    check_for_new_firmware_release_rpc_target::<Self>(&self.info).await
  }
}
