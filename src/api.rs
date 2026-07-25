use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread;
use zip::ZipArchive;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channel {
    Stable,
    Dev,
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Channel::Stable => write!(f, "Stable"),
            Channel::Dev => write!(f, "Dev"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Build {
    pub id: String,
    pub version: String,
    pub channel: Channel,
    pub release_date: String,
    pub download_url: String,
    pub file_name: String,
    pub changelog: String,
}

#[derive(Debug)]
pub enum FetchResult {
    Success(Vec<Build>),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum InstallProgress {
    Started,
    Downloading { downloaded: u64, total: Option<u64> },
    Extracting,
    Finished(PathBuf),
    Failed(String),
}

// GitHub API models
#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    name: Option<String>,
    published_at: Option<String>,
    body: Option<String>,
    prerelease: bool,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// Spawns a background thread to fetch available builds from GitHub Releases.
pub fn fetch_builds_async(sender: Sender<FetchResult>) {
    thread::spawn(move || {
        let result = fetch_builds_sync();
        let _ = sender.send(result);
    });
}

fn fetch_builds_sync() -> FetchResult {
    let client = match reqwest::blocking::Client::builder()
        .user_agent("ChroMapper-Version-Manager/1.0")
        .build()
    {
        Ok(c) => c,
        Err(e) => return FetchResult::Error(format!("Failed to initialize HTTP client: {e}")),
    };

    let url = "https://api.github.com/repos/Caeden117/ChroMapper/releases";

    match client.get(url).send() {
        Ok(res) if res.status().is_success() => {
            match res.json::<Vec<GhRelease>>() {
                Ok(releases) => {
                    let mut builds = Vec::new();

                    for rel in releases {
                        let channel = if rel.prerelease {
                            Channel::Dev
                        } else {
                            Channel::Stable
                        };

                        let date = rel
                            .published_at
                            .as_deref()
                            .and_then(|d| d.split('T').next())
                            .unwrap_or("Unknown")
                            .to_string();

                        if let Some(asset) = find_os_asset(&rel.assets) {
                            builds.push(Build {
                                id: format!("gh-{}", rel.tag_name),
                                version: rel.name.unwrap_or_else(|| rel.tag_name.clone()),
                                channel,
                                release_date: date,
                                download_url: asset.browser_download_url.clone(),
                                file_name: asset.name.clone(),
                                changelog: rel
                                    .body
                                    .unwrap_or_else(|| "No changelog available.".to_string()),
                            });
                        }
                    }

                    FetchResult::Success(builds)
                }
                Err(e) => FetchResult::Error(format!("Failed to parse release JSON: {e}")),
            }
        }
        Ok(res) => FetchResult::Error(format!("GitHub API error: HTTP {}", res.status())),
        Err(e) => FetchResult::Error(format!("Network request failed: {e}")),
    }
}

fn find_os_asset(assets: &[GhAsset]) -> Option<&GhAsset> {
    let target_os = std::env::consts::OS;

    assets
        .iter()
        .find(|a| {
            let lower = a.name.to_lowercase();
            match target_os {
                "windows" => lower.contains("win") || lower.ends_with(".zip"),
                "linux" => lower.contains("linux") || lower.ends_with(".tar.gz") || lower.ends_with(".zip"),
                "macos" => lower.contains("mac") || lower.contains("osx") || lower.ends_with(".dmg") || lower.ends_with(".zip"),
                _ => true,
            }
        })
        .or_else(|| assets.first())
}

/// Spawns background thread to download archive AND automatically extract it.
pub fn install_build_async(url: String, target_dir: PathBuf, sender: Sender<InstallProgress>) {
    thread::spawn(move || {
        let _ = sender.send(InstallProgress::Started);

        let client = match reqwest::blocking::Client::builder()
            .user_agent("ChroMapper-Version-Manager/1.0")
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = sender.send(InstallProgress::Failed(e.to_string()));
                return;
            }
        };

        let mut response = match client.get(&url).send() {
            Ok(res) if res.status().is_success() => res,
            Ok(res) => {
                let _ = sender.send(InstallProgress::Failed(format!("HTTP Error {}", res.status())));
                return;
            }
            Err(e) => {
                let _ = sender.send(InstallProgress::Failed(e.to_string()));
                return;
            }
        };

        let total_size = response.content_length();
        let temp_zip_path = target_dir.with_extension("download.tmp");

        if let Some(parent) = temp_zip_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut file = match File::create(&temp_zip_path) {
            Ok(f) => f,
            Err(e) => {
                let _ = sender.send(InstallProgress::Failed(format!("Disk write error: {e}")));
                return;
            }
        };

        let mut downloaded: u64 = 0;
        let mut buffer = [0u8; 8192];

        // 1. Download
        loop {
            match response.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes_read) => {
                    if let Err(e) = file.write_all(&buffer[..bytes_read]) {
                        let _ = sender.send(InstallProgress::Failed(format!("Write error: {e}")));
                        let _ = std::fs::remove_file(&temp_zip_path);
                        return;
                    }
                    downloaded += bytes_read as u64;
                    let _ = sender.send(InstallProgress::Downloading {
                        downloaded,
                        total: total_size,
                    });
                }
                Err(e) => {
                    let _ = sender.send(InstallProgress::Failed(format!("Read stream error: {e}")));
                    let _ = std::fs::remove_file(&temp_zip_path);
                    return;
                }
            }
        }

        // 2. Extract
        let _ = sender.send(InstallProgress::Extracting);
        if let Err(e) = extract_zip(&temp_zip_path, &target_dir) {
            let _ = sender.send(InstallProgress::Failed(e));
            let _ = std::fs::remove_file(&temp_zip_path);
            return;
        }

        // Cleanup temporary archive file
        let _ = std::fs::remove_file(&temp_zip_path);
        let _ = sender.send(InstallProgress::Finished(target_dir));
    });
}

fn extract_zip(zip_path: &Path, extract_to: &Path) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| format!("Failed to open downloaded archive: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Invalid ZIP archive: {e}"))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| format!("Archive entry read error: {e}"))?;
        let outpath = match file.enclosed_name() {
            Some(path) => extract_to.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath).map_err(|e| format!("Failed to create folder: {e}"))?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p).map_err(|e| format!("Failed to create parent directory: {e}"))?;
                }
            }
            let mut outfile = File::create(&outpath).map_err(|e| format!("Failed to create file: {e}"))?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| format!("Failed to write unzipped file: {e}"))?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_permissions() {
                let _ = std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode));
            }
        }
    }
    Ok(())
}