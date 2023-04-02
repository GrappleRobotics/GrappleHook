use grapple_frc_msgs::{ni::{NiRioHearbeat1, NiDeviceMessage, NiRobotControllerMessage, NiRioHeartbeat}, Message, ManufacturerMessage};
use serde::Serialize;
use tokio::sync::{RwLock, mpsc};

use crate::{rpc::{RpcResult, to_rpc_result}, device_rpc_impl};

use super::{DeviceInfo, BasicDevice, Capability, Device};

#[derive(Serialize, schemars::JsonSchema)]
pub struct RoboRIORpcState {
  last_heartbeat: NiRioHearbeat1
}

pub struct RoboRIO {
  info: RwLock<DeviceInfo>,
  last_heartbeat: RwLock<NiRioHearbeat1>
}

impl RoboRIO {
  pub fn new(can_id: u8, heartbeat: NiRioHearbeat1) -> Self {
    Self {
      last_heartbeat: RwLock::new(heartbeat),
      info: RwLock::new(DeviceInfo {
        device_type: super::DeviceType::RoboRIO,
        firmware_version: None,
        serial: None,
        is_dfu: false,
        is_dfu_in_progress: false,
        name: None,
        can_id: Some(can_id),
      })
    }
  }
}

#[async_trait::async_trait]
impl BasicDevice for RoboRIO {
  fn capabilities(&self) -> Vec<Capability> {
    vec![]
  }

  async fn info(&self) -> DeviceInfo {
    self.info.read().await.clone()
  }

  async fn set_info(&self, info: DeviceInfo) {
    // This never gets called since the RoboRIO doesn't receive enumeration responses
    *self.info.write().await = info;
  }

  async fn send_now(&self, _: Message) -> anyhow::Result<()> {
    anyhow::bail!("Can't send messages to the RoboRIO!")
  }

  async fn handle_msg(&self, msg: Message) -> anyhow::Result<()> {
    if Some(msg.device_id) == self.info().await.can_id {
      match msg.msg {
        ManufacturerMessage::Ni(NiDeviceMessage::RobotController(msg)) => match msg {
          NiRobotControllerMessage::Heartbeat(NiRioHeartbeat::Hearbeat(msg)) => {
            *self.last_heartbeat.write().await = msg;
          }
        },
        _ => ()
      }
    }
    Ok(())
  }

  async fn rpc_specific(&self, _: serde_json::Value) -> RpcResult {
    to_rpc_result(())
  }

  async fn state_specific(&self) -> serde_json::Value {
    serde_json::to_value(RoboRIORpcState {
      last_heartbeat: self.last_heartbeat.read().await.clone()
    }).unwrap()
  }
}

#[async_trait::async_trait]
impl Device for RoboRIO {
  async fn handle(&self, message: Message) -> anyhow::Result<()> {
    BasicDevice::handle_msg(self, message).await
  }
}

device_rpc_impl!(RoboRIO);