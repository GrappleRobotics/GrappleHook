use std::collections::HashMap;
use std::str;
use std::sync::Arc;

use grapple_frc_msgs::grapple::{TaggedGrappleMessage, GrappleMessageId};
use grapple_frc_msgs::MessageId;
use grapple_frc_msgs::DEVICE_ID_BROADCAST;
use grapple_frc_msgs::grapple::{GrappleDeviceMessage, GrappleBroadcastMessage, device_info::{GrappleDeviceInfo, GrappleModelId}};
use grapple_hook_macros::rpc;
use log::warn;
use serde::{Serialize, Deserialize};
use tokio::sync::{RwLock, mpsc, oneshot};
use uuid::Uuid;

use super::lasercan::LaserCan;
use super::{DeviceType, Device, DeviceInfo, VersionGatedDevice, RootDevice};
// use super::{DeviceInfo, spiderlan::SpiderLAN};
use crate::rpc::RpcBase;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, Hash, PartialEq, Eq)]
pub enum DeviceId {
  Serial(u32),
  CanId(u8)
}

pub type Domain = String;

pub struct DeviceEntry {
  device: Box<dyn RootDevice + Send + Sync>,
  info: Arc<RwLock<DeviceInfo>>,
  last_seen: std::time::Instant
}

pub type RepliesWaiting = Arc<RwLock<HashMap<u32, HashMap<Uuid, oneshot::Sender<TaggedGrappleMessage>>>>>;

pub struct DeviceManager {
  send: HashMap<Domain, mpsc::Sender<TaggedGrappleMessage>>,
  replies_waiting: HashMap<Domain, RepliesWaiting>,
  devices: RwLock<HashMap<Domain, HashMap<DeviceId, DeviceEntry>>>,
}

impl DeviceManager {
  pub fn new(send: HashMap<Domain, mpsc::Sender<TaggedGrappleMessage>>) -> Self {
    let mut devices = HashMap::new();
    let mut replies_waiting = HashMap::new();

    for domain in send.keys() {
      devices.insert(domain.clone(), HashMap::new());
      replies_waiting.insert(domain.clone(), Arc::new(RwLock::new(HashMap::new())));
    }

    Self { send, devices: RwLock::new(devices), replies_waiting }
  }

  pub async fn reset(&self) {
    for (_, devices) in self.devices.write().await.iter_mut() {
      devices.clear();
    }
  }
  
  async fn on_enumerate_response(&self, domain: &String, info: DeviceInfo) -> anyhow::Result<()> {
    let id = DeviceId::Serial(info.serial.unwrap());
    let now = std::time::Instant::now();

    let mut dev_map = self.devices.write().await;
    let devices = dev_map.get_mut(domain).unwrap();
    if !devices.contains_key(&id) {
      let device_type = info.device_type.clone();
      let info_arc = Arc::new(RwLock::new(info));

      let send = super::SendWrapper(self.send.get(domain).unwrap().clone(), self.replies_waiting.get(domain).unwrap().clone());

      let device = match device_type {
        DeviceType::Grapple(GrappleModelId::LaserCan) => LaserCan::maybe_gate(send, info_arc.clone(), LaserCan::new).await,
        _ => unreachable!()
      };

      devices.insert(id, DeviceEntry { device, info: info_arc, last_seen: now });
    } else {
      let deventry = devices.get_mut(&id).unwrap();
      *deventry.info.write().await = info;
      deventry.last_seen = now;
    }
    Ok(())
  }

  // async fn maybe_add_device(&self, domain: &String, id: &DeviceId, info: DeviceInfo, device: Box<dyn Device + Send + Sync>) -> anyhow::Result<()> {
  //   let now = std::time::Instant::now();

  //   let mut dev_map = self.devices.write().await;
  //   let devices = dev_map.get_mut(domain).unwrap();
  //   if !devices.contains_key(id) {
  //     devices.insert(id.clone(), DeviceEntry { device, info: Arc::new(RwLock::new(info)), last_seen: now });
  //   } else {
  //     let deventry = devices.get_mut(id).unwrap();
  //     *deventry.info.write().await = info;
  //     deventry.last_seen = now;
  //   }
  //   Ok(())
  // }

  pub async fn on_message(&self, domain: String, id: GrappleMessageId, message: TaggedGrappleMessage) -> anyhow::Result<()> {
    let msg_id_u32: u32 = Into::<MessageId>::into(id).into();

    let waiting = self.replies_waiting.get(&domain).unwrap();
    if waiting.read().await.contains_key(&msg_id_u32) {
      let mut w = waiting.write().await;
      for (_, waiting_element) in w.remove(&msg_id_u32).unwrap() {
        waiting_element.send(message.clone()).ok();   // ok since it's fine if the channel is closed, e.g. timeouts.
      }
    }

    match message.msg.clone() {
      GrappleDeviceMessage::Broadcast(GrappleBroadcastMessage::DeviceInfo(dinfo)) => match dinfo {
        GrappleDeviceInfo::EnumerateResponse { model_id, serial, is_dfu, is_dfu_in_progress, name, version } => {
          self.on_enumerate_response(&domain, DeviceInfo {
            device_type: DeviceType::Grapple(model_id),
            firmware_version: Some(version),
            serial: Some(serial),
            is_dfu,
            is_dfu_in_progress,
            name: Some(name),
            device_id: Some(message.device_id)
          }).await?;
        },
        _ => ()
      }
      _ => (),
    }
    
    for (_, device) in self.devices.read().await.get(&domain).unwrap().iter() {
      match device.device.handle(message.clone()).await {
        Ok(()) => (),
        Err(e) => warn!("Error in message handler: {}", e)
      }
    }

    Ok(())
  }

  pub async fn on_tick(&self) -> anyhow::Result<()> {
    for (_domain, send) in self.send.iter() {
      send.send(TaggedGrappleMessage::new(DEVICE_ID_BROADCAST, GrappleDeviceMessage::Broadcast(GrappleBroadcastMessage::DeviceInfo(GrappleDeviceInfo::EnumerateRequest)))).await?;
    }

    // Check age off
    let mut dev_map = self.devices.write().await;
    for (_domain, devices) in dev_map.iter_mut() {
      devices.retain(|_, device| {
        device.last_seen.elapsed().as_secs() < 4
      });
    }

    Ok(())
  }
}

#[rpc]
impl DeviceManager {
  async fn call(&self, domain: Domain, device_id: DeviceId, data: serde_json::Value) -> anyhow::Result<serde_json::Value> {
    Ok(self.devices.read().await
      .get(&domain)
      .unwrap()
      .get(&device_id)
      .ok_or(anyhow::anyhow!("No device with ID {:?}", device_id))?
      .device
      .rpc_call(data).await?)
  }

  async fn devices(&self) -> anyhow::Result<HashMap<Domain, Vec<(DeviceId, DeviceInfo, String)>>> {
    let mut device_states = HashMap::new();

    let devices = self.devices.read().await;
    for (domain, devices) in devices.iter() {
      let mut vec = vec![];
      for (id, device) in devices.iter() {
        vec.push((id.clone(), device.info.read().await.clone(), device.device.device_class().to_owned()));
      }
      device_states.insert(domain.clone(), vec);
    }

    Ok(device_states)
  }
}
