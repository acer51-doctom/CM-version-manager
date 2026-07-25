use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::logger;

#[derive(Debug, Clone)]
pub struct InstalledBuild {
    pub name: String,
    pub path: PathBuf,
    pub executable_path: PathBuf,
}

/// Scans the <install_dir>/versions directory for installed ChroMapper builds
pub fn scan_installed_builds(base_dir: &str) -> Vec<InstalledBuild> {
    logger::info(format!("Scanning for installed builds in '{base_dir}'..."));
    let versions_dir = PathBuf::from(base_dir).join("versions");
    let mut installed = Vec::new();

    if !versions_dir.exists() {
        let _ = fs::create_dir_all(&versions_dir);
        return installed;
    }

    if let Ok(entries) = fs::read_dir(&versions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(exe_path) = find_executable(&path) {
                    let folder_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown Version")
                        .to_string();

                    installed.push(InstalledBuild {
                        name: folder_name,
                        path,
                        executable_path: exe_path,
                    });
                }
            }
        }
    }

    logger::info(format!("Found {} installed build(s).", installed.len()));
    // Sort alphabetically/by version name
    installed.sort_by(|a, b| b.name.cmp(&a.name));
    installed
}

/// Finds the executable inside a build folder depending on the OS
fn find_executable(build_dir: &Path) -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        let direct_exe = build_dir.join("ChroMapper.exe");
        if direct_exe.exists() {
            return Some(direct_exe);
        }
        // Check subfolder if unzipped with a root directory
        if let Ok(entries) = fs::read_dir(build_dir) {
            for entry in entries.flatten() {
                let sub_exe = entry.path().join("ChroMapper.exe");
                if sub_exe.exists() {
                    return Some(sub_exe);
                }
            }
        }
    } else if cfg!(target_os = "macos") {
        let app_bundle = build_dir.join("ChroMapper.app");
        if app_bundle.exists() {
            return Some(app_bundle);
        }
        let direct_bin = build_dir.join("ChroMapper");
        if direct_bin.exists() {
            return Some(direct_bin);
        }
    } else {
        // Linux
        let direct_bin = build_dir.join("ChroMapper");
        if direct_bin.exists() {
            return Some(direct_bin);
        }
        let x86_bin = build_dir.join("ChroMapper.x86_64");
        if x86_bin.exists() {
            return Some(x86_bin);
        }
    }

    None
}

/// Kills any active ChroMapper processes on the host OS
pub fn kill_chromapper_process() {
    logger::action("Executing auto-kill on active ChroMapper processes...");
    if cfg!(target_os = "windows") {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "ChroMapper.exe"])
            .output();
    } else {
        let _ = Command::new("pkill")
            .arg("-f")
            .arg("ChroMapper")
            .output();
    }
}

/// Launches ChroMapper, optionally killing running instances first
pub fn launch_build(build: &InstalledBuild, auto_kill: bool) -> Result<(), String> {
    logger::action(format!("User requested launch for build: '{}'", build.name));
    if auto_kill {
        kill_chromapper_process();
    }

    let result = if cfg!(target_os = "windows") {
        Command::new(&build.executable_path)
            .current_dir(&build.path)
            .spawn()
            .map_err(|e| format!("Failed to launch executable: {e}"))
    } else if cfg!(target_os = "macos") {
        if build.executable_path.extension().and_then(|s| s.to_str()) == Some("app") {
            Command::new("open")
                .arg(&build.executable_path)
                .spawn()
                .map_err(|e| format!("Failed to open .app bundle: {e}"))
        } else {
            Command::new(&build.executable_path)
                .current_dir(&build.path)
                .spawn()
                .map_err(|e| format!("Failed to launch binary: {e}"))
        }
    } else {
        // Linux: ensure execution bit is set
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(
                &build.executable_path,
                fs::Permissions::from_mode(0o755),
            );
        }

        Command::new(&build.executable_path)
            .current_dir(&build.path)
            .spawn()
            .map_err(|e| format!("Failed to launch binary: {e}"))
    };

    match &result {
        Ok(_) => logger::info(format!("Successfully spawned process for '{}'", build.name)),
        Err(e) => logger::error(format!("Failed to launch '{}': {e}", build.name)),
    }

    result.map(|_| ())
}

/// Deletes an installed build directory from disk
pub fn delete_build(build: &InstalledBuild) -> Result<(), String> {
    logger::action(format!("User requested deletion of build: '{}'", build.name));
    match fs::remove_dir_all(&build.path) {
        Ok(_) => {
            logger::info(format!("Successfully deleted directory: {:?}", build.path));
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to delete folder: {e}");
            logger::error(&err_msg);
            Err(err_msg)
        }
    }
}