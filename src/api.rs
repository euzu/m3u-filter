use std::path::PathBuf;
use std::time::Duration;

use actix_cors::Cors;
use actix_files::NamedFile;
use actix_web::{App, get, HttpRequest, HttpResponse, HttpServer, web};
use chrono::{Local};
use cron::Schedule;
use std::str::FromStr;
use actix_web::web::Data;

use crate::model_api::{AppState, PlaylistRequest, ServerConfig};
use crate::config::{Config, ConfigInput, InputType, ProcessTargets};
use crate::{exit, playlist_processor};
use crate::download::get_m3u_playlist;
use crate::xtream_player_api::xtream_player_api;
use log::{error};

#[get("/")]
async fn index(
    _req: HttpRequest,
    _app_state: web::Data<AppState>,
) -> std::io::Result<NamedFile> {
    let path: PathBuf = [_app_state.config.api.web_root.clone(), String::from("index.html")].iter().collect();
    NamedFile::open(path)
}

pub(crate) async fn playlist(
    req: web::Json<PlaylistRequest>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    // TODO refactor this
    let input = ConfigInput {
        id: 0,
        input_type: InputType::M3u,
        headers: Default::default(),
        url: String::from(&req.url),
        username: None,
        password: None,
        persist: None,
        prefix: None,
        suffix: None,
        enabled: true,
    };
    let result = get_m3u_playlist(&_app_state.config, &input, &_app_state.config.working_dir);
    HttpResponse::Ok().json(result)
}

pub(crate) async fn config(
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let sources: Vec<String> = _app_state.config.sources.iter().map(|t| t.input.url.clone()).collect();
    let result = ServerConfig {
        sources
    };
    HttpResponse::Ok().json(result)
}

#[actix_web::main]
pub(crate) async fn start_server(cfg: Config, targets: ProcessTargets) -> futures::io::Result<()> {
    let host = cfg.api.host.clone();
    let port = cfg.api.port;
    let web_dir = cfg.api.web_root.clone();
    let web_dir_path = PathBuf::from(&web_dir);
    if !web_dir_path.exists() || !web_dir_path.is_dir() {
        exit!("web_root does not exists or is not an directory: {:?}", &web_dir_path);
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
        .wrap(Cors::default()
            .supports_credentials()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST", "OPTIONS", "HEAD"])
            .allow_any_header()
            .max_age(3600)
        )
        .app_data(shared_data.clone())
        .service(
            web::scope("/api/v1")
                .route("/playlist", web::post().to(playlist))
                .route("/config", web::get().to(config))
        ).service(xtream_player_api)
        .service(index)
        .service(actix_files::Files::new("/", web_dir.clone()))
    )
        .bind(format!("{}:{}", host, port))?
        .run().await
    //
    // .service(actix_files::Files::new("/static", ".").show_files_listing())
}

async fn start_scheduler(expression: &String, data: Data<AppState>) -> ! {
    let schedule = Schedule::from_str(expression).unwrap();
    let offset = *Local::now().offset();
    loop {
        let mut upcoming = schedule.upcoming(offset).take(1);
        actix_rt::time::sleep(Duration::from_millis(500)).await;
        let local = &Local::now();

        if let Some(datetime) = upcoming.next() {
            if datetime.timestamp() <= local.timestamp() {
                playlist_processor::process_sources((&data.config).clone(), &data.targets);
            }
        }
    }
}