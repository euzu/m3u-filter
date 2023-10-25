use std::collections::HashMap;
use std::fs::File;
use std::{fs, io};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use actix_web::{HttpResponse, Scope, web};
use serde_json::json;
use uuid::Uuid;
use crate::api::api_model::{AppState, FileDownloadRequest, PlaylistRequest, ServerConfig, ServerInputConfig, ServerSourceConfig, ServerTargetConfig};
use crate::download::{get_m3u_playlist, get_xtream_playlist};
use crate::model::config::{ConfigInput, InputType};
use crate::utils::{bytes_to_megabytes};
use futures::stream::TryStreamExt;
use log::{error, info};
use reqwest::{header, Response};
use reqwest::header::{HeaderName, HeaderValue};
use unidecode::unidecode;

const DOWNLOAD_HEADERS: &[(&str, &str)] = &[
    ("Accept", "video/*"),
    ("User-Agent", "AppleTV/tvOS/9.1.1.")
];

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
            Some(ConfigInput {
                id: 0,
                headers: Default::default(),
                input_type: InputType::M3u,
                url: String::from(url),
                username: None,
                password: None,
                persist: None,
                prefix: None,
                suffix: None,
                name: None,
                enabled: true,
            })
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
        sources
    };
    HttpResponse::Ok().json(result)
}

async fn async_download_file(download_id: &String, path: &PathBuf, response: Response, downloads: Arc<Mutex<HashMap<String, u64>>>) -> Result<u64, String> {
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
                                    Err(err) => return Err(format!("Error while writing to file: {}", err))
                                }
                            }
                            None => {
                                let megabytes = bytes_to_megabytes(downloaded);
                                info!("Downloaded {}, filesize: {}MB", path.to_str().unwrap_or("?"), megabytes);
                                return Ok(downloaded)
                            },
                        }
                    }
                    Err(err) => return Err(format!("Error while writing to file: {}", err)),
                }
            }
        }
        Err(err) => Err(format!("Error while writing to file: {}", err))
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
                for (key, value) in DOWNLOAD_HEADERS {
                    headers.insert(
                        HeaderName::from_bytes(key.as_bytes()).unwrap(),
                        HeaderValue::from_bytes(value.as_bytes()).unwrap(),
                    );
                }
                match client.get(_url).headers(headers).send().await {
                    Ok(response) => {
                        let filename = unidecode(&req.filename).replace(' ', "_");
                        let path: PathBuf = [(download.directory.clone().unwrap().as_str()), filename.as_str()].iter().collect();
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
        .route("/playlist", web::post().to(playlist))
        .route("/file/download", web::post().to(download_file))
        .route("/file/download/{did}", web::get().to(download_file_info))
}
