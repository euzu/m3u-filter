use std::collections::VecDeque;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use actix_cors::Cors;
use actix_web::{App, HttpServer, web};
use actix_web::http::StatusCode;
use actix_web::middleware::{ErrorHandlers, Logger};
use log::info;

use crate::api::api_model::{AppState, DownloadQueue, SharedLocks};
use crate::api::m3u_api::m3u_api_register;
use crate::api::scheduler::start_scheduler;
use crate::api::v1_api::v1_api_register;
use crate::api::web_index::index_register;
use crate::api::xmltv_api::xmltv_api_register;
use crate::api::xtream_api::xtream_api_register;
use crate::auth::authenticator::handle_unauthorized;
use crate::model::config::{Config, ProcessTargets};
use crate::processing::playlist_processor;

fn get_web_dir_path(web_ui_enabled: bool, web_root: &str) -> Result<PathBuf, std::io::Error> {
    let web_dir = web_root.to_string();
    let web_dir_path = PathBuf::from(&web_dir);
    if web_ui_enabled {
        if !&web_dir_path.exists() || !&web_dir_path.is_dir() {
            return Err(std::io::Error::new(ErrorKind::NotFound,
                                           format!("web_root does not exists or is not an directory: {:?}", &web_dir_path)));
        }
    };
    Ok(web_dir_path)
}

#[actix_web::main]
pub(crate) async fn start_server(cfg: Arc<Config>, targets: Arc<ProcessTargets>) -> futures::io::Result<()> {
    let host = cfg.api.host.to_string();
    let port = cfg.api.port;
    let web_ui_enabled = cfg.web_ui_enabled;
    let web_dir_path = match get_web_dir_path(web_ui_enabled, cfg.api.web_root.as_str()) {
        Ok(result) => result,
        Err(err) => return Err(err)
    };

    let schedule = cfg.schedule.clone();

    let shared_data = web::Data::new(AppState {
        config: Arc::clone(&cfg),
        targets: Arc::clone(&targets),
        downloads: Arc::from(DownloadQueue {
            queue: Arc::from(Mutex::new(VecDeque::new())),
            active: Arc::from(RwLock::new(None)),
            finished: Arc::from(RwLock::new(Vec::new())),
        }),
        shared_locks: Arc::new(SharedLocks::new()),
    });

    // Scheduler
    if let Some(expression) = schedule {
        let cloned_data = shared_data.clone();
        actix_rt::spawn(async move {
            start_scheduler(&expression, cloned_data).await
        });
    }

    if cfg.update_on_boot {
        let cfg_clone = Arc::clone(&cfg);
        let targets_clone = Arc::clone(&targets);
        actix_rt::spawn(
            async move { playlist_processor::exec_processing(cfg_clone, targets_clone).await }
        );
    }

    if web_ui_enabled {
        info!("Web root: {:?}", &web_dir_path);
    }
    let web_auth_enabled = cfg.web_auth_enabled;

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
            .wrap(ErrorHandlers::new().handler(StatusCode::UNAUTHORIZED, handle_unauthorized))
            .configure(|cfg| {
                if web_ui_enabled {
                    cfg.service(actix_files::Files::new("/static", web_dir_path.join("static")));
                    cfg.configure(v1_api_register(web_auth_enabled));
                }
            })
            .configure(xtream_api_register)
            .configure(m3u_api_register)
            .configure(xmltv_api_register)
            .configure(|cfg| {
                if web_ui_enabled {
                    cfg.configure(index_register(&web_dir_path, web_auth_enabled));
                }
            })
    }).bind(format!("{}:{}", host, port))?.run().await
}

