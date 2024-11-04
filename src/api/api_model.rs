use std::collections::VecDeque;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use actix_web::web;
use chrono::{Duration, Local};
use serde::{Deserialize, Serialize};
use unidecode::unidecode;

use crate::model::api_proxy::{ApiProxyConfig, ApiProxyServerInfo, ProxyUserCredentials};
use crate::model::config::{Config, ConfigApi, ConfigRename, ConfigSort, ConfigTargetOptions, InputType, MessagingConfig, ProcessTargets, TargetOutput, VideoConfig, VideoDownloadConfig};
use crate::model::config::ProcessingOrder;
use crate::repository::storage::{hash_string_as_hex};

/// File-Download information.
#[derive(Clone)]
pub struct FileDownload {
    /// uuid of the download for identification.
    pub uuid: String,
    /// `file_dir` is the directory where the download should be placed.
    pub file_dir: PathBuf,
    /// `file_path` is the complete path including the filename.
    pub file_path: PathBuf,
    /// filename is the filename.
    pub filename: String,
    /// url is the download url.
    pub url: reqwest::Url,
    /// finished is true, if download is finished, otherweise false
    pub finished: bool,
    /// the filesize.
    pub size: u64,
    /// Optional error if something goes wrong during downloading.
    pub error: Option<String>,
}

/// Returns the directory for th file download.
/// if option `organize_into_directories` is set, the root directory is determined.
/// - For series, the episode pattern is used to determine the sub directory for the series.
/// - For vod files, the title is used to determine the sub directory.
///
/// # Arguments
/// * `download_cfg` the download configuration
/// * `filestem` the prepared filestem to use as sub directory
///
fn get_download_directory(download_cfg: &VideoDownloadConfig, filestem: &str) -> PathBuf {
    if download_cfg.organize_into_directories {
        let mut stem = filestem;
        if let Some(re) = &download_cfg.t_re_episode_pattern {
            if let Some(captures) = re.captures(stem) {
                if let Some(episode) = captures.name("episode") {
                    if !episode.as_str().is_empty() {
                        stem = &stem[..episode.start()];
                    }
                }
            }
        }
        let re_ending = download_cfg.t_re_remove_filename_ending.as_ref().unwrap();
        let dir_name = re_ending.replace(stem, "");
        let file_dir: PathBuf = [download_cfg.directory.as_ref().unwrap(), dir_name.as_ref()].iter().collect();
        file_dir
    } else {
        PathBuf::from(download_cfg.directory.as_ref().unwrap())
    }
}

const FILENAME_TRIM_PATTERNS: &[char] = &['.', '-', '_'];

impl FileDownload {

    // TODO read header size info  and restart support
    // "content-type" => ".../..."
    // "content-length" => "1975828544"
    // "accept-ranges" => "0-1975828544"
    // "content-range" => "bytes 0-1975828543/1975828544"

    pub fn new(req_url: &str, req_filename: &str, download_cfg: &VideoDownloadConfig) -> Option<Self> {
        match reqwest::Url::parse(req_url) {
            Ok(url) => {
                let filename_re = download_cfg.t_re_filename.as_ref().unwrap();
                let tmp_filename = filename_re.replace_all(&unidecode(req_filename)
                    .replace(' ', "_"), "")
                    .replace("__", "_")
                    .replace("_-_", "-");
                let filename_path = Path::new(&tmp_filename);
                let file_stem = filename_path.file_stem().and_then(OsStr::to_str).unwrap_or("").trim_matches(FILENAME_TRIM_PATTERNS);
                let file_ext = filename_path.extension().and_then(OsStr::to_str).unwrap_or("");

                let mut filename = format!("{file_stem}.{file_ext}");
                let file_dir = get_download_directory(download_cfg, file_stem);
                let mut file_path: PathBuf = file_dir.clone();
                file_path.push(&filename);
                let mut x: usize = 1;
                while file_path.is_file() {
                    filename = format!("{file_stem}_{x}.{file_ext}");
                    file_path.clone_from(&file_dir);
                    file_path.push(&filename);
                    x += 1;
                }

                file_path.to_str()?;

                Some(Self {
                    uuid: hash_string_as_hex(req_url),
                    file_dir,
                    file_path,
                    filename,
                    url,
                    finished: false,
                    size: 0,
                    error: None,
                })
            }
            Err(_) => None
        }
    }
}


pub struct DownloadQueue {
    pub queue: Arc<Mutex<VecDeque<FileDownload>>>,
    pub active: Arc<RwLock<Option<FileDownload>>>,
    pub finished: Arc<RwLock<Vec<FileDownload>>>,
}

pub struct AppState {
    pub config: Arc<Config>,
    pub targets: Arc<ProcessTargets>,
    pub downloads: Arc<DownloadQueue>,
}

#[derive(Serialize)]
pub struct XtreamUserInfo {
    pub active_cons: String,
    pub allowed_output_formats: Vec<String>,
    //["ts"],
    pub auth: u16,
    // 0 | 1
    pub created_at: i64,
    //1623429679,
    pub exp_date: i64,
    //1628755200,
    pub is_trial: String,
    // 0 | 1
    pub max_connections: String,
    pub message: String,
    pub password: String,
    pub username: String,
    pub status: String, // "Active"
}

#[derive(Serialize)]
pub struct XtreamServerInfo {
    pub url: String,
    pub port: String,
    pub https_port: String,
    pub server_protocol: String,
    // http, https
    pub rtmp_port: String,
    pub timezone: String,
    pub timestamp_now: i64,
    pub time_now: String, //"2021-06-28 17:07:37"
}

#[derive(Serialize)]
pub struct XtreamAuthorizationResponse {
    pub user_info: XtreamUserInfo,
    pub server_info: XtreamServerInfo,
}

impl XtreamAuthorizationResponse {
    pub fn new(server_info: &ApiProxyServerInfo, user: &ProxyUserCredentials) -> Self {
        let now = Local::now();
        Self {
            user_info: XtreamUserInfo {
                active_cons: "0".to_string(),
                allowed_output_formats: Vec::from(["ts".to_string(), "m3u8".to_string(), "rtmp".to_string()]),
                auth: 1,
                created_at: (now - Duration::days(365)).timestamp(), // fake
                exp_date: (now + Duration::days(365)).timestamp(), // fake
                is_trial: "0".to_string(),
                max_connections: "1".to_string(),
                message: server_info.message.to_string(),
                password: user.password.to_string(),
                username: user.username.to_string(),
                status: "Active".to_string(),
            },
            server_info: XtreamServerInfo {
                url: server_info.host.clone(),
                port: server_info.http_port.clone(),
                https_port: server_info.https_port.clone(),
                server_protocol: server_info.protocol.clone(),
                rtmp_port: server_info.rtmp_port.clone(),
                timezone: server_info.timezone.to_string(),
                timestamp_now: now.timestamp(),
                time_now: now.format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        }
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct UserApiRequest {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub series_id: String,
    #[serde(default)]
    pub vod_id: String,
    #[serde(default)]
    pub stream_id: String,
    #[serde(default)]
    pub category_id: String,
    #[serde(default)]
    pub limit: String,
    #[serde(default)]
    pub start: String,
    #[serde(default)]
    pub end: String,
    #[serde(default)]
    pub stream: String,
    #[serde(default)]
    pub duration: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ServerInputConfig {
    pub id: u16,
    pub input_type: InputType,
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub persist: Option<String>,
    pub name: Option<String>,
    pub enabled: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ServerTargetConfig {
    pub id: u16,
    pub enabled: bool,
    pub name: String,
    pub options: Option<ConfigTargetOptions>,
    pub sort: Option<ConfigSort>,
    pub filter: String,
    #[serde(alias = "type")]
    pub output: Vec<TargetOutput>,
    pub rename: Option<Vec<ConfigRename>>,
    pub mapping: Option<Vec<String>>,
    pub processing_order: ProcessingOrder,
    pub watch: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ServerSourceConfig {
    pub inputs: Vec<ServerInputConfig>,
    pub targets: Vec<ServerTargetConfig>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ServerConfig {
    pub api: ConfigApi,
    pub threads: u8,
    pub working_dir: String,
    pub backup_dir: Option<String>,
    pub schedule: Option<String>,
    pub sources: Vec<ServerSourceConfig>,
    pub messaging: Option<MessagingConfig>,
    pub video: Option<VideoConfig>,
    pub api_proxy: Option<ApiProxyConfig>,
}


#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PlaylistRequest {
    pub url: Option<String>,
    pub input_id: Option<u16>,
}

impl From<web::Json<Self>> for PlaylistRequest {
    fn from(req: web::Json<Self>) -> Self {
        req.clone()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct FileDownloadRequest {
    pub url: String,
    pub filename: String,
}

impl From<web::Json<Self>> for FileDownloadRequest {
    fn from(req: web::Json<Self>) -> Self {
        req.clone()
    }
}

