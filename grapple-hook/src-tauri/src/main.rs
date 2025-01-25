// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

// use devices::device_manager::DeviceManager;
use env_logger::Builder;
use grapple_hook::{devices::provider_manager::ProviderManager, rpc::RpcBase, updates::{most_recent_update_available, LightReleaseResponse}};
use tauri::Manager;

static NEW_UPDATE: Mutex<Option<LightReleaseResponse>> = Mutex::new(None);

#[tauri::command]
async fn provider_manager_rpc(msg: serde_json::Value, manager: tauri::State<'_, Arc<ProviderManager>>) -> Result<serde_json::Value, String> {
  // manager.rpc(serde_json::from_value(msg).map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())
  manager.rpc_call(msg).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn is_update_available() -> Result<Option<LightReleaseResponse>, String> {
  Ok(NEW_UPDATE.lock().map_err(|e| e.to_string())?.clone())
}

#[tokio::main]
async fn main() {
  Builder::new().filter_level(log::LevelFilter::Info).init();

  let provider_manager = Arc::new(ProviderManager::new().await);
  let most_recent = most_recent_update_available("https://api.github.com/repos/GrappleRobotics/GrappleHook/releases", |_| true).await;

  if let Err(e) = &most_recent {
    log::warn!("Could not get latest GrappleHook version: {:?}", e)
  }

  let most_recent = most_recent.ok().flatten();

  tauri::async_runtime::set(tokio::runtime::Handle::current());
  
  tauri::Builder::default()
    .manage(provider_manager.clone())
    .setup(|app| {
      log::info!("This version: {}, Most Recent: {:?}", app.package_info().version.to_string(), most_recent.clone().map(|x| x.tag_name));

      if let Some(most_recent) = most_recent {
        if let Ok(vers) = semver::Version::parse(&most_recent.tag_name[1..]) {
          if vers > app.package_info().version {
            let mut update = NEW_UPDATE.lock().unwrap();
            update.replace(most_recent);
          }
        }
      }

      Ok(())
    })
    .invoke_handler(tauri::generate_handler![provider_manager_rpc, is_update_available])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
