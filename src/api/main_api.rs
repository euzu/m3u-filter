use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::web::Data;
use actix_web::{web, App, HttpResponse, HttpServer};
use parking_lot::{Mutex as PlMutex};
use log::{error, info};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use mime::APPLICATION_JSON;
use crate::api::endpoints::hls_api::hls_api_register;
use crate::api::endpoints::m3u_api::m3u_api_register;
use crate::api::model::app_state::{AppState, HdHomerunAppState};
use crate::api::model::download::DownloadQueue;
use crate::api::model::streams::shared_stream_manager::SharedStreamManager;
use crate::api::scheduler::start_scheduler;
use crate::api::endpoints::v1_api::v1_api_register;
use crate::api::endpoints::web_index::index_register;
use crate::api::endpoints::xmltv_api::xmltv_api_register;
use crate::api::endpoints::xtream_api::xtream_api_register;
use crate::api::model::active_user_manager::ActiveUserManager;
use crate::model::config::{validate_targets, Config, ProcessTargets, ScheduleConfig};
use crate::model::healthcheck::Healthcheck;
use crate::processing::processor::playlist;
use crate::tools::lru_cache::{LRUResourceCache};
use crate::utils::size_utils::human_readable_byte_size;
use crate::utils::sys_utils;
use crate::{BUILD_TIMESTAMP, VERSION};
use crate::api::endpoints::hdhomerun_api::{hdhr_api_register};
use crate::api::model::active_provider_manager::ActiveProviderManager;

fn get_web_dir_path(web_ui_enabled: bool, web_root: &str) -> Result<PathBuf, std::io::Error> {
    let web_dir = web_root.to_string();
    let web_dir_path = PathBuf::from(&web_dir);
    if web_ui_enabled && (!&web_dir_path.exists() || !&web_dir_path.is_dir()) {
        return Err(std::io::Error::new(ErrorKind::NotFound,
                                       format!("web_root does not exists or is not an directory: {:?}", &web_dir_path)));
    };
    Ok(web_dir_path)
}

fn create_healthcheck(app_state: &web::Data<Arc<AppState>>) -> Healthcheck {
    let server_time = chrono::offset::Local::now().with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S %Z").to_string();
    let cache = app_state.cache.as_ref().as_ref().map(|c| c.lock().get_size_text());
    let (active_clients, active_connections) =  {
        let active_user = &app_state.active_users;
        (active_user.active_users(), active_user.active_connections())
    };
    let build_time: Option<String> = BUILD_TIMESTAMP.to_string().parse::<DateTime<Utc>>().ok().map(|datetime| datetime.format("%Y-%m-%d %H:%M:%S %Z").to_string());
    Healthcheck {
        status: "ok".to_string(),
        version: VERSION.to_string(),
        build_time,
        server_time,
        memory: sys_utils::get_memory_usage().map_or(String::from("?"), human_readable_byte_size),
        active_clients,
        active_connections,
        cache,
    }
}

async fn healthcheck(app_state: web::Data<Arc<AppState>>,) -> HttpResponse {
    HttpResponse::Ok().json(create_healthcheck(&app_state))
}

async fn status(app_state: web::Data<Arc<AppState>>,) -> HttpResponse {
    let status = create_healthcheck(&app_state);
    match serde_json::to_string_pretty(&status) {
        Ok(pretty_json) => HttpResponse::Ok().content_type(APPLICATION_JSON).body(pretty_json),
        Err(_) => HttpResponse::Ok().json(status),
    }
}

fn create_shared_data(cfg: &Arc<Config>) -> AppState {
    let lru_cache = cfg.reverse_proxy.as_ref().and_then(|r| r.cache.as_ref()).and_then(|c| if c.enabled  {
        Some(PlMutex::new(LRUResourceCache::new(c.t_size, &PathBuf::from(c.dir.as_ref().unwrap()))))
    } else { None} );
    let cache = Arc::new(lru_cache);
    let cache_scanner = Arc::clone(&cache);
    actix_rt::spawn(async move {
        if let Some(m) = cache_scanner.as_ref() {
            let mut c = m.lock();
            if let Err(err) = (*c).scan() {
                error!("Failed to scan cache {err}");
            }
        }
    });
    let user_access_control = cfg.user_access_control;
    AppState {
        config: Arc::clone(cfg),
        http_client: Arc::new(reqwest::Client::new()),
        downloads: Arc::from(DownloadQueue::new()),
        cache,
        shared_stream_manager: Arc::new(SharedStreamManager::new()),
        active_users: Arc::new(ActiveUserManager::new()),
        active_provider: Arc::new(ActiveProviderManager::new(user_access_control)),
    }
}

fn exec_update_on_boot(client: Arc<reqwest::Client>, cfg: &Arc<Config>, targets: &Arc<ProcessTargets>) {
    if cfg.update_on_boot {
        let cfg_clone = Arc::clone(cfg);
        let targets_clone = Arc::clone(targets);
        actix_rt::spawn(
            async move { playlist::exec_processing(client, cfg_clone, targets_clone).await }
        );
    }
}


fn get_process_targets(cfg: &Arc<Config>, process_targets: &Arc<ProcessTargets>, exec_targets: Option<&Vec<String>>) -> Arc<ProcessTargets> {
    if let Ok(user_targets) = validate_targets(exec_targets, &cfg.sources) {
        if user_targets.enabled {
            if !process_targets.enabled {
                return Arc::new(user_targets);
            }

            let inputs: Vec<u16> = user_targets.inputs.iter()
                .filter(|&id| process_targets.inputs.contains(id))
                .copied()
                .collect();
            let targets: Vec<u16> = user_targets.targets.iter()
                .filter(|&id| process_targets.inputs.contains(id))
                .copied()
                .collect();
            return Arc::new(ProcessTargets {
                enabled: user_targets.enabled,
                inputs,
                targets,
            });
        }
    }
    Arc::clone(process_targets)
}

fn exec_scheduler(client: &Arc<reqwest::Client>, cfg: &Arc<Config>, targets: &Arc<ProcessTargets>) {
    let schedules: Vec<ScheduleConfig> = if let Some(schedules) = &cfg.schedules {
        schedules.clone()
    } else {
        vec![]
    };
    for schedule in schedules {
        let expression = schedule.schedule.to_string();
        let exec_targets = get_process_targets(cfg, targets, schedule.targets.as_ref());
        let cfg_clone = Arc::clone(cfg);
        let http_client = Arc::clone(client);
        actix_rt::spawn(async move {
            start_scheduler(http_client, expression.as_str(), cfg_clone, exec_targets).await;
        });
    }
}

fn is_web_auth_enabled(cfg: &Arc<Config>, web_ui_enabled: bool) -> bool {
    if web_ui_enabled {
        if let Some(web_auth) = &cfg.web_auth {
            return web_auth.enabled;
        }
    }
    false
}

fn start_hdhomerun(cfg: &Arc<Config>, app_state: &Arc<AppState>, infos: &mut Vec<String>) {
    let host = cfg.api.host.to_string();
    if let Some(hdhomerun) = &cfg.hdhomerun {
        if hdhomerun.enabled {
            for device in &hdhomerun.devices {
                if device.t_enabled {
                    let app_data = Arc::clone(app_state);
                    let app_host = host.clone();
                    let port = device.port;
                    let device_clone = Arc::new(device.clone());
                    infos.push(format!("HdHomeRun Server '{}' running: http://{host}:{port}", device.name));
                    actix_rt::spawn(async move {
                        HttpServer::new(move || {
                            App::new()
                                .wrap(Logger::default())
                                .wrap(Cors::default()
                                    .supports_credentials()
                                    .allow_any_origin()
                                    .allowed_methods(vec!["GET", "POST", "OPTIONS", "HEAD"])
                                    .allow_any_header()
                                    .max_age(3600))
                                .app_data(Data::new(HdHomerunAppState {
                                    app_state: Arc::clone(&app_data),
                                    device: Arc::clone(&device_clone),
                                }))
                                .configure(hdhr_api_register)
                        }).bind(format!("{}:{port}", app_host.clone()))?.run().await
                    });
                }
            }
        }
    }
}

#[actix_web::main]
pub async fn start_server(cfg: Arc<Config>, targets: Arc<ProcessTargets>) -> futures::io::Result<()> {
    let mut infos = Vec::new();
    let host = cfg.api.host.to_string();
    let port = cfg.api.port;
    let web_ui_enabled = cfg.web_ui_enabled;
    let web_dir_path = match get_web_dir_path(web_ui_enabled, cfg.api.web_root.as_str()) {
        Ok(result) => result,
        Err(err) => return Err(err)
    };
    if web_ui_enabled {
        infos.push(format!("Web root: {:?}", &web_dir_path));
    }
    let app_state = Arc::new(create_shared_data(&cfg));
    let shared_data = Data::new(Arc::clone(&app_state));

    exec_scheduler(&Arc::clone(&shared_data.http_client), &cfg, &targets);
    exec_update_on_boot(Arc::clone(&shared_data.http_client), &cfg, &targets);
    let web_auth_enabled = is_web_auth_enabled(&cfg, web_ui_enabled);

    if cfg.t_api_proxy.read().is_some() {
        start_hdhomerun(&cfg, &app_state, &mut infos);
    }

    infos.push(format!("Server running: http://{}:{}", &cfg.api.host, &cfg.api.port));
    for info in &infos {
        info!("{info}");
    }
    // Web Server
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Cors::default()
                .supports_credentials()
                .allow_any_origin()
                .allowed_methods(vec!["GET", "POST", "OPTIONS", "HEAD"])
                .allow_any_header()
                .max_age(3600))
            .app_data(shared_data.clone())
            // .wrap(Condition::new(web_auth_enabled, ErrorHandlers::new().handler(StatusCode::UNAUTHORIZED, handle_unauthorized)))
            .configure(|srvcfg| {
                if web_ui_enabled {
                    srvcfg.service(actix_files::Files::new("/static", web_dir_path.join("static")));
                    srvcfg.configure(v1_api_register(web_auth_enabled));
                }
                srvcfg.service(web::resource("/healthcheck").route(web::get().to(healthcheck)));
                srvcfg.service(web::resource("/status").route(web::get().to(status)));
            })
            .configure(xtream_api_register)
            .configure(m3u_api_register)
            .configure(xmltv_api_register)
            .configure(hls_api_register)
            .configure(|srvcfg| {
                if web_ui_enabled {
                    srvcfg.configure(index_register(&web_dir_path));
                }
            })
    }).bind(format!("{host}:{port}"))?.run().await
}
