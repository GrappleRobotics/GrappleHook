use std::{path::Path, fs, env};

use grapple_hook::devices::{provider_manager::{ProviderManagerRequest, ProviderManagerResponse}, lasercan::{LaserCanRequest, LaserCanResponse}, OldVersionDeviceRequest, OldVersionDeviceResponse};

#[derive(schemars::JsonSchema)]
#[allow(unused)]
struct MegaSchema {
  provider_manager_req: ProviderManagerRequest,
  provider_manager_rsp: ProviderManagerResponse,
  
  // spiderlan_req: SpiderLanRequest,
  // spiderlan_rsp: SpiderLanResponse
  old_version_req: OldVersionDeviceRequest,
  old_version_rsp: OldVersionDeviceResponse,

  lasercan_req: LaserCanRequest,
  lasercan_rsp: LaserCanResponse,
}

fn main() -> anyhow::Result<()> {
  let args: Vec<String> = env::args().collect();
  let file = Path::new(args.get(1).expect("No path provided"));
  let schema = schemars::schema_for!(MegaSchema);

  fs::write(file, serde_json::to_string_pretty(&schema)?)?;
  Ok(())
}