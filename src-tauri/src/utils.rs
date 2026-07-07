use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Platform-agnostic method to clean out the default Unity folder and hook up our universal directory link
pub fn create_instance_plugin_link(instance_dir: &Path, global_plugins_dir: &Path) -> std::io::Result<()> {
    let instance_plugins = instance_dir.join("Plugins");
    
    if instance_plugins.exists() {
        if instance_plugins.is_dir() {
            fs::remove_dir_all(&instance_plugins)?;
        } else {
            fs::remove_file(&instance_plugins)?;
        }
    }

    #[cfg(target_os = "windows")]
    {
        std::os::windows::fs::symlink_dir(global_plugins_dir, instance_plugins)?;
    }
    
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        std::os::unix::fs::symlink(global_plugins_dir, instance_plugins)?;
    }

    Ok(())
}

/// Locates the platform-specific hidden directory path for Unity's runtime log output
pub fn get_unity_log_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    
    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        PathBuf::from(local_app_data).join("LocalLow/BinaryElement/ChroMapper/Player.log")
    }
    
    #[cfg(target_os = "macos")]
    {
        PathBuf::from(home).join("Library/Logs/BinaryElement/ChroMapper/Player.log")
    }
    
    #[cfg(target_os = "linux")]
    {
        let config_home = std::env::var("XDG_CONFIG_HOME").unwrap_or(format!("{}/.config", home));
        PathBuf::from(config_home).join("unity3d/BinaryElement/ChroMapper/Player.log")
    }
}

/// Clears out the temporary caching database files that cause typical startup boot loops
pub fn clear_unity_cache() -> std::io::Result<()> {
    let log_path = get_unity_log_path();
    if let Some(parent_dir) = log_path.parent() {
        let cache_dir = parent_dir.join("Cache");
        if cache_dir.exists() && cache_dir.is_dir() {
            fs::remove_dir_all(cache_dir)?;
        }
    }
    Ok(())
}

/// Dispatches a sub-process execution call to bundle audio conversions cleanly via command-line arguments
pub fn convert_to_vorbis_ogg(ffmpeg_binary_path: &Path, input: &str, output: &str) -> Result<(), String> {
    let status = Command::new(ffmpeg_binary_path)
        .args(&["-i", input, "-c:a", "libvorbis", "-q:a", "6", "-y", output])
        .status()
        .map_err(|e| format!("Failed to execute audio extraction sub-process: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err("Audio compiler returned a non-zero exit validation code.".into())
    }
}