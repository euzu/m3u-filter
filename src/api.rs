use std::path::PathBuf;

use actix_cors::Cors;
use actix_files::NamedFile;
use actix_web::{App, get, HttpRequest, HttpResponse, HttpServer, web};

use crate::api_model::{AppState, PlaylistRequest, ServerConfig};
use crate::config::{Config, ConfigInput, InputType};
use crate::service::get_m3u_playlist;

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
    // TODO refactor this
    let input = ConfigInput {
        input_type: InputType::M3u,
        headers: Default::default(),
        url: String::from(&req.url),
        username: "".to_string(),
        password: "".to_string(),
        persist: "".to_string(),
        prefix: None,
        suffix: None,
        enabled: true,
    };
    let result = get_m3u_playlist(&input, &_app_state.config.working_dir, false);
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
pub(crate) async fn start_server(cfg: Config) -> futures::io::Result<()> {
    let host = cfg.api.host.clone();
    let port = cfg.api.port;
    let web_dir = cfg.api.web_root.clone();
    let web_dir_path = PathBuf::from(&web_dir);
    if !web_dir_path.exists() || !web_dir_path.is_dir() {
        panic!("web_root does not exists or is not an directory: {:?}", &web_dir_path)
    }

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