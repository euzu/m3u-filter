use std::sync::Arc;
use axum::debug_middleware;
use chrono::{Local, Duration};
use jsonwebtoken::{Algorithm, DecodingKey, encode, decode, EncodingKey, Header, Validation, TokenData};
use crate::model::config::WebAuthConfig;
use crate::api::model::app_state::AppState;
use crate::m3u_filter_error::to_io_error;

const ROLE_ADMIN: &str = "ADMIN";
const ROLE_USER: &str = "USER";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Claims {
    pub(crate) username: String,
    iss: String,
    iat: i64,
    exp: i64,
    roles: Vec<String>,
}

pub fn create_jwt_admin(web_auth_config: &WebAuthConfig, username: &str) -> Result<String, std::io::Error> {
    create_jwt(web_auth_config, username, vec![ROLE_ADMIN.to_string()])
}

pub fn create_jwt_user(web_auth_config: &WebAuthConfig, username: &str) -> Result<String, std::io::Error> {
    create_jwt(web_auth_config, username, vec![ROLE_USER.to_string()])
}

fn create_jwt(web_auth_config: &WebAuthConfig, username: &str, roles: Vec<String>) -> Result<String, std::io::Error> {
    let mut header = Header::new(Algorithm::HS256);
    header.typ = Some("JWT".to_string());
    let now = Local::now();
    let iat = now.timestamp();
    let exp = (now + Duration::minutes(30)).timestamp();
    let claims = Claims {
        username: username.to_string(),
        iss: web_auth_config.issuer.clone(),
        iat,
        exp,
        roles
    };
    match encode(&header, &claims, &EncodingKey::from_secret(web_auth_config.secret.as_bytes())) {
        Ok(jwt) => Ok(jwt),
        Err(err) => Err(to_io_error(err))
    }
}

pub(crate) fn verify_token(token: &str, secret_key: &[u8]) -> Option<TokenData<Claims>> {
    if let Ok(token_data) = decode::<Claims>(token, &DecodingKey::from_secret(secret_key), &Validation::new(Algorithm::HS256)) {
        return Some(token_data);
    }
    None
}

fn has_role(token_data: Option<TokenData<Claims>>, role: &str) -> bool {
    if let Some(data) = token_data {
        data.claims.roles.contains(&role.to_string())
    } else {
        false
    }
}

pub fn is_admin(token_data: Option<TokenData<Claims>>) -> bool {
    has_role(token_data, ROLE_ADMIN)
}

pub fn is_user(token_data: Option<TokenData<Claims>>) -> bool {
    has_role(token_data, ROLE_USER)
}

pub fn verify_token_admin(bearer: &str, secret_key: &[u8]) -> bool {
    has_role(verify_token(bearer, secret_key), ROLE_ADMIN)
}

pub fn verify_token_user(bearer: &str, secret_key: &[u8]) -> bool {
    has_role(verify_token(bearer, secret_key), ROLE_USER)
}

fn validate_request(
    app_state: &Arc<AppState>,
    token: &str,
    verify_fn: fn(&str, &[u8]) -> bool,
) -> Result<(), ()> {
    if let Some(web_auth_config) = app_state.config.web_auth.as_ref() {
        let secret_key = web_auth_config.secret.as_ref();
        if verify_fn(token, secret_key) {
            return Ok(());
        }
    }
    Err(())
}

#[debug_middleware]
pub async fn validator_admin(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum_auth::AuthBearer(token): axum_auth::AuthBearer,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    match validate_request(&app_state, &token, verify_token_admin) {
        Ok(()) => Ok(next.run(request).await),
        Err(()) => Err(axum::http::StatusCode::UNAUTHORIZED)

    }
}

#[debug_middleware]
pub async fn validator_user(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum_auth::AuthBearer(token): axum_auth::AuthBearer,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    match validate_request(&app_state, &token, verify_token_user) {
        Ok(()) => Ok(next.run(request).await),
        Err(()) => Err(axum::http::StatusCode::UNAUTHORIZED)
    }
}
