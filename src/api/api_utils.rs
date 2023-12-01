use std::path::{Path};
use actix_web::http::header::{CACHE_CONTROL, HeaderValue};
use actix_web::{HttpRequest, HttpResponse, web};
use crate::api::api_model::{AppState, UserApiRequest};
use crate::model::api_proxy::{UserCredentials};
use crate::model::config::ConfigTarget;

pub(crate) async fn serve_file(file_path: &Path, req: &HttpRequest) -> HttpResponse {
    if file_path.exists() {
        if let Ok(file) = actix_files::NamedFile::open_async(file_path).await {
            let mut result = file.set_content_type(mime::APPLICATION_JSON)
                .disable_content_disposition().into_response(req);
            let headers = result.headers_mut();
            headers.insert(CACHE_CONTROL, HeaderValue::from_bytes("no-cache".as_bytes()).unwrap());
            return result;
        }
    }
    HttpResponse::NoContent().finish()
}

pub(crate) fn get_user_target_by_credentials<'a>(username: &str, password: &str, api_req: &'a web::Query<UserApiRequest>, app_state: &'a web::Data<AppState>) -> Option<(UserCredentials, &'a ConfigTarget)> {
    if !username.is_empty() && !password.is_empty() {
        app_state.config.get_target_for_user(username, password)
    } else {
        let token = api_req.token.as_str().trim();
        if !token.is_empty() {
            app_state.config.get_target_for_user_by_token(token)
        } else {
            None
        }
    }
}


pub(crate) fn get_user_target<'a>(api_req: &'a web::Query<UserApiRequest>, app_state: &'a web::Data<AppState>) -> Option<(UserCredentials, &'a ConfigTarget)> {
    let username = api_req.username.as_str().trim();
    let password = api_req.password.as_str().trim();
    get_user_target_by_credentials(username, password, api_req, app_state)
}