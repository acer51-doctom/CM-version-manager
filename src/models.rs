use serde::{Deserialize, Serialize};

#[derive(PartialEq)]
pub enum AppTab {
    Versions,
    PluginStore,
    Migration,
    Settings,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum BuildChannel {
    Stable,
    Dev,
}

#[derive(Clone, Debug)]
pub struct CloudBuild {
    pub id: String,
    pub channel: BuildChannel,
    pub name: String,
    pub date: String,
    pub is_compiled: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub github_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub install_directory: String,
    pub auto_kill_chromapper: bool,
    pub dark_mode: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        let default_path = if cfg!(target_os = "windows") {
            dirs::document_dir()
                .unwrap_or_default()
                .join("cm-version-manager")
                .display()
                .to_string()
        } else if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            dirs::home_dir()
                .unwrap_or_default()
                .join("cm-version-manager")
                .display()
                .to_string()
        } else {
            "./cm-version-manager".to_string()
        };

        Self {
            install_directory: default_path,
            auto_kill_chromapper: true,
            dark_mode: true,
        }
    }
}