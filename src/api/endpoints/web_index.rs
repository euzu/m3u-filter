use std::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use axum::response::IntoResponse;
use crate::api::api_utils::serve_file;
use crate::api::model::app_state::AppState;
use crate::auth::authenticator::{create_jwt_admin, create_jwt_user, is_admin, verify_token};
use crate::auth::password::verify_password;
use crate::auth::user::UserCredential;

fn no_web_auth_token() ->  impl axum::response::IntoResponse + Send {
    axum::Json(HashMap::from([("token", "authorized")])).into_response()
}

async fn token(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(mut req): axum::extract::Json<UserCredential>,
) ->  impl axum::response::IntoResponse + Send {
    match &app_state.config.web_auth {
        None => no_web_auth_token().into_response(),
        Some(web_auth) => {
            if !web_auth.enabled {
                return no_web_auth_token().into_response();
            }
            let username = req.username.as_str();
            let password = req.password.as_str();

            if !(username.is_empty() || password.is_empty()) {
                if let Some(hash) = web_auth.get_user_password(username) {
                    if verify_password(hash, password.as_bytes()) {
                        if let Ok(token) = create_jwt_admin(web_auth, username) {
                            req.zeroize();
                            return axum::Json(HashMap::from([("token", token)])).into_response();
                        }
                    }
                }
                if let Some(credentials) = app_state.config.get_user_credentials(username).await {
                    if credentials.password == password {
                        if let Ok(token) = create_jwt_user(web_auth, username) {
                            req.zeroize();
                            return axum::Json(HashMap::from([("token", token)])).into_response();
                        }
                    }
                }
            }

            req.zeroize();
            axum::http::StatusCode::BAD_REQUEST.into_response()
        }
    }
}

async fn token_refresh(
    axum_auth::AuthBearer(token): axum_auth::AuthBearer,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) ->  impl axum::response::IntoResponse + Send {
    match &app_state.config.web_auth {
        None => no_web_auth_token().into_response(),
        Some(web_auth) => {
            if !web_auth.enabled {
                return no_web_auth_token().into_response();
            }
            let secret_key = web_auth.secret.as_ref();
            let maybe_token_data = verify_token(&token, secret_key);
            if let Some(token_data) = maybe_token_data {
                let username = token_data.claims.username.clone();
                let web_auth_cfg = app_state.config.web_auth.as_ref().unwrap();
                let new_token =  if is_admin(Some(token_data)) {
                    create_jwt_admin(web_auth_cfg, &username)
                } else {
                    create_jwt_user(web_auth_cfg, &username)
                };
                if let Ok(token) = new_token {
                    return axum::Json(HashMap::from([("token", token)])).into_response();
                }
            }
            axum::http::StatusCode::BAD_REQUEST.into_response()
        }
    }
}

async fn index(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) -> impl axum::response::IntoResponse + Send {
    let path: PathBuf = [&app_state.config.api.web_root, "index.html"].iter().collect();
    serve_file(&path, mime::TEXT_HTML_UTF_8).await.into_response()
}

pub fn index_register(web_dir_path: &Path) -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .nest("/auth", axum::Router::new()
            .route("/token", axum::routing::post(token))
            .route("/refresh", axum::routing::post(token_refresh)))
        .merge(axum::Router::new()
            .route("/", axum::routing::get(index))
            .fallback(axum::routing::get_service(tower_http::services::ServeDir::new(web_dir_path))))
}
// pub fn index_register(web_dir_path: &Path) -> impl Fn(&mut web::ServiceConfig) + '_ {
//     move |cfg: &mut web::ServiceConfig| {
//         cfg.service(web::scope("/auth")
//             .route("/token", web::post().to(token))
//             .route("/refresh", web::post().to(token_refresh)));
//         cfg.service(web::scope("")
//             .route("/", web::get().to(index))
//             .service(actix_files::Files::new("", web_dir_path)));
//     }
// }