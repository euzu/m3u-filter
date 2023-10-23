use std::path::PathBuf;
use actix_web::http::header::{CACHE_CONTROL, HeaderValue};
use actix_web::{HttpRequest, HttpResponse, web};
use crate::api::model_api::{AppState, UserApiRequest};
use crate::model::config::ConfigTarget;

pub(crate) async fn serve_file(file_path: &PathBuf, req: &HttpRequest) -> HttpResponse {
    if file_path.exists() {
        let file = actix_files::NamedFile::open_async(file_path).await.unwrap()
            .set_content_type(mime::APPLICATION_JSON)
            .disable_content_disposition();
        let mut result = file.into_response(req);
        let headers = result.headers_mut();
        headers.insert(CACHE_CONTROL, HeaderValue::from_bytes("no-cache".as_bytes()).unwrap());
        result
    } else {
        HttpResponse::NoContent().finish()
    }
}

pub(crate) fn get_user_target<'a>(api_req: &'a web::Query<UserApiRequest>, app_state: &'a web::Data<AppState>) -> Option<&'a ConfigTarget> {
    let username = api_req.username.as_str();
    let password = api_req.password.as_str();
    if !username.is_empty() && !password.is_empty() {
        app_state.config.get_target_for_user(username, password)
    } else {
        let token = api_req.token.as_str();
        if !token.is_empty() {
            app_state.config.get_target_for_user_by_token(token)
        } else {
            None
        }
    }
}