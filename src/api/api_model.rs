use std::collections::{VecDeque};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use actix_web::web;
use serde::{Deserialize, Serialize};
use unidecode::unidecode;
use crate::model::api_proxy::{ApiProxyConfig};
use crate::model::config::{Config, ConfigTargetOptions, ConfigRename, ConfigSort, InputType, ProcessTargets, TargetOutput, VideoConfig, VideoDownloadConfig, ConfigApi, MessagingConfig};
use crate::model::config::{ProcessingOrder};
use crate::utils::default_utils::default_as_empty_str;

/// File-Download information.
#[derive(Clone)]
pub(crate) struct FileDownload {
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

    pub fn new(req_url: &str, req_filename: &str, download_cfg: &VideoDownloadConfig) -> Option<FileDownload> {
        match reqwest::Url::parse(req_url) {
            Ok(url) => {
                let filename_re = download_cfg.t_re_filename.as_ref().unwrap();
                let tmp_filename = filename_re.replace_all(&unidecode(req_filename)
                    .replace(' ', "_"), "")
                    .replace("__", "_")
                    .replace("_-_", "-").to_string();
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

                Some(FileDownload {
                    uuid: uuid::Uuid::new_v4().to_string(),
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


pub(crate) struct DownloadQueue {
    pub queue: Arc<Mutex<VecDeque<FileDownload>>>,
    pub active: Arc<RwLock<Option<FileDownload>>>,
    pub finished: Arc<RwLock<Vec<FileDownload>>>,
}

pub(crate) struct AppState {
    pub config: Arc<Config>,
    pub targets: Arc<ProcessTargets>,
    pub downloads: Arc<DownloadQueue>,
}

#[derive(Serialize)]
pub(crate) struct XtreamUserInfo {
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
pub(crate) struct XtreamServerInfo {
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
pub(crate) struct XtreamAuthorizationResponse {
    pub user_info: XtreamUserInfo,
    pub server_info: XtreamServerInfo,
}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct UserApiRequest {
    #[serde(default = "default_as_empty_str")]
    pub username: String,
    #[serde(default = "default_as_empty_str")]
    pub password: String,
    #[serde(default = "default_as_empty_str")]
    pub token: String,
    #[serde(default = "default_as_empty_str")]
    pub action: String,
    #[serde(default = "default_as_empty_str")]
    pub series_id: String,
    #[serde(default = "default_as_empty_str")]
    pub vod_id: String,
    #[serde(default = "default_as_empty_str")]
    pub stream_id: String,
    #[serde(default = "default_as_empty_str")]
    pub category_id: String,
    #[serde(default = "default_as_empty_str")]
    pub limit: String,
    #[serde(default = "default_as_empty_str")]
    pub start: String,
    #[serde(default = "default_as_empty_str")]
    pub end: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct ServerInputConfig {
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
pub(crate) struct ServerTargetConfig {
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
pub(crate) struct ServerSourceConfig {
    pub inputs: Vec<ServerInputConfig>,
    pub targets: Vec<ServerTargetConfig>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct ServerConfig {
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
pub(crate) struct PlaylistRequest {
    pub url: Option<String>,
    pub input_id: Option<u16>,
}

impl From<web::Json<PlaylistRequest>> for PlaylistRequest {
    fn from(req: web::Json<PlaylistRequest>) -> Self {
        req.clone()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct FileDownloadRequest {
    pub url: String,
    pub filename: String,
}

impl From<web::Json<FileDownloadRequest>> for FileDownloadRequest {
    fn from(req: web::Json<FileDownloadRequest>) -> Self {
        req.clone()
    }
}

