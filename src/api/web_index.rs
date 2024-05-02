use std::path::PathBuf;

use actix_files::NamedFile;
use actix_web::{HttpRequest, web};
use actix_web::middleware::Condition;
use actix_web_httpauth::middleware::HttpAuthentication;

use crate::api::api_model::AppState;
use crate::auth::authenticator::validator;

async fn index(
    _req: HttpRequest,
    _app_state: web::Data<AppState>,
) -> std::io::Result<NamedFile> {
    let path: PathBuf = [&_app_state.config.api.web_root, "index.html"].iter().collect();
    NamedFile::open(path)
}


pub(crate) fn index_register(web_dir_path: &PathBuf, web_auth_enabled: bool) -> impl Fn(&mut web::ServiceConfig) -> () {
    let wdp = web_dir_path.clone();
    return move |cfg: &mut web::ServiceConfig| {
        cfg.service(web::scope("/")
            .wrap(Condition::new(web_auth_enabled, HttpAuthentication::with_fn(validator)))
            .route("", web::get().to(index))
            .service(actix_files::Files::new("/", &wdp)));
    };
}