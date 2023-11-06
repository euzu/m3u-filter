use std::fs::File;
use std::{fs, io};
use std::ffi::OsStr;
use std::io::{ErrorKind, Write};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use actix_web::{HttpResponse, Scope, web};
use serde_json::{json};
use crate::api::api_model::{AppState, DownloadErrorInfo, DownloadQueue, FileDownload, FileDownloadRequest, PlaylistRequest, ServerConfig, ServerInputConfig, ServerSourceConfig, ServerTargetConfig};
use crate::download::{get_m3u_playlist, get_xtream_playlist};
use crate::model::config::{ConfigInput, InputType, validate_targets, VideoDownloadConfig};
use crate::utils::{bytes_to_megabytes, get_request_headers};
use futures::stream::TryStreamExt;
use log::{error, info};
use reqwest::header::HeaderMap;
use unidecode::unidecode;
use crate::config_reader::save_api_proxy;
use crate::m3u_filter_error::M3uFilterError;
use crate::model::api_proxy::{ApiProxyConfig, ServerInfo, TargetUser};
use crate::processing::playlist_processor::exec_processing;

fn save_config_api_proxy(api_proxy: &mut ApiProxyConfig) -> Option<M3uFilterError> {
    match save_api_proxy(api_proxy) {
        Ok(_) => {}
        Err(err) => {
            error!("Failed to save api_proxy.yml {}", err.to_string());
            return Some(err);
        }
    }
    None
}

pub(crate) async fn config_api_proxy_user(
    mut req: web::Json<Vec<TargetUser>>,
    mut _app_state: web::Data<AppState>,
) -> HttpResponse {
    req.0.iter_mut().flat_map(|t| &mut t.credentials).for_each(|c| c.trim());
    if let Some(api_proxy) = _app_state.config._api_proxy.write().unwrap().as_mut() {
        api_proxy.user = req.0;
        if let Some(err) = save_config_api_proxy(api_proxy) {
            return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
        }
    }
    HttpResponse::Ok().finish()
}

pub(crate) async fn config_api_proxy_server_info(
    mut req: web::Json<ServerInfo>,
    mut _app_state: web::Data<AppState>,
) -> HttpResponse {
    if req.0.is_valid() {
        if let Some(api_proxy) = _app_state.config._api_proxy.write().unwrap().as_mut() {
            api_proxy.server = req.0;
            if let Some(err) = save_config_api_proxy(api_proxy) {
                return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
            }
        }
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::BadRequest().json(json!({"error": "Invalid content"}))
    }
}

pub(crate) async fn playlist_update(
    req: web::Json<Vec<String>>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let targets = req.0;
    let user_targets = if targets.is_empty() { None } else { Some(targets) };
    let process_targets = validate_targets(&user_targets, &_app_state.config.sources);
    match process_targets {
        Ok(valid_targets) => {
            exec_processing(Arc::clone(&_app_state.config), Arc::new(valid_targets)).await;
            HttpResponse::Ok().finish()
        }
        Err(err) => {
            error!("Failed playlist update {}", err.to_string());
            HttpResponse::BadRequest().json(json!({"error": err.to_string()}))
        }
    }
}

fn create_config_input_for_url(url: &str) -> ConfigInput {
    ConfigInput {
        id: 0,
        headers: Default::default(),
        input_type: InputType::M3u,
        url: String::from(url),
        epg_url: None,
        username: None,
        password: None,
        persist: None,
        prefix: None,
        suffix: None,
        name: None,
        enabled: true,
    }
}

pub(crate) async fn playlist(
    req: web::Json<PlaylistRequest>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    match match &req.input_id {
        Some(input_id) => {
            _app_state.config.get_input_by_id(input_id)
        }
        None => {
            let url = req.url.as_deref().unwrap_or("");
            Some(create_config_input_for_url(url))
        }
    } {
        None => HttpResponse::BadRequest().json(json!({"error": "Invalid Arguments"})),
        Some(input) => {
            let (result, errors) =
                match input.input_type {
                    InputType::M3u => get_m3u_playlist(&_app_state.config, &input, &_app_state.config.working_dir).await,
                    InputType::Xtream => get_xtream_playlist(&input, &_app_state.config.working_dir).await,
                };
            if result.is_empty() {
                let error_strings: Vec<String> = errors.iter().map(|err| err.to_string()).collect();
                HttpResponse::BadRequest().json(json!({"error": error_strings.join(", ")}))
            } else {
                HttpResponse::Ok().json(result)
            }
        }
    }
}

pub(crate) async fn config(
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let sources: Vec<ServerSourceConfig> = _app_state.config.sources.iter()
        .map(|s| ServerSourceConfig {
            inputs: s.inputs.iter().map(|i| ServerInputConfig {
                id: i.id,
                input_type: i.input_type.clone(),
                url: i.url.clone(),
                username: i.username.clone(),
                password: i.password.clone(),
                persist: i.persist.clone(),
                name: i.name.clone(),
                enabled: i.enabled,
            }).collect(),
            targets: s.targets.iter().map(|t| ServerTargetConfig {
                id: t.id,
                enabled: t.enabled,
                name: t.name.clone(),
                options: t.options.clone(),
                sort: t.sort.clone(),
                filter: t.filter.clone(),
                output: t.output.clone(),
                rename: t.rename.clone(),
                mapping: t.mapping.clone(),
                processing_order: t.processing_order.clone(),
                watch: t.watch.clone(),
            }).collect(),
        }).collect();


    let result = ServerConfig {
        video: _app_state.config.video.clone(),
        sources,
        api_proxy: _app_state.config._api_proxy.read().unwrap().clone(),
    };
    HttpResponse::Ok().json(result)
}

async fn download_file(active: Arc<RwLock<Option<FileDownload>>>, headers: HeaderMap) -> Result<(), String> {
    let client = reqwest::Client::new();
    let file_download = active.read().unwrap().as_ref().unwrap().clone();
    match client.get(file_download.url.clone()).headers(headers).send().await {
        Ok(response) => {
            match fs::create_dir_all(&file_download.file_dir) {
                Ok(_) => {
                    let file_path_str = file_download.file_path.to_str().unwrap_or("?");
                    match File::create(&file_download.file_path) {
                        Ok(mut file) => {
                            info!("Downloading {}", file_download.file_path.to_str().unwrap_or("?"));
                            let mut stream = response.bytes_stream().map_err(|err| io::Error::new(ErrorKind::Other, err));
                            let mut downloaded: u64 = 0;
                            loop {
                                match stream.try_next().await {
                                    Ok(item) => {
                                        match item {
                                            Some(chunk) => {
                                                match file.write_all(&chunk) {
                                                    Ok(_) => {
                                                        downloaded += (chunk.len() as u64);
                                                        active.write().unwrap().as_mut().unwrap().size = downloaded;
                                                    }
                                                    Err(err) => return Err(format!("Error while writing to file: {} {}", file_path_str, err))
                                                }
                                            }
                                            None => {
                                                let megabytes = bytes_to_megabytes(downloaded);
                                                info!("Downloaded {}, filesize: {}MB", file_path_str, megabytes);
                                                active.write().unwrap().as_mut().unwrap().size = downloaded;
                                                return Ok(());
                                            }
                                        }
                                    }
                                    Err(err) => return Err(format!("Error while writing to file: {} {}", file_path_str, err))
                                }
                            }
                        }
                        Err(err) => Err(format!("Error while writing to file: {} {}", file_path_str, err))
                    }
                }
                Err(err) => Err(format!("Error while creating directory to file: {} {}", &file_download.file_dir.to_str().unwrap_or("?"), err))
            }
        }
        Err(err) => Err(format!("Error while opening url: {} {}", &file_download.url, err))
    }
}

pub(crate) async fn download_file_info(
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let error_list: &[DownloadErrorInfo] = &_app_state.downloads.errors.write().unwrap().drain(..)
        .map(|e| DownloadErrorInfo { filename: e.filename, error: e.error.unwrap() }).collect::<Vec<DownloadErrorInfo>>();
    let errors = match serde_json::to_string(error_list) {
        Ok(value) => value,
        Err(_) => "[]".to_string()
    };
    match &*_app_state.downloads.active.read().unwrap() {
        None => HttpResponse::Ok().json(json!({"finished": true, "errors": errors})),
        Some(file_download) =>
            HttpResponse::Ok().json(json!({"filename":  file_download.filename, "filesize": file_download.size, "errors": errors}))
    }
}

fn run_download_queue(download_cfg: &VideoDownloadConfig, download_queue: Arc<DownloadQueue>) {
    if let Some(file_download) = download_queue.as_ref().queue.lock().unwrap().pop() {
        *download_queue.as_ref().active.write().unwrap() = Some(file_download);
        let headers = get_request_headers(&download_cfg.headers);
        let dq = Arc::clone(&download_queue);
        actix_rt::spawn(async move {
            loop {
                let opt: Option<FileDownload> = {
                    dq.active.read().unwrap().deref().clone()
                };
                match opt {
                    Some(_) => {
                        match download_file(Arc::clone(&dq.active), headers.clone()).await {
                            Ok(_) => {
                                *dq.active.write().unwrap() = dq.queue.lock().unwrap().pop();
                            }
                            Err(err) => {
                                if let Some(fd) = &mut *dq.active.write().unwrap() {
                                    fd.error = Some(err);
                                    dq.errors.write().unwrap().push(fd.clone());
                                }
                                *dq.active.write().unwrap() = dq.queue.lock().unwrap().pop();
                            }
                        }
                    }
                    None => {
                        return;
                    }
                }
            }
        });
    }
}


pub(crate) async fn queue_download_file(
    req: web::Json<FileDownloadRequest>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    if let Some(download) = &_app_state.config.video.as_ref().unwrap().download {
        if download.directory.is_none() {
            return HttpResponse::BadRequest().json(json!({"error": "Server config missing video.download.directory configuration"}));
        }

        match reqwest::Url::parse(&req.url) {
            Ok(_url) => {
                let filename_re = download._re_filename.as_ref().unwrap();
                let filename = filename_re.replace_all(&unidecode(&req.filename).replace(' ', "_"), "").to_string();
                let file_name = filename.clone();
                let file_dir = get_download_directory(download, &filename);
                let mut file_path: PathBuf = file_dir.clone();
                file_path.push(&filename);
                let file_download = FileDownload {
                    file_dir,
                    file_path,
                    filename,
                    url: _url,
                    size: 0,
                    error: None,
                };
                _app_state.downloads.queue.lock().unwrap().push(file_download);
                if _app_state.downloads.active.read().unwrap().is_none() {
                    run_download_queue(download, Arc::clone(&_app_state.downloads));
                }
                HttpResponse::Ok().json(json!({"success": file_name}))
            }
            Err(_) => HttpResponse::BadRequest().json(json!({"error": "Invalid Arguments"})),
        }
    } else {
        HttpResponse::BadRequest().json(json!({"error": "Server config missing video.download configuration"}))
    }
}

fn get_download_directory(download: &VideoDownloadConfig, filename: &String) -> PathBuf {
    if download.organize_into_directories {
        let mut file_stem = Path::new(&filename).file_stem().and_then(OsStr::to_str).unwrap_or("");
        if let Some(re) = &download._re_episode_pattern {
            if let Some(captures) = re.captures(file_stem) {
                if let Some(episode) = captures.name("episode") {
                    if !episode.as_str().is_empty() {
                        file_stem = &file_stem[..episode.start()];
                    }
                }
            }
        }
        let re_ending = download._re_remove_filename_ending.as_ref().unwrap();
        let dir_name = re_ending.replace(file_stem, "");
        let file_dir: PathBuf = [download.directory.as_ref().unwrap(), dir_name.as_ref()].iter().collect();
        file_dir
    } else {
        PathBuf::from(download.directory.as_ref().unwrap())
    }
}

pub(crate) fn v1_api_register() -> Scope {
    web::scope("/api/v1")
        .route("/config", web::get().to(config))
        .route("/config/user", web::post().to(config_api_proxy_user))
        .route("/config/serverinfo", web::post().to(config_api_proxy_server_info))
        .route("/playlist", web::post().to(playlist))
        .route("/playlist/update", web::post().to(playlist_update))
        .route("/file/download", web::post().to(queue_download_file))
        .route("/file/download/info", web::get().to(download_file_info))
}
