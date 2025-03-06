use std::sync::Arc;
use actix_web::{dev::ServiceRequest, Error, web};
use actix_web_httpauth::extractors::bearer::BearerAuth;
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

pub(crate) fn verify_token(bearer: Option<BearerAuth>, secret_key: &[u8]) -> Option<TokenData<Claims>> {
    if let Some(auth) = bearer {
        let token = auth.token();
        if let Ok(token_data) = decode::<Claims>(token, &DecodingKey::from_secret(secret_key), &Validation::new(Algorithm::HS256)) {
            return Some(token_data);
        }
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

pub fn verify_token_admin(bearer: Option<BearerAuth>, secret_key: &[u8]) -> bool {
    has_role(verify_token(bearer, secret_key), ROLE_ADMIN)
}

pub fn verify_token_user(bearer: Option<BearerAuth>, secret_key: &[u8]) -> bool {
    has_role(verify_token(bearer, secret_key), ROLE_USER)
}

fn validate_request(
    req: ServiceRequest,
    credentials: Option<BearerAuth>,
    verify_fn: fn(Option<BearerAuth>, &[u8]) -> bool, // Funktions-Parameter für Admin/User-Check
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    if let Some(app_state) = req.app_data::<web::Data<Arc<AppState>>>() {
        if let Some(web_auth_config) = app_state.config.web_auth.as_ref() {
            let secret_key = web_auth_config.secret.as_ref();
            if verify_fn(credentials, secret_key) {
                return Ok(req);
            }
        }
    }
    Err((actix_web::error::ErrorUnauthorized("Unauthorized"), req))
}

pub async fn validator_admin(
    req: ServiceRequest,
    credentials: Option<BearerAuth>,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    validate_request(req, credentials, verify_token_admin)
}

pub async fn validator_user(
    req: ServiceRequest,
    credentials: Option<BearerAuth>,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    validate_request(req, credentials, verify_token_user)
}

// pub fn handle_unauthorized<B>(srvres: ServiceResponse<B>) -> actix_web::Result<ErrorHandlerResponse<B>> {
//     let (req, _) = srvres.into_parts();
//     let resp = HttpResponse::TemporaryRedirect().insert_header(("Location", "/auth/login")).finish();
//     let result = ServiceResponse::new(req, resp)
//         .map_into_boxed_body()
//         .map_into_right_body();
//     Ok(ErrorHandlerResponse::Response(result))
// }
