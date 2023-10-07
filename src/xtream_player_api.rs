use actix_web::{HttpResponse, web, get};
use crate::model_api::AppState;
use crate::repository::{COL_CAT_LIVE, COL_CAT_SERIES, COL_CAT_VOD, COL_LIVE, COL_SERIES, COL_VOD, get_all};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct XtreamApiRequest {
    username: String,
    password: String,
    action: String,
}

#[get("/player_api.php")]
pub(crate) async fn xtream_player_api(
    api_req: web::Query<XtreamApiRequest>,
    //req: HttpRequest,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    match _app_state.config.get_target_for_user(api_req.username.as_str(), api_req.password.as_str()) {
        Some(target_name) => {
            let target = target_name.as_str();
            match api_req.action.as_str() {
                "get_live_categories" => HttpResponse::Ok().json(get_all(&_app_state.config, target, COL_CAT_LIVE)),
                "get_vod_categories" => HttpResponse::Ok().json(get_all(&_app_state.config, target, COL_CAT_VOD)),
                "get_series_categories" => HttpResponse::Ok().json(get_all(&_app_state.config, target, COL_CAT_SERIES)),
                "get_live_streams" => HttpResponse::Ok().json(get_all(&_app_state.config, target, COL_LIVE)),
                "get_vod_streams" => HttpResponse::Ok().json(get_all(&_app_state.config, target, COL_VOD)),
                "get_series" => HttpResponse::Ok().json(get_all(&_app_state.config, target, COL_SERIES)),
                _ => HttpResponse::BadRequest().finish()
            }
        }
        _ => HttpResponse::BadRequest().finish()
    }
}