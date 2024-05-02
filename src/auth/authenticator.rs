use actix_web::{dev::ServiceRequest, Error, HttpResponse};
use actix_web::dev::ServiceResponse;
use actix_web::middleware::ErrorHandlerResponse;
use actix_web_httpauth::extractors::bearer::BearerAuth;
use log::info;

pub(crate) async fn validator(
    req: ServiceRequest,
    credentials: Option<BearerAuth>,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    info!("{:?}", credentials);
    // Ok(req)
    Err((actix_web::error::ErrorUnauthorized("Unauthorized"), req))
}

pub(crate) fn handle_unauthorized<B>(srvres: ServiceResponse<B>) -> actix_web::Result<ErrorHandlerResponse<B>> {
    let (req, _) = srvres.into_parts();
    let resp = HttpResponse::TemporaryRedirect().insert_header(("Location", "login")).finish();
    let result = ServiceResponse::new(req, resp)
        .map_into_boxed_body()
        .map_into_right_body();
    Ok(ErrorHandlerResponse::Response(result))
}
