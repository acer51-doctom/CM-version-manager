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
    Release,
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Channel::Stable => write!(f, "Stable (CDN)"),
            Channel::Dev => write!(f, "Dev (Jenkins)"),
            Channel::Release => write!(f, "GitHub Releases"),
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

// --- Jenkins API Models (Dev) ---
#[derive(Debug, Deserialize)]
struct JenkinsJob {
    builds: Vec<JenkinsBuild>,
}

#[derive(Debug, Deserialize)]
struct JenkinsBuild {
    id: String,
    timestamp: u64, // Jenkins returns milliseconds since epoch
    url: String,
    artifacts: Vec<JenkinsArtifact>,
}

#[derive(Debug, Deserialize)]
struct JenkinsArtifact {
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "relativePath")]
    relative_path: String,
}

// --- GitHub API Models (Releases) ---
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    published_at: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Spawns a background thread to fetch available builds for a specific channel.
pub fn fetch_builds_async(channel: Channel, sender: Sender<FetchResult>) {
    thread::spawn(move || {
        let result = fetch_builds_sync(channel);
        let _ = sender.send(result);
    });
}

fn fetch_builds_sync(channel: Channel) -> FetchResult {
    let client = match reqwest::blocking::Client::builder()
        .user_agent("ChroMapper-Version-Manager")
        .build()
    {
        Ok(c) => c,
        Err(e) => return FetchResult::Error(format!("Failed to initialize HTTP client: {e}")),
    };

    // Route the request based on the requested channel
    match channel {
        Channel::Stable => {
            match fetch_cdn_stable(&client) {
                Ok(builds) => FetchResult::Success(builds),
                Err(e) => FetchResult::Error(format!("CDN fetch failed: {e}")),
            }
        }
        Channel::Dev => {
            match fetch_jenkins_dev(&client) {
                Ok(builds) => FetchResult::Success(builds),
                Err(e) => FetchResult::Error(format!("Jenkins fetch failed: {e}")),
            }
        }
        Channel::Release => {
            match fetch_github_releases(&client) {
                Ok(builds) => FetchResult::Success(builds),
                Err(e) => FetchResult::Error(format!("GitHub releases fetch failed: {e}")),
            }
        }
    }
}

fn fetch_cdn_stable(client: &reqwest::blocking::Client) -> Result<Vec<Build>, String> {
    // 1. Get the latest stable build number from the official CDN
    let stable_url = "https://cm.topc.at/stable";
    let res = client.get(stable_url).send().map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("CDN HTTP Error: {}", res.status()));
    }

    let build_num = res.text().map_err(|e| e.to_string())?.trim().to_string();

    // 2. Figure out OS-specific prefix and asset filename
    let target_os = std::env::consts::OS;
    let (prefix, filename) = match target_os {
        "windows" => ("win", "ChroMapper.zip"),
        "linux" => ("linux", "ChroMapper.tar.gz"),
        "macos" => ("osx", "ChroMapper.zip"),
        _ => return Err(format!("Unsupported OS: {}", target_os)),
    };

    // 3. Construct the direct CDN download URL
    let download_url = format!("https://cm.topc.at/{}/{}/{}", prefix, build_num, filename);

    let build = Build {
        id: format!("cdn-{}", build_num),
        version: format!("Stable Build {}", build_num),
        channel: Channel::Stable,
        release_date: "Latest Stable".to_string(),
        download_url,
        file_name: filename.to_string(),
        changelog: "Fetched directly from the official ChroMapper CDN.".to_string(),
    };

    Ok(vec![build])
}

fn fetch_jenkins_dev(client: &reqwest::blocking::Client) -> Result<Vec<Build>, String> {
    let jenkins_url = "https://jenkins.kirkstall.top-cat.me/job/ChroMapper/api/json?tree=builds[id,timestamp,url,artifacts[fileName,relativePath]]";
    
    let res = client.get(jenkins_url).send().map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("HTTP {}", res.status()));
    }

    let job: JenkinsJob = res.json().map_err(|e| e.to_string())?;
    let mut builds = Vec::new();

    for build in job.builds {
        if let Some(artifact) = find_os_asset_jenkins(&build.artifacts) {
            let date = format!("Timestamp: {}", build.timestamp);

            builds.push(Build {
                id: format!("jenkins-{}", build.id),
                version: format!("Dev Build {}", build.id),
                channel: Channel::Dev,
                release_date: date,
                download_url: format!("{}artifact/{}", build.url, artifact.relative_path),
                file_name: artifact.file_name.clone(),
                changelog: "Check Jenkins for commit history.".to_string(),
            });
        }
    }

    Ok(builds)
}

fn fetch_github_releases(client: &reqwest::blocking::Client) -> Result<Vec<Build>, String> {
    let github_url = "https://api.github.com/repos/rcelyte/ChroMapper/releases";
    
    let res = client.get(github_url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("GitHub API Error: HTTP {}", res.status()));
    }

    let releases: Vec<GitHubRelease> = res.json().map_err(|e| e.to_string())?;
    let mut builds = Vec::new();

    for release in releases {
        if let Some(asset) = find_os_asset_github(&release.assets) {
            let version_name = release.name.unwrap_or_else(|| release.tag_name.clone());
            
            builds.push(Build {
                id: format!("gh-{}", release.tag_name),
                version: version_name,
                channel: Channel::Release,
                release_date: release.published_at,
                download_url: asset.browser_download_url.clone(),
                file_name: asset.name.clone(),
                changelog: release.body.unwrap_or_else(|| "No changelog provided.".to_string()),
            });
        }
    }

    Ok(builds)
}

fn find_os_asset_jenkins(artifacts: &[JenkinsArtifact]) -> Option<&JenkinsArtifact> {
    let target_os = std::env::consts::OS;
    artifacts.iter().find(|a| {
        let lower = a.file_name.to_lowercase();
        match target_os {
            "windows" => lower.contains("win") || lower.ends_with(".zip"),
            "linux" => lower.contains("linux") || lower.ends_with(".tar.gz") || lower.ends_with(".zip"),
            "macos" => lower.contains("mac") || lower.contains("osx") || lower.ends_with(".dmg") || lower.ends_with(".zip"),
            _ => true,
        }
    }).or_else(|| artifacts.first())
}

fn find_os_asset_github(assets: &[GitHubAsset]) -> Option<&GitHubAsset> {
    let target_os = std::env::consts::OS;
    assets.iter().find(|a| {
        let lower = a.name.to_lowercase();
        match target_os {
            "windows" => (lower.contains("win") || lower.contains("windows")) && (lower.ends_with(".zip") || lower.ends_with(".exe")),
            "linux" => lower.contains("linux") && (lower.ends_with(".tar.gz") || lower.ends_with(".zip")),
            "macos" => (lower.contains("mac") || lower.contains("osx") || lower.contains("darwin")) && (lower.ends_with(".zip") || lower.ends_with(".dmg")),
            _ => true,
        }
    }).or_else(|| assets.first())
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