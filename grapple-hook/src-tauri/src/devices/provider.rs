use grapple_hook_macros::{rpc_provider, rpc};

use crate::rpc::RpcBase;

use super::device_manager::{DeviceManagerRequest, DeviceManagerResponse};

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProviderInfo {
  pub description: String,
  pub address: String,
  pub connected: bool,
}

#[async_trait::async_trait]
pub trait DeviceProvider {
  async fn connect(&self) -> anyhow::Result<()>;
  async fn disconnect(&self) -> anyhow::Result<()>;
  async fn info(&self) -> anyhow::Result<ProviderInfo>;

  async fn device_manager_call(&self, req: DeviceManagerRequest) -> anyhow::Result<DeviceManagerResponse>;
}

pub struct WrappedDeviceProvider {
  inner: Box<dyn DeviceProvider + Send + Sync>
}

impl WrappedDeviceProvider {
  pub fn new(inner: Box<dyn DeviceProvider + Send + Sync>) -> Self {
    Self { inner }
  }
}

#[rpc]
impl WrappedDeviceProvider {
  pub async fn connect(&self) -> anyhow::Result<()> {
    self.inner.connect().await
  }

  pub async fn disconnect(&self) -> anyhow::Result<()> {
    self.inner.disconnect().await
  }

  pub async fn info(&self) -> anyhow::Result<ProviderInfo> {
    self.inner.info().await
  }

  pub async fn device_manager_call(&self, req: DeviceManagerRequest) -> anyhow::Result<DeviceManagerResponse> {
    self.inner.device_manager_call(req).await
  }
}
