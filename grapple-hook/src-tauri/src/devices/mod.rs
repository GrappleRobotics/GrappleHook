pub mod device_manager;
pub mod provider;
pub mod provider_manager;
pub mod roborio;
pub mod lasercan;
pub mod flexican;
pub mod mitocandria;
pub mod generic_usb;
// pub mod powerful_panda;

use std::{borrow::Cow, collections::HashMap, io::{Cursor, Read}, marker::PhantomData, sync::Arc, time::Duration};

use bounded_static::IntoBoundedStatic;
use grapple_frc_msgs::{Validate, grapple::{device_info::GrappleModelId, GrappleDeviceMessage, firmware::GrappleFirmwareMessage, TaggedGrappleMessage, GrappleMessageId}, DEVICE_ID_BROADCAST, binmarshal::{MarshalUpdate, AsymmetricCow, Payload}, MessageId};
use grapple_hook_macros::rpc;
use log::info;
use semver::{Version, VersionReq};
use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc, RwLock, Notify, oneshot};
use uuid::Uuid;

use crate::{rpc::RpcBase, updates::LightReleaseResponse};

use self::device_manager::RepliesWaiting;

#[derive(Clone)]
pub struct SendWrapper(mpsc::Sender<TaggedGrappleMessage<'static>>, RepliesWaiting);

impl SendWrapper {
  async fn send(&self, msg: TaggedGrappleMessage<'static>) -> anyhow::Result<()> {
    msg.msg.validate()?;
    self.0.send(msg).await?;
    Ok(())
  }

  async fn request_inner(&self, msg: TaggedGrappleMessage<'static>, reply_id: GrappleMessageId, timeout_ms: usize) -> anyhow::Result<TaggedGrappleMessage> {
    let complement_id_u32: u32 = Into::<MessageId>::into(reply_id).into();

    let uuid = Uuid::new_v4();

    let (tx, rx) = oneshot::channel();
    {
      let mut hm = self.1.write().await;
      if !hm.contains_key(&complement_id_u32) {
        hm.insert(complement_id_u32, HashMap::new());
      }
      hm.get_mut(&complement_id_u32).unwrap().insert(uuid, tx);
      drop(hm);
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

  async fn request(&self, mut msg: TaggedGrappleMessage<'static>, timeout_ms: usize, retry: usize) -> anyhow::Result<TaggedGrappleMessage> {
    let mut id = GrappleMessageId::new(msg.device_id);
    msg.msg.update(&mut id);

    let mut complement_id = id.clone();
    complement_id.ack_flag = true;
    
    match self.request_inner(msg.clone(), complement_id, timeout_ms).await {
      Ok(x) => Ok(x),
      Err(_) if retry >= 1 => Box::pin(self.request(msg, timeout_ms, retry - 1)).await,
      Err(e) => Err(e)
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
  async fn handle(&self, _msg: TaggedGrappleMessage<'static>) -> anyhow::Result<()> { Ok(()) }
}

#[async_trait::async_trait]
pub trait RootDevice : Device {
  fn device_class(&self) -> &'static str;
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
          name: Cow::<str>::Owned(name).into()
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

pub struct FirmwareUpgradeDevice<T: FirmwareValidatingDevice> {
  sender: SendWrapper,
  info: SharedInfo,
  progress: Arc<RwLock<Option<f64>>>,
  ack: Arc<Notify>,
  chunk_size: usize,
  _t: PhantomData<T>
}

impl<T: FirmwareValidatingDevice> FirmwareUpgradeDevice<T> {
  pub fn new(sender: SendWrapper, info: SharedInfo, chunk_size: usize) -> Self {
    Self { sender, info, progress: Arc::new(RwLock::new(None)), ack: Arc::new(Notify::new()), chunk_size, _t: PhantomData }
  }

  pub async fn field_upgrade_worker(sender: SendWrapper, id: u8, data: &[u8], progress: Arc<RwLock<Option<f64>>>, ack: Arc<Notify>, chunk_size: usize) -> anyhow::Result<()> {
    *progress.write().await = Some(0.0);
    let chunks = data.chunks(chunk_size);
    let nchunks = chunks.len();
    for (i, chunk) in chunks.enumerate() {
      info!("Chunk {} (len: {})", i, chunk.len());

      let mut padded = vec![0u8; chunk_size];

      for i in 0..chunk.len() {
        padded[i] = chunk[i];
      }

      sender.send(TaggedGrappleMessage::new(
        id,
        GrappleDeviceMessage::FirmwareUpdate(
          GrappleFirmwareMessage::UpdatePart(AsymmetricCow(Cow::<Payload>::Borrowed(Into::into(&padded[..]))).into_static())
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

pub async fn start_field_upgrade(sender: &SendWrapper, serial: u32) -> anyhow::Result<()> {
  sender.send(TaggedGrappleMessage::new(
    DEVICE_ID_BROADCAST,
    GrappleDeviceMessage::FirmwareUpdate(
      GrappleFirmwareMessage::StartFieldUpgrade { serial }
    )
  )).await
}

fn maybe_unpack_firmware(data: &[u8]) -> anyhow::Result<Vec<u8>> {
  let mut archive = zip::ZipArchive::new(Cursor::new(data))?;

  // TODO: Use https://github.com/GrappleRobotics/bundle/tree/master/grapple-bundle-lib

  let mut index_file = archive.by_name("index")?;
  let mut index_content = String::new();
  index_file.read_to_string(&mut index_content)?;
  let index: serde_json::Value = serde_json::from_str(&index_content)?;
  drop(index_file);

  let mut firmware_f = archive.by_name(index.get("firmware_update_bin").map(|x| x.as_str()).flatten().ok_or(anyhow::anyhow!("Invalid Index"))?)?;
  let mut v = vec![];
  firmware_f.read_to_end(&mut v)?;
  Ok(v)
}

#[rpc]
impl<T: FirmwareValidatingDevice + HasFirmwareUpdateURLDevice + Send + Sync> FirmwareUpgradeDevice<T> {
  async fn do_field_upgrade(&self, data: Vec<u8>) -> anyhow::Result<()> {
    let buf = match maybe_unpack_firmware(&data) {
      Ok(buf) => buf,
      Err(_) => {
        let d = data;
        <T>::validate_firmware(&*self.info.read().await, &d).map_err(|e| anyhow::anyhow!("Not a valid firmware file: {}", e))?;
        d
      }
    };

    let sender = self.sender.clone();
    let progress = self.progress.clone();
    let id = self.info.read().await.require_device_id()?;
    let notify = self.ack.clone();
    let chunk_size = self.chunk_size;

    tokio::task::spawn(async move {
      let d = buf;
      Self::field_upgrade_worker(sender, id, &d[..], progress, notify, chunk_size).await.ok();
    });
    Ok(())
  }

  async fn progress(&self) -> anyhow::Result<Option<f64>> {
    Ok(self.progress.read().await.clone())
  }

  async fn get_firmware_url(&self) -> anyhow::Result<Option<String>> {
    Ok(T::firmware_url())
  }
}

impl<T: FirmwareValidatingDevice + HasFirmwareUpdateURLDevice + Send + Sync> RootDevice for FirmwareUpgradeDevice<T> {
  fn device_class(&self) ->  &'static str {
    "GrappleFirmwareUpgrade"
  }
}

#[async_trait::async_trait]
impl<T: FirmwareValidatingDevice + HasFirmwareUpdateURLDevice + Send + Sync> Device for FirmwareUpgradeDevice<T> {
  async fn handle(&self, msg: TaggedGrappleMessage<'static>) -> anyhow::Result<()> {
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

pub trait HasFirmwareUpdateURLDevice {
  fn firmware_url() -> Option<String>;
}

pub trait FirmwareValidatingDevice {
  fn validate_firmware(info: &DeviceInfo, buf: &[u8]) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
pub trait VersionGatedDevice : HasFirmwareUpdateURLDevice + RootDevice + Sized + Sync + 'static {
  fn validate_version(version: Option<String>) -> anyhow::Result<()>;
  async fn check_for_new_firmware_release(current_version: &str) -> Option<LightReleaseResponse>;

  fn require_version(version: Option<String>, req: &str) -> anyhow::Result<()> {
    if let Some(v) = version {
      let v = Version::parse(&v)?;
      if !VersionReq::parse(req)?.matches(&v) {
        anyhow::bail!("Invalid version: {}, expected: {}", v, req);
      }
    }
    Ok(())
  }

  async fn maybe_gate<F: FnOnce(SendWrapper, Arc<RwLock<DeviceInfo>>) -> Self + Send>(send: SendWrapper, info: Arc<RwLock<DeviceInfo>>, create_fn: F) -> Box<dyn RootDevice + Send + Sync + 'static> {
    match Self::validate_version(info.clone().read().await.firmware_version.clone()) {
      Ok(_) => Box::new(create_fn(send, info)),
      Err(e) => Box::new(OldVersionDevice::new(send, info, format!("{}", e), Self::firmware_url()))
    }
  }
}

async fn check_for_new_firmware_release_rpc_target<T: VersionGatedDevice>(info: &SharedInfo) -> anyhow::Result<Option<LightReleaseResponse>> {
  let fw = &info.read().await.firmware_version;
  match fw {
    Some(x) => Ok(T::check_for_new_firmware_release(&x).await),
    None => anyhow::bail!("Can't check for new releases - device has no version")
  }
}

pub struct OldVersionDevice {
  grapple_device: GrappleDevice,
  error: String,
  firmware_url: Option<String>
}

impl OldVersionDevice {
  pub fn new(sender: SendWrapper, info: SharedInfo, error: String, firmware_url: Option<String>) -> Self {
    Self {
      grapple_device: GrappleDevice::new(sender.clone(), info.clone()),
      error, firmware_url
    }
  }
}

#[async_trait::async_trait]
impl Device for OldVersionDevice {
  async fn handle(&self, msg: TaggedGrappleMessage<'static>) -> anyhow::Result<()> {
    self.grapple_device.handle(msg.clone()).await?;
    Ok(())
  }
}

#[async_trait::async_trait]
impl RootDevice for OldVersionDevice {
  fn device_class(&self) -> &'static str {
    "OldVersionDevice"
  }
}

#[rpc]
impl OldVersionDevice {
  async fn start_field_upgrade(&self) -> anyhow::Result<()> {
    let serial = self.grapple_device.info.read().await.require_serial()?;
    start_field_upgrade(&self.grapple_device.sender, serial).await
  }

  async fn get_error(&self) -> anyhow::Result<String> {
    Ok(self.error.clone())
  }

  async fn get_firmware_url(&self) -> anyhow::Result<Option<String>> {
    Ok(self.firmware_url.clone())
  }

  async fn grapple(&self, msg: GrappleDeviceRequest) -> anyhow::Result<GrappleDeviceResponse> {
    self.grapple_device.rpc_process(msg).await
  }
}
