use crate::model::config::VideoDownloadConfig;
use crate::utils::request_utils;
use actix_web::{web, HttpResponse};
use futures::stream::TryStreamExt;
use log::info;
use serde_json::{json, Value};
use std::fs::File;
use std::io::{ErrorKind, Write};
use std::ops::Deref;
use std::sync::{Arc};
use async_std::sync::RwLock;
use std::{fs, io};
use crate::api::model::app_state::AppState;
use crate::api::model::download::{DownloadQueue, FileDownload, FileDownloadRequest};

async fn download_file(active: Arc<RwLock<Option<FileDownload>>>, client: &reqwest::Client) -> Result<(), String> {
    let file_download = { active.read().await.as_ref().unwrap().clone() };
    match client.get(file_download.url.clone()).send().await {
        Ok(response) => {
            match fs::create_dir_all(&file_download.file_dir) {
                Ok(()) => {
                    if let Some(file_path_str) = file_download.file_path.to_str() {
                        info!("Downloading {}", file_path_str);
                        match File::create(&file_download.file_path) {
                            Ok(mut file) => {
                                let mut downloaded: u64 = 0;
                                let mut stream = response.bytes_stream().map_err(|err| io::Error::new(ErrorKind::Other, err));
                                loop {
                                    match stream.try_next().await {
                                        Ok(item) => {
                                            if let Some(chunk) = item {
                                                match file.write_all(&chunk) {
                                                    Ok(()) => {
                                                        downloaded += chunk.len() as u64;
                                                        active.write().await.as_mut().unwrap().size = downloaded;
                                                    }
                                                    Err(err) => return Err(format!("Error while writing to file: {file_path_str} {err}"))
                                                }
                                            } else {
                                                let megabytes = request_utils::bytes_to_megabytes(downloaded);
                                                info!("Downloaded {}, filesize: {}MB", file_path_str, megabytes);
                                                active.write().await.as_mut().unwrap().size = downloaded;
                                                return Ok(());
                                            }
                                        }
                                        Err(err) => return Err(format!("Error while writing to file: {file_path_str} {err}"))
                                    }
                                }
                            }
                            Err(err) => Err(format!("Error while writing to file: {file_path_str} {err}"))
                        }
                    } else {
                        Err("Error file-download file-path unknown".to_string())
                    }
                }
                Err(err) => Err(format!("Error while creating directory for file: {} {}", &file_download.file_dir.to_str().unwrap_or("?"), err))
            }
        }
        Err(err) => Err(format!("Error while opening url: {} {}", &file_download.url, err))
    }
}

async fn run_download_queue(download_cfg: &VideoDownloadConfig, download_queue: &Arc<DownloadQueue>) -> Result<(), String> {
    let next_download = download_queue.as_ref().queue.lock().await.pop_front();
    if next_download.is_some() {
        { *download_queue.as_ref().active.write().await = next_download; }
        let headers = request_utils::get_request_headers(Some(&download_cfg.headers), None);
        let dq = Arc::clone(download_queue);
        match reqwest::Client::builder().default_headers(headers).build() {
            Ok(client) => {
                actix_rt::spawn(async move {
                    loop {
                        if dq.active.read().await.deref().is_some() {
                            match download_file(Arc::clone(&dq.active), &client).await {
                                Ok(()) => {
                                    if let Some(fd) = &mut *dq.active.write().await {
                                        fd.finished = true;
                                        dq.finished.write().await.push(fd.clone());
                                    }
                                }
                                Err(err) => {
                                    if let Some(fd) = &mut *dq.active.write().await {
                                        fd.finished = true;
                                        fd.error = Some(err);
                                        dq.finished.write().await.push(fd.clone());
                                    }
                                }
                            }
                            *dq.active.write().await = dq.queue.lock().await.pop_front();
                        } else {
                            break;
                        }
                    }
                });
            }
            Err(_) => return Err("Failed to build http client".to_string()),
        }
    }
    Ok(())
}


macro_rules! download_info {
    ($file_download:expr) => {
       json!({"uuid": $file_download.uuid, "filename":  $file_download.filename,
       "filesize": $file_download.size, "finished": $file_download.finished,
       "error": $file_download.error})
    }
}

pub async fn queue_download_file(
    req: web::Json<FileDownloadRequest>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    if let Some(download_cfg) = &app_state.config.video.as_ref().unwrap().download {
        if download_cfg.directory.is_none() {
            return HttpResponse::BadRequest().json(json!({"error": "Server config missing video.download.directory configuration"}));
        }
        match FileDownload::new(req.url.as_str(), req.filename.as_str(), download_cfg) {
            Some(file_download) => {
                let response = HttpResponse::Ok().json(download_info!(file_download));
                app_state.downloads.queue.lock().await.push_back(file_download);
                if app_state.downloads.active.read().await.is_none() {
                    match run_download_queue(download_cfg, &app_state.downloads).await {
                        Ok(()) => {}
                        Err(err) => return HttpResponse::InternalServerError().json(json!({"error": err})),
                    }
                }
                response
            }
            None => HttpResponse::BadRequest().json(json!({"error": "Invalid Arguments"})),
        }
    } else {
        HttpResponse::BadRequest().json(json!({"error": "Server config missing video.download configuration"}))
    }
}

pub async fn download_file_info(
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let finished_list: &[Value] = &app_state.downloads.finished.write().await.drain(..)
        .map(|fd| download_info!(fd)).collect::<Vec<Value>>();

    (*app_state.downloads.active.read().await).as_ref().map_or_else(|| HttpResponse::Ok().json(json!({
            "completed": true, "downloads": finished_list
        })), |file_download| HttpResponse::Ok().json(json!({
            "completed": false, "downloads": finished_list, "active": download_info!(file_download)
        })))
}