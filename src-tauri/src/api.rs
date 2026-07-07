use crate::models::{BuildChannel, CloudBuild};
use serde::Deserialize;
use reqwest::header::USER_AGENT;

#[derive(Deserialize)]
struct JenkinsResponse {
    #[serde(rename = "allBuilds")]
    all_builds: Vec<JenkinsBuild>,
}

#[derive(Deserialize)]
struct JenkinsBuild {
    number: u32,
    url: String,
    timestamp: i64,
    artifacts: Vec<JenkinsArtifact>,
}

#[derive(Deserialize)]
struct JenkinsArtifact {
    #[serde(rename = "relativePath")]
    relative_path: String,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    published_at: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

pub async fn fetch_dev_builds() -> Result<Vec<CloudBuild>, String> {
    let url = "https://jenkins.kirkstall.top-cat.me/view/All/job/ChroMapper/api/json?tree=allBuilds[number,url,timestamp,artifacts[fileName,relativePath]]";
    let client = reqwest::Client::new();
    
    let res = client.get(url)
        .header(USER_AGENT, "ChroMapper-Version-Manager")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<JenkinsResponse>()
        .await
        .map_err(|e| e.to_string())?;

    let mut builds = Vec::new();
    for b in res.all_builds {
        let is_compiled = !b.artifacts.is_empty();
        
        let download_url = if is_compiled {
            Some(format!("{}artifact/{}", b.url, b.artifacts[0].relative_path))
        } else {
            None
        };

        let datetime = chrono::DateTime::from_timestamp(b.timestamp / 1000, 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        builds.push(CloudBuild {
            id: b.number.to_string(),
            channel: BuildChannel::Dev,
            name: format!("Build #{}", b.number),
            date: datetime,
            download_url,
            is_compiled,
        });
    }

    Ok(builds)
}

pub async fn fetch_stable_builds() -> Result<Vec<CloudBuild>, String> {
    let url = "https://api.github.com/repos/Caeden117/ChroMapper/releases";
    let client = reqwest::Client::new();

    let res = client.get(url)
        .header(USER_AGENT, "ChroMapper-Version-Manager")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<GitHubRelease>>()
        .await
        .map_err(|e| e.to_string())?;

    let mut builds = Vec::new();
    for release in res {
        let download_url = release.assets.first().map(|a| a.browser_download_url.clone());
        
        let datetime = chrono::DateTime::parse_from_rfc3339(&release.published_at)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        builds.push(CloudBuild {
            id: release.tag_name.clone(),
            channel: BuildChannel::Stable,
            name: release.tag_name,
            date: datetime,
            download_url,
            is_compiled: true, 
        });
    }

    Ok(builds)
}