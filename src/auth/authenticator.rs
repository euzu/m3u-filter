use actix_web::{dev::ServiceRequest, Error, web};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use chrono::{Local, Duration};
use jsonwebtoken::{Algorithm, DecodingKey, encode, decode, EncodingKey, Header, Validation};
use crate::api::api_model::AppState;
use crate::model::config::WebAuthConfig;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Claims {
    iss: String,
    iat: i64,
    exp: i64,
}

pub(crate) fn create_jwt(web_auth_config: &WebAuthConfig) -> Result<String, std::io::Error> {
    let mut header = Header::new(Algorithm::HS256);
    header.typ = Some("JWT".to_string());
    let now = Local::now();
    let iat = now.timestamp();
    let exp = (now + Duration::minutes(30)).timestamp();
    let claims = Claims {
        iss: web_auth_config.issuer.clone(),
        iat,
        exp,
    };
    match encode(&header, &claims, &EncodingKey::from_secret(web_auth_config.secret.as_bytes())) {
        Ok(jwt) => Ok(jwt),
        Err(err) => Err(std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))
    }
}

pub(crate) fn verify_token(bearer: Option<BearerAuth>, secret_key: &[u8]) -> bool {
    if let Some(auth) = bearer {
        let token = auth.token();
        let token_message = decode::<Claims>(&token, &DecodingKey::from_secret(secret_key), &Validation::new(Algorithm::HS256));
        if let Ok(_) = token_message {
            return true;
        }
    }
    false
}

pub(crate) async fn validator(
    req: ServiceRequest,
    credentials: Option<BearerAuth>,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    let app_state: &web::Data<AppState> = req.app_data::<web::Data<AppState>>().unwrap();
    let secret_key = app_state.config.web_auth.as_ref().unwrap().secret.as_ref();
    if verify_token(credentials, secret_key) {
        Ok(req)
    } else {
        Err((actix_web::error::ErrorUnauthorized("Unauthorized"), req))
    }
}

// pub(crate) fn handle_unauthorized<B>(srvres: ServiceResponse<B>) -> actix_web::Result<ErrorHandlerResponse<B>> {
//     let (req, _) = srvres.into_parts();
//     let resp = HttpResponse::TemporaryRedirect().insert_header(("Location", "/auth/login")).finish();
//     let result = ServiceResponse::new(req, resp)
//         .map_into_boxed_body()
//         .map_into_right_body();
//     Ok(ErrorHandlerResponse::Response(result))
// }
