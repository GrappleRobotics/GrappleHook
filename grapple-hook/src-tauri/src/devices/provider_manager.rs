use std::collections::HashMap;

use grapple_hook_macros::rpc;
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use tokio::sync::RwLock;


use super::{generic_usb::GenericUSB, provider::{DeviceProvider, ProviderInfo, WrappedDeviceProvider, WrappedDeviceProviderRequest, WrappedDeviceProviderResponse}, roborio::daemon::RoboRioDaemon};
use crate::rpc::RpcBase;

pub struct ProviderContainer {
  provider: WrappedDeviceProvider,
  is_autodetect: bool,
  last_autodetect: std::time::Instant
}

pub struct ProviderManager {
  providers: RwLock<HashMap<String, ProviderContainer>>,
  last_detect: RwLock<std::time::Instant>,
}

impl ProviderManager {
  pub async fn new() -> Self {
    let mut hm = HashMap::new();
    let rr = RoboRioDaemon::new();
    hm.insert(rr.info().await.unwrap().address, ProviderContainer { 
      provider: WrappedDeviceProvider::new(Box::new(rr)),
      is_autodetect: false,
      last_autodetect: std::time::Instant::now()
    });
    Self {
      providers: RwLock::new(hm),
      last_detect: RwLock::new(std::time::Instant::now())
    }
  }

  pub fn interfaces() -> Vec<NetworkInterface> {
    NetworkInterface::show().unwrap_or(vec![])
  }

  pub async fn detect_devices(&self) -> anyhow::Result<()> {
    let mut providers: tokio::sync::RwLockWriteGuard<HashMap<String, ProviderContainer>> = self.providers.write().await;

    // Look for USB devices
    let now = std::time::Instant::now();
    if self.last_detect.read().await.elapsed().as_millis() > 500 {

      if let Ok(ports) = tokio_serial::available_ports() {
        for port in ports {
          match port.port_type {
            tokio_serial::SerialPortType::UsbPort(usbi) => {
              if usbi.vid == 0x16c0 && usbi.pid == 0x27dd {
                let addr = port.port_name;
                if !providers.contains_key(&addr) {
                  providers.insert(addr.clone(), ProviderContainer {
                    provider: WrappedDeviceProvider::new(Box::new(GenericUSB::new(addr))),
                    is_autodetect: true,
                    last_autodetect: now
                  });
                } else {
                  providers.get_mut(&addr).unwrap().last_autodetect = now;
                }
              }
            },
            _ => ()
          }
        }
      }

      *self.last_detect.write().await = now;
    }

    // Age off old detections
    let mut is_connected = HashMap::new();
    for (k, v) in providers.iter() {
      is_connected.insert(k.clone(), v.provider.info().await?.connected);
    }
    providers.retain(|k, v| *is_connected.get(k).unwrap() || !v.is_autodetect || v.last_autodetect.elapsed().as_secs() < 2);
    Ok(())
  }
}

#[rpc]
impl ProviderManager {
  async fn delete(&self, address: String) -> anyhow::Result<()> {
    if self.providers.read().await.contains_key(&address) {
      let v = self.providers.write().await.remove(&address);
      if let Some(provider) = v {
        if provider.provider.info().await?.connected {
          provider.provider.disconnect().await?;
        }
      }
    }
    Ok(())
  }

  async fn provider(&self, address: String, msg: WrappedDeviceProviderRequest) -> anyhow::Result<WrappedDeviceProviderResponse> {
    self.providers.read().await.get(&address).unwrap().provider.rpc_process(msg).await
  }

  async fn providers(&self) -> anyhow::Result<HashMap<String, ProviderInfo>> {
    self.detect_devices().await.ok();

    let mut map = HashMap::new();
    let providers = self.providers.read().await;
    for (address, provider) in providers.iter() {
      map.insert(address.clone(), provider.provider.info().await?);
    }
    Ok(map)
  }
}
