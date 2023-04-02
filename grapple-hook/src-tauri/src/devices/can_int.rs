use grapple_frc_msgs::can::CANMessage;
use serde::Serialize;

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CanIntMessage {
  pub time: u32,
  pub length: u8,
  pub message: CANMessage
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CanIntState {
  pub new_can_messages: Vec<CanIntMessage>
}