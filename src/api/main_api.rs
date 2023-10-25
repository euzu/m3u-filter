use std::io::ErrorKind;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use actix_cors::Cors;
use actix_files::NamedFile;
use actix_web::{App, get, HttpRequest, HttpServer, web};
use actix_web::middleware::Logger;
use actix_web::web::Data;
use chrono::Local;
use cron::Schedule;
use crate::api::m3u_api::m3u_api;

use crate::api::api_model::{AppState};
use crate::api::v1_api::{v1_api_register};
use crate::api::xtream_player_api::xtream_player_api;
use crate::model::config::{Config,ProcessTargets};
use crate::processing::playlist_processor::exec_processing;

#[get("/")]
async fn index(
    _req: HttpRequest,
    _app_state: web::Data<AppState>,
) -> std::io::Result<NamedFile> {
    let path: PathBuf = [&_app_state.config.api.web_root, "index.html"].iter().collect();
    NamedFile::open(path)
}


#[actix_web::main]
pub(crate) async fn start_server(cfg: Arc<Config>, targets: Arc<ProcessTargets>) -> futures::io::Result<()> {
    let host = cfg.api.host.to_string();
    let port = cfg.api.port;
    let web_dir = cfg.api.web_root.to_string();
    let web_dir_path = PathBuf::from(&web_dir);
    if !web_dir_path.exists() || !web_dir_path.is_dir() {
        return Err(std::io::Error::new(ErrorKind::NotFound, format!("web_root does not exists or is not an directory: {:?}", &web_dir_path)));
    }

    let schedule = cfg.schedule.clone();

    let shared_data = web::Data::new(AppState {
        config: cfg,
        targets,
    });

    // Scheduler
    if let Some(expression) = schedule {
        let cloned_data = shared_data.clone();
        actix_rt::spawn(async move {
            start_scheduler(&expression, cloned_data).await
        });
    }

    // Web Server
    HttpServer::new(move || App::new()
        .wrap(Logger::default())
        .wrap(Cors::default()
            .supports_credentials()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST", "OPTIONS", "HEAD"])
            .allow_any_header()
            .max_age(3600)
        )
        .app_data(shared_data.clone())
        .service(v1_api_register())
        .service(xtream_player_api)
        .service(m3u_api)
        .service(index)
        .service(actix_files::Files::new("/", web_dir.to_string()))
    )
        .bind(format!("{}:{}", host, port))?
        .run().await
    //
    // .service(actix_files::Files::new("/static", ".").show_files_listing())
}

async fn start_scheduler(expression: &str, data: Data<AppState>) -> ! {
    let schedule = Schedule::from_str(expression).unwrap();
    let offset = *Local::now().offset();
    loop {
        let mut upcoming = schedule.upcoming(offset).take(1);
        actix_rt::time::sleep(Duration::from_millis(500)).await;
        let local = &Local::now();

        if let Some(datetime) = upcoming.next() {
            if datetime.timestamp() <= local.timestamp() {
                exec_processing(data.config.clone(), data.targets.clone());
            }
        }
    }
}