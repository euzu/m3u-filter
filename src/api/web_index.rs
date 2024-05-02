use std::collections::HashMap;
use std::path::PathBuf;

use actix_files::NamedFile;
use actix_web::{HttpRequest, HttpResponse, web};
use actix_web_httpauth::extractors::bearer::BearerAuth;

use crate::api::api_model::AppState;
use crate::auth::authenticator::{create_jwt, verify_token};
use crate::auth::password::verify_password;
use crate::auth::user::UserCredential;
use crate::model::config::WebAuthConfig;

async fn token(
    mut req: web::Json<UserCredential>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let username = req.username.as_str();
    let password = req.password.as_str();

    if username.len() > 0 && password.len() > 0 {
        let web_auth = app_state.config.web_auth.as_ref().unwrap();
        if let Some(hash) = web_auth.get_user_password(username) {
            if verify_password(hash, &password.as_bytes()) {
                req.zeroize();
                if let Ok(token) = create_jwt(web_auth) {
                    return HttpResponse::Ok().json(HashMap::from([("token", token)]));
                }
            };
        }
    }
    req.zeroize();
    HttpResponse::BadRequest().finish()
}

async fn token_refresh(
    _req: HttpRequest,
    credentials: Option<BearerAuth>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let secret_key = app_state.config.web_auth.as_ref().unwrap().secret.as_ref();
    if verify_token(credentials, secret_key) {
        if let Ok(token) = create_jwt(app_state.config.web_auth.as_ref().unwrap()) {
            return HttpResponse::Ok().json(HashMap::from([("token", token)]));
        }
    }
    HttpResponse::BadRequest().finish()
}


async fn index(
    _req: HttpRequest,
    app_state: web::Data<AppState>,
) -> std::io::Result<NamedFile> {
    let path: PathBuf = [&app_state.config.api.web_root, "index.html"].iter().collect();
    NamedFile::open(path)
}

pub(crate) fn index_register(web_dir_path: &PathBuf, web_auth_config: WebAuthConfig) -> impl Fn(&mut web::ServiceConfig) -> () {
    let wdp = web_dir_path.clone();
    let web_auth_enabled = web_auth_config.enabled;
    return move |cfg: &mut web::ServiceConfig| {
        if web_auth_enabled {
            cfg.service(web::scope("/auth")
                .route("/token", web::post().to(token))
                .route("/refresh", web::post().to(token_refresh)));
        }
        cfg.service(web::scope("/")
            .route("", web::get().to(index))
            .service(actix_files::Files::new("/", &wdp)));
    };
}