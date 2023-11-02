use std::collections::HashMap;
use std::fs::File;
use std::{fs, io};
use std::ffi::OsStr;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use actix_web::{HttpResponse, Scope, web};
use serde_json::json;
use uuid::Uuid;
use crate::api::api_model::{AppState, FileDownloadRequest, PlaylistRequest, ServerConfig, ServerInputConfig, ServerSourceConfig, ServerTargetConfig};
use crate::download::{get_m3u_playlist, get_xtream_playlist};
use crate::model::config::{ConfigInput, InputType, validate_targets};
use crate::utils::{bytes_to_megabytes};
use futures::stream::TryStreamExt;
use log::{error, info};
use regex::Regex;
use reqwest::{header, Response};
use reqwest::header::{HeaderName, HeaderValue};
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
    let process_targets = validate_targets(&Some(targets), &_app_state.config.sources);
    match process_targets {
        Ok(valid_targets) => {
            exec_processing(_app_state.config.clone(), Arc::new(valid_targets));
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
                    InputType::M3u => get_m3u_playlist(&_app_state.config, &input, &_app_state.config.working_dir),
                    InputType::Xtream => get_xtream_playlist(&input, &_app_state.config.working_dir),
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

async fn async_download_file(download_id: &String, path: &Path, response: Response, downloads: Arc<Mutex<HashMap<String, u64>>>) -> Result<u64, String> {
    match File::create(path) {
        Ok(mut file) => {
            info!("Downloading {}", path.to_str().unwrap_or("?"));
            let mut stream = response.bytes_stream().map_err(|err| io::Error::new(ErrorKind::Other, err));
            let mut downloaded: u64 = 0;
            downloads.lock().unwrap().insert(download_id.to_owned(), downloaded);
            loop {
                match stream.try_next().await {
                    Ok(item) => {
                        match item {
                            Some(chunk) => {
                                match file.write_all(&chunk) {
                                    Ok(_) => {
                                        let new = downloaded + (chunk.len() as u64);
                                        downloaded = new;
                                        downloads.lock().unwrap().insert(download_id.to_owned(), downloaded);
                                    }
                                    Err(err) => return Err(format!("Error while writing to file: {} {}", path.to_str().unwrap_or("?"), err))
                                }
                            }
                            None => {
                                let megabytes = bytes_to_megabytes(downloaded);
                                info!("Downloaded {}, filesize: {}MB", path.to_str().unwrap_or("?"), megabytes);
                                return Ok(downloaded);
                            }
                        }
                    }
                    Err(err) => return Err(format!("Error while writing to file: {} {}", path.to_str().unwrap_or("?"), err))
                }
            }
        }
        Err(err) => Err(format!("Error while writing to file: {} {}", path.to_str().unwrap_or("?"), err))
    }
}

pub(crate) async fn download_file_info(
    info: web::Path<String>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let did: String = info.into_inner();
    match _app_state.downloads.lock().unwrap().get(&did) {
        // @TODO it is only a success when the file remains.
        None => HttpResponse::Ok().json(json!({"download_id":  &did, "finished": true})),
        Some(downloaded) => HttpResponse::Ok().json(json!({"download_id":  &did, "filesize": downloaded}))
    }
}

pub(crate) async fn download_file(
    req: web::Json<FileDownloadRequest>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    if let Some(download) = &_app_state.config.video.as_ref().unwrap().download {
        if download.directory.is_none() {
            return HttpResponse::BadRequest().json(json!({"error": "Server config missing video.download.directory configuration"}));
        }

        match reqwest::Url::parse(&req.url) {
            Ok(_url) => {
                let client = reqwest::Client::new();
                let mut headers = header::HeaderMap::new();
                for (key, value) in &download.headers {
                    headers.insert(
                        HeaderName::from_bytes(key.as_bytes()).unwrap(),
                        HeaderValue::from_bytes(value.as_bytes()).unwrap(),
                    );
                }
                match client.get(_url).headers(headers).send().await {
                    Ok(response) => {
                        let filename_re = Regex::new(r"[^A-Za-z0-9_.-]").unwrap();
                        let filename = filename_re.replace_all(&unidecode(&req.filename).replace(' ', "_"), "").to_string();
                        let file_stem = Path::new(&filename).file_stem().and_then(OsStr::to_str).unwrap_or("");
                        let file_dir: PathBuf = [download.directory.as_ref().unwrap(), file_stem].iter().collect();
                        match fs::create_dir_all(&file_dir) {
                            Ok(_) => {
                                let path = file_dir.join(filename.as_str());
                                let download_id = Uuid::new_v4().to_string();
                                let response_download_id = download_id.clone();
                                actix_rt::spawn(async move {
                                    let downloads = _app_state.downloads.clone();
                                    match async_download_file(&download_id, &path, response, downloads.clone()).await {
                                        Ok(_) => {
                                            downloads.lock().unwrap().remove(&download_id);
                                        }
                                        Err(err) => {
                                            downloads.lock().unwrap().remove(&download_id);
                                            let _ = fs::remove_file(&path);
                                            error!("{}", err);
                                        }
                                    }
                                });
                                HttpResponse::Ok().json(json!({"download_id": response_download_id}))
                            }
                            Err(err) => HttpResponse::InternalServerError().json(json!({"error": format!("{}", err)}))
                        }
                    }
                    Err(err) => HttpResponse::InternalServerError().json(json!({"error": format!("{}", err)})),
                }

                //
                // use rocket::futures::TryStreamExt; // for map_err() call below:
                // let reader = StreamReader::new(response.bytes_stream().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
                // rocket::response::Stream::chunked(reader, 4096)
            }
            Err(_) => HttpResponse::BadRequest().json(json!({"error": "Invalid Arguments"})),
        }
    } else {
        HttpResponse::BadRequest().json(json!({"error": "Server config missing video.download configuration"}))
    }
}

pub(crate) fn v1_api_register() -> Scope {
    web::scope("/api/v1")
        .route("/config", web::get().to(config))
        .route("/config/user", web::post().to(config_api_proxy_user))
        .route("/config/serverinfo", web::post().to(config_api_proxy_server_info))
        .route("/playlist", web::post().to(playlist))
        .route("/playlist/update", web::post().to(playlist_update))
        .route("/file/download", web::post().to(download_file))
        .route("/file/download/{did}", web::get().to(download_file_info))
}
