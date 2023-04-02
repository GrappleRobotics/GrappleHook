// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

// use devices::device_manager::DeviceManager;
use env_logger::Builder;
use grapple_hook::{devices::provider_manager::ProviderManager, rpc::RpcBase};

#[tauri::command]
async fn provider_manager_rpc(msg: serde_json::Value, manager: tauri::State<'_, Arc<ProviderManager>>) -> Result<serde_json::Value, String> {
  // manager.rpc(serde_json::from_value(msg).map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())
  manager.rpc_call(msg).await.map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() {
  Builder::new().filter_level(log::LevelFilter::Info).init();

  let provider_manager = Arc::new(ProviderManager::new().await);
  let provider_manager2 = provider_manager.clone();

  tauri::async_runtime::set(tokio::runtime::Handle::current());
  
  tauri::Builder::default()
    .manage(provider_manager.clone())
    .setup(|app| {
      // let handle = app.handle();

      // TODO: Entire ProviderManager is getting locked because of the RPC call. Need to let it happen in the background as a tokio task.
      // RPC call should be non-blocking...

      tauri::async_runtime::spawn(async move {
        provider_manager2.run().await.unwrap()
      });
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![provider_manager_rpc])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
