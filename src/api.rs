use std::path::PathBuf;

use actix_cors::Cors;
use actix_files::NamedFile;
use actix_web::{App, get, HttpRequest, HttpResponse, HttpServer, web};

use crate::api_model::{AppState, PlaylistRequest, ServerConfig};
use crate::config::Config;
use crate::service::get_playlist;

#[get("/")]
async fn index(
    _req: HttpRequest,
    _app_state: web::Data<AppState>,
) -> std::io::Result<NamedFile> {
    let path: PathBuf = [_app_state.config.api.web_root.clone(), String::from("index.html")].iter().collect();
    Ok(NamedFile::open(path)?)
}

pub(crate) async fn playlist(
    req: web::Json<PlaylistRequest>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let result = get_playlist(&req.url.as_str(), None);
    HttpResponse::Ok().json(result)
}

pub(crate) async fn config(
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let sources = vec![_app_state.config.input.url.clone()];
    let result = ServerConfig {
        sources
    };
    HttpResponse::Ok().json(result)
}

#[actix_web::main]
pub(crate) async fn start_server(cfg: Config) -> futures::io::Result<()> {
    let host = cfg.api.host.clone();
    let port = cfg.api.port;
    let web_dir = cfg.api.web_root.clone();

    let shared_data = web::Data::new(AppState {
        config: cfg,
    });
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
        )
        .service(index)
        .service(actix_files::Files::new("/", web_dir.clone()))
    )
        .bind(format!("{}:{}", host, port))?
        .run().await
    //
    // .service(actix_files::Files::new("/static", ".").show_files_listing())
}