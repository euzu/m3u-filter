use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::web::Data;
use actix_web::{web, App, HttpResponse, HttpServer};
use async_std::sync::{Mutex, RwLock};
use log::info;
use std::collections::{HashMap, VecDeque};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;

use crate::api::m3u_api::m3u_api_register;
use crate::api::model::app_state::AppState;
use crate::api::model::download::DownloadQueue;
use crate::api::scheduler::start_scheduler;
use crate::api::v1_api::v1_api_register;
use crate::api::web_index::index_register;
use crate::api::xmltv_api::xmltv_api_register;
use crate::api::xtream_api::xtream_api_register;
use crate::model::config::{validate_targets, Config, ProcessTargets, ScheduleConfig};
use crate::model::healthcheck::Healthcheck;
use crate::processing::playlist_processor;
use crate::VERSION;

fn get_web_dir_path(web_ui_enabled: bool, web_root: &str) -> Result<PathBuf, std::io::Error> {
    let web_dir = web_root.to_string();
    let web_dir_path = PathBuf::from(&web_dir);
    if web_ui_enabled && (!&web_dir_path.exists() || !&web_dir_path.is_dir()) {
        return Err(std::io::Error::new(ErrorKind::NotFound,
                                       format!("web_root does not exists or is not an directory: {:?}", &web_dir_path)));
    };
    Ok(web_dir_path)
}

async fn healthcheck() -> HttpResponse {
    let ts = chrono::offset::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    HttpResponse::Ok().json(Healthcheck {
        status: "ok".to_string(),
        version: VERSION.to_string(),
        time: ts,
    })
}

fn create_shared_data(cfg: &Arc<Config>) -> Data<AppState> {
    Data::new(AppState {
        config: Arc::clone(cfg),
        downloads: Arc::from(DownloadQueue {
            queue: Arc::from(Mutex::new(VecDeque::new())),
            active: Arc::from(RwLock::new(None)),
            finished: Arc::from(RwLock::new(Vec::new())),
        }),
        shared_streams: Arc::new(Mutex::new(HashMap::new())),
    })
}

fn exec_update_on_boot(cfg: &Arc<Config>, targets: &Arc<ProcessTargets>) {
    if cfg.update_on_boot {
        let cfg_clone = Arc::clone(cfg);
        let targets_clone = Arc::clone(targets);
        actix_rt::spawn(
            async move { playlist_processor::exec_processing(cfg_clone, targets_clone).await }
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

fn exec_scheduler(cfg: &Arc<Config>, targets: &Arc<ProcessTargets>) {
    let schedules: Vec<ScheduleConfig> = if let Some(schedules) = &cfg.schedules {
        schedules.clone()
    } else {
        vec![]
    };
    for schedule in schedules {
        let expression = schedule.schedule.to_string();
        let exec_targets = get_process_targets(cfg, targets, schedule.targets.as_ref());
        let cfg_clone = Arc::clone(cfg);
        actix_rt::spawn(async move {
            start_scheduler(expression.as_str(), cfg_clone, exec_targets).await;
        });
    }
}

fn is_web_auth_enabled(cfg: &Arc<Config>, web_ui_enabled: bool, web_dir_path: &PathBuf) -> bool {
    if web_ui_enabled {
        info!("Web root: {:?}", &web_dir_path);
        if let Some(web_auth) = &cfg.web_auth {
            return web_auth.enabled;
        }
    }
    false
}

#[actix_web::main]
pub async fn start_server(cfg: Arc<Config>, targets: Arc<ProcessTargets>) -> futures::io::Result<()> {
    let host = cfg.api.host.to_string();
    let port = cfg.api.port;
    let web_ui_enabled = cfg.web_ui_enabled;
    let web_dir_path = match get_web_dir_path(web_ui_enabled, cfg.api.web_root.as_str()) {
        Ok(result) => result,
        Err(err) => return Err(err)
    };

    exec_scheduler(&cfg, &targets);
    exec_update_on_boot(&cfg, &targets);
    let web_auth_enabled = is_web_auth_enabled(&cfg, web_ui_enabled, &web_dir_path);

    let shared_data = create_shared_data(&cfg);
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
            })
            .configure(xtream_api_register)
            .configure(m3u_api_register)
            .configure(xmltv_api_register)
            .configure(|srvcfg| {
                if web_ui_enabled {
                    srvcfg.configure(index_register(&web_dir_path));
                }
            })
    }).bind(format!("{host}:{port}"))?.run().await
}
