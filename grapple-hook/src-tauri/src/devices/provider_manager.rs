use std::{collections::HashMap, time::Duration};

use grapple_frc_msgs::grapple::{udp::GrappleUDPMessage, device_info::GrappleModelId};
use grapple_hook_macros::{rpc_provider, rpc};
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use futures_util::StreamExt;
use log::{error, info, warn};
use serde::{Serialize, Deserialize};
use tokio::{sync::RwLock, net::UdpSocket};
use tokio_util::udp::UdpFramed;

use super::{provider::{DeviceProvider, ProviderInfo, WrappedDeviceProvider, WrappedDeviceProviderRequest, WrappedDeviceProviderResponse}, roborio::daemon::RoboRioDaemon};
use crate::{rpc::RpcBase, codecs::udp::GrappleUdpCodec};

pub struct ProviderContainer {
  provider: WrappedDeviceProvider,
  is_autodetect: bool,
  last_autodetect: std::time::Instant
}

pub struct ProviderManager {
  providers: RwLock<HashMap<String, ProviderContainer>>,
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
    }
  }

  pub fn interfaces() -> Vec<NetworkInterface> {
    NetworkInterface::show().unwrap_or(vec![])
  }

  pub async fn detect_devices(&self) -> anyhow::Result<()> {
    let now = std::time::Instant::now();
    let mut providers = self.providers.write().await;

    // USB
    // for port in SpiderLanUsb::spiderlan_ports() {
    //   if providers.contains_key(&port) {
    //     providers.get_mut(&port).unwrap().last_autodetect = now;
    //   } else {
    //     providers.insert(port.clone(), ProviderContainer {
    //       provider: WrappedDeviceProvider::new(Box::new(SpiderLanUsb::new(port))),
    //       is_autodetect: true,
    //       last_autodetect: now,
    //     });
    //   }
    // }

    // Age off old detections
    let mut is_connected = HashMap::new();
    for (k, v) in providers.iter() {
      is_connected.insert(k.clone(), v.provider.info().await?.connected);
    }
    providers.retain(|k, v| *is_connected.get(k).unwrap() || !v.is_autodetect || v.last_autodetect.elapsed().as_secs() < 1);
    Ok(())
  }

  pub async fn run(&self) -> anyhow::Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:7171").await?;
    
    // Join multicast groups to receive SpiderLan UDP advertisement
    for iface in Self::interfaces() {
      for addr in iface.addr {
        match addr.ip() {
          std::net::IpAddr::V4(v4) if v4.is_private() => {
            match sock.join_multicast_v4("224.0.0.71".parse()?, v4) {
              Ok(_) => info!("Joined multicast on {}", v4),
              Err(e) => warn!("Could not join Multicast on iface addr {} ({})", v4, e)
            }
          },
          _ => ()
        };
      }
    }

    let mut framed = UdpFramed::new(sock, GrappleUdpCodec {});

    let mut detect_interval = tokio::time::interval(Duration::from_millis(500));

    loop {
      tokio::select! {
        msg = framed.next() => match msg {
          Some(Ok((msg, addr))) => match msg {
            GrappleUDPMessage::Discover(device_type) => {
              let addr = format!("{}", addr.ip());

              let now = std::time::Instant::now();
              let mut providers = self.providers.write().await;

              if providers.contains_key(&addr) {
                providers.get_mut(&addr).unwrap().last_autodetect = now;
              } else {
                info!("Don't know what provider to use for Grapple Device: {:?}", device_type)
              }

              // Age off is handled in detect_devices()
            }
          },
          Some(Err(e)) => error!("UDP Decode Error: {}", e),
          None => (),
        },
        _ = detect_interval.tick() => {
          self.detect_devices().await?;
        }
      }
    }
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
    let mut map = HashMap::new();
    let providers = self.providers.read().await;
    for (address, provider) in providers.iter() {
      map.insert(address.clone(), provider.provider.info().await?);
    }
    Ok(map)
  }
}
