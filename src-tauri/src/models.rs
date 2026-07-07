use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum BuildChannel {
    Stable,
    Dev,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CloudBuild {
    pub id: String,                 
    pub channel: BuildChannel,
    pub name: String,               
    pub date: String,               
    pub download_url: Option<String>,
    pub is_compiled: bool,          
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstalledInstance {
    pub id: String,
    pub channel: BuildChannel,
    pub name: String,
    pub path: PathBuf,
    pub auto_update: bool,          
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub global_plugins_path: PathBuf, 
    pub instances_path: PathBuf,      
    pub custom_maps_path: Option<PathBuf>, 
}