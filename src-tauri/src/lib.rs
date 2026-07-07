pub mod models;
pub mod api;
pub mod utils;

use models::CloudBuild;
use std::path::PathBuf;

#[tauri::command]
async fn get_cloud_builds(channel_type: String) -> Result<Vec<CloudBuild>, String> {
    if channel_type == "dev" {
        api::fetch_dev_builds().await
    } else {
        api::fetch_stable_builds().await
    }
}

#[tauri::command]
fn open_diagnostic_log() -> Result<(), String> {
    let path = utils::get_unity_log_path();
    if path.exists() {
        // Platform specific default file openers
        #[cfg(target_os = "windows")] {
            std::process::Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?;
        }
        #[cfg(target_os = "macos")] {
            std::process::Command::new("open").arg(&path).spawn().map_err(|e| e.to_string())?;
        }
        #[cfg(target_os = "linux")] {
            std::process::Command::new("xdg-open").arg(&path).spawn().map_err(|e| e.to_string())?;
        }
        Ok(())
    } else {
        Err("Active system log structure could not be identified.".into())
    }
}

#[tauri::command]
fn wipe_editor_cache() -> Result<String, String> {
    utils::clear_unity_cache()
        .map(|_| "Temporary Unity workspace files successfully cleared.".to_string())
        .map_err(|e| format!("Failed to safely purge operational workspace: {}", e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_cloud_builds,
            open_diagnostic_log,
            wipe_editor_cache
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}