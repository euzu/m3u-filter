use std::collections::HashMap;
use std::path::{Path, PathBuf};
use actix_files::NamedFile;
use actix_web::{HttpRequest, HttpResponse, web};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use crate::api::model::app_state::AppState;
use crate::auth::authenticator::{create_jwt_admin, create_jwt_user, is_admin, verify_token};
use crate::auth::password::verify_password;
use crate::auth::user::UserCredential;

fn no_web_auth_token() -> HttpResponse {
    HttpResponse::Ok().json(HashMap::from([("token", "authorized")]))
}

async fn token(
    mut req: web::Json<UserCredential>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    match &app_state.config.web_auth {
        None => no_web_auth_token(),
        Some(web_auth) => {
            if !web_auth.enabled {
                return no_web_auth_token();
            }
            let username = req.username.as_str();
            let password = req.password.as_str();

            if !(username.is_empty() || password.is_empty()) {
                if let Some(hash) = web_auth.get_user_password(username) {
                    if verify_password(hash, password.as_bytes()) {
                        if let Ok(token) = create_jwt_admin(web_auth, username) {
                            req.zeroize();
                            return HttpResponse::Ok().json(HashMap::from([("token", token)]));
                        }
                    }
                }
                if let Some(credentials) = app_state.config.get_user_credentials(username) {
                    if credentials.password == password {
                        if let Ok(token) = create_jwt_user(web_auth, username) {
                            req.zeroize();
                            return HttpResponse::Ok().json(HashMap::from([("token", token)]));
                        }
                    }
                }
            }

            req.zeroize();
            HttpResponse::BadRequest().finish()
        }
    }
}

async fn token_refresh(
    _req: HttpRequest,
    credentials: Option<BearerAuth>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    match &app_state.config.web_auth {
        None => {
            no_web_auth_token()
        },
        Some(web_auth) => {
            if !web_auth.enabled {
                return no_web_auth_token();
            }
            let secret_key = web_auth.secret.as_ref();
            let maybe_token_data = verify_token(credentials, secret_key);
            if let Some(token_data) = maybe_token_data {
                let username = token_data.claims.username.clone();
                let web_auth_cfg = app_state.config.web_auth.as_ref().unwrap();
                let new_token =  if is_admin(Some(token_data)) {
                    create_jwt_admin(web_auth_cfg, &username)
                } else {
                    create_jwt_user(web_auth_cfg, &username)
                };
                if let Ok(token) = new_token {
                    return HttpResponse::Ok().json(HashMap::from([("token", token)]));
                }
            }
            HttpResponse::BadRequest().finish()
        }
    }
}

async fn index(
    _req: HttpRequest,
    app_state: web::Data<AppState>,
) -> std::io::Result<NamedFile> {
    let path: PathBuf = [&app_state.config.api.web_root, "index.html"].iter().collect();
    NamedFile::open(path)
}

pub fn index_register(web_dir_path: &Path) -> impl Fn(&mut web::ServiceConfig) + '_ {
    move |cfg: &mut web::ServiceConfig| {
        cfg.service(web::scope("/auth")
            .route("/token", web::post().to(token))
            .route("/refresh", web::post().to(token_refresh)));
        cfg.service(web::scope("")
            .route("/", web::get().to(index))
            .service(actix_files::Files::new("", web_dir_path)));
    }
}