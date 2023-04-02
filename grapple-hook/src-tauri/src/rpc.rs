#[async_trait::async_trait]
pub trait RpcBase {
  async fn rpc_call(&self, data: serde_json::Value) -> anyhow::Result<serde_json::Value>;
}
