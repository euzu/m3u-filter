use std::path::PathBuf;
use actix_web::http::header::{CACHE_CONTROL, HeaderValue};
use actix_web::{HttpRequest, HttpResponse};

pub async fn serve_file(file_path: &PathBuf, req: &HttpRequest) -> HttpResponse {
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