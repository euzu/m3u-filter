use crate::api::endpoints::hdhomerun_api::hdhr_api_register;
use crate::api::endpoints::hls_api::hls_api_register;
use crate::api::endpoints::m3u_api::m3u_api_register;
use crate::api::endpoints::v1_api::v1_api_register;
use crate::api::endpoints::web_index::{index_register_with_path, index_register_without_path};
use crate::api::endpoints::xmltv_api::xmltv_api_register;
use crate::api::endpoints::xtream_api::xtream_api_register;
use crate::api::model::active_provider_manager::ActiveProviderManager;
use crate::api::model::active_user_manager::ActiveUserManager;
use crate::api::model::app_state::{AppState, HdHomerunAppState};
use crate::api::model::download::DownloadQueue;
use crate::api::model::hls_cache::HlsCache;
use crate::api::model::streams::shared_stream_manager::SharedStreamManager;
use crate::api::scheduler::start_scheduler;
use crate::model::config::{validate_targets, Config, ProcessTargets, RateLimitConfig, ScheduleConfig};
use crate::model::healthcheck::{Healthcheck, StatusCheck};
use crate::processing::processor::playlist;
use crate::tools::lru_cache::LRUResourceCache;
use crate::utils::size_utils::human_readable_byte_size;
use crate::utils::sys_utils;
use crate::{BUILD_TIMESTAMP, VERSION};
use axum::response::IntoResponse;
use chrono::{DateTime, Utc};
use log::{error, info};
use reqwest::Client;
use std::collections::BTreeMap;
use std::future::IntoFuture;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use axum::Router;
use tokio::sync::Mutex;
use tower_governor::key_extractor::SmartIpKeyExtractor;

fn get_web_dir_path(web_ui_enabled: bool, web_root: &str) -> Result<PathBuf, std::io::Error> {
    let web_dir = web_root.to_string();
    let web_dir_path = PathBuf::from(&web_dir);
    if web_ui_enabled && (!&web_dir_path.exists() || !&web_dir_path.is_dir()) {
        return Err(std::io::Error::new(ErrorKind::NotFound,
                                       format!("web_root does not exists or is not an directory: {:?}", &web_dir_path)));
    };
    Ok(web_dir_path)
}


fn get_server_time() -> String {
    chrono::offset::Local::now().with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S %Z").to_string()
}

fn get_build_time() -> Option<String> {
    BUILD_TIMESTAMP.to_string().parse::<DateTime<Utc>>().ok().map(|datetime| datetime.format("%Y-%m-%d %H:%M:%S %Z").to_string())
}

fn get_memory_usage() -> String {
    sys_utils::get_memory_usage().map_or(String::from("?"), human_readable_byte_size)
}

fn create_healthcheck() -> Healthcheck {
    Healthcheck {
        status: "ok".to_string(),
        version: VERSION.to_string(),
        build_time: get_build_time(),
        server_time: get_server_time(),
        memory: get_memory_usage(),
    }
}

async fn create_status_check(app_state: &Arc<AppState>) -> StatusCheck {
    let cache = match app_state.cache.as_ref().as_ref() {
        None => None,
        Some(lock) => {
            Some(lock.lock().await.get_size_text())
        }
    };
    let (active_clients, active_client_connections) = {
        let active_user = &app_state.active_users;
        (active_user.active_users().await, active_user.active_connections().await)
    };

    let active_provider_connections = app_state.active_provider.active_connections().map(|c| c.into_iter().collect::<BTreeMap<_, _>>());

    StatusCheck {
        status: "ok".to_string(),
        version: VERSION.to_string(),
        build_time: get_build_time(),
        server_time: get_server_time(),
        memory: get_memory_usage(),
        active_clients,
        active_client_connections,
        active_provider_connections,
        cache,
    }
}

async fn healthcheck() -> impl axum::response::IntoResponse {
    axum::Json(create_healthcheck())
}

async fn status(axum::extract::State(app_state): axum::extract::State<Arc<AppState>>) -> impl axum::response::IntoResponse {
    let status = create_status_check(&app_state).await;
    match serde_json::to_string_pretty(&status) {
        Ok(pretty_json) => axum::response::Response::builder().status(axum::http::StatusCode::OK)
            .header(axum::http::header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string()).body(pretty_json).unwrap().into_response(),
        Err(_) => axum::Json(status).into_response(),
    }
}

fn create_shared_data(cfg: &Arc<Config>) -> AppState {
    let lru_cache = cfg.reverse_proxy.as_ref().and_then(|r| r.cache.as_ref()).and_then(|c| if c.enabled {
        Some(Mutex::new(LRUResourceCache::new(c.t_size, &PathBuf::from(c.dir.as_ref().unwrap()))))
    } else { None });
    let cache = Arc::new(lru_cache);
    let cache_scanner = Arc::clone(&cache);
    tokio::spawn(async move {
        if let Some(m) = cache_scanner.as_ref() {
            let mut c = m.lock().await;
            if let Err(err) = (*c).scan() {
                error!("Failed to scan cache {err}");
            }
        }
    });

    let active_users = Arc::new(ActiveUserManager::new());
    let active_provider = Arc::new(ActiveProviderManager::new(cfg));

    let client = if cfg.connect_timeout_secs > 0 {
        Client::builder()
            .connect_timeout(Duration::from_secs(u64::from(cfg.connect_timeout_secs)))
            .build().unwrap_or_else(|_| Client::new())
    } else {
        Client::new()
    };

    AppState {
        config: Arc::clone(cfg),
        http_client: Arc::new(client),
        downloads: Arc::new(DownloadQueue::new()),
        cache,
        hls_cache: HlsCache::garbage_collected(),
        shared_stream_manager: Arc::new(SharedStreamManager::new()),
        active_users,
        active_provider,
    }
}

fn exec_update_on_boot(client: Arc<reqwest::Client>, cfg: &Arc<Config>, targets: &Arc<ProcessTargets>) {
    if cfg.update_on_boot {
        let cfg_clone = Arc::clone(cfg);
        let targets_clone = Arc::clone(targets);
        tokio::spawn(
            async move { playlist::exec_processing(client, cfg_clone, targets_clone).await }
        );
    }
}


fn get_process_targets(cfg: &Arc<Config>, process_targets: &Arc<ProcessTargets>, exec_targets: Option<&Vec<String>>) -> Arc<ProcessTargets> {
    if let Ok(user_targets) = validate_targets(exec_targets, &cfg.sources) {
        if user_targets.enabled {
            if !process_targets.enabled {
                return Arc::new(user_targets);
            }

            let inputs: Vec<u16> = user_targets.inputs.iter()
                .filter(|&id| process_targets.inputs.contains(id))
                .copied()
                .collect();
            let targets: Vec<u16> = user_targets.targets.iter()
                .filter(|&id| process_targets.inputs.contains(id))
                .copied()
                .collect();
            return Arc::new(ProcessTargets {
                enabled: user_targets.enabled,
                inputs,
                targets,
            });
        }
    }
    Arc::clone(process_targets)
}

fn exec_scheduler(client: &Arc<reqwest::Client>, cfg: &Arc<Config>, targets: &Arc<ProcessTargets>) {
    let schedules: Vec<ScheduleConfig> = if let Some(schedules) = &cfg.schedules {
        schedules.clone()
    } else {
        vec![]
    };
    for schedule in schedules {
        let expression = schedule.schedule.to_string();
        let exec_targets = get_process_targets(cfg, targets, schedule.targets.as_ref());
        let cfg_clone = Arc::clone(cfg);
        let http_client = Arc::clone(client);
        tokio::spawn(async move {
            start_scheduler(http_client, expression.as_str(), cfg_clone, exec_targets).await;
        });
    }
}

fn is_web_auth_enabled(cfg: &Arc<Config>, web_ui_enabled: bool) -> bool {
    if web_ui_enabled {
        if let Some(web_auth) = &cfg.web_auth {
            return web_auth.enabled;
        }
    }
    false
}

fn create_cors_layer() -> tower_http::cors::CorsLayer {
    tower_http::cors::CorsLayer::new()
        // .allow_credentials(true)
        .allow_origin(tower_http::cors::Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST, axum::http::Method::OPTIONS, axum::http::Method::HEAD])
        .allow_headers(tower_http::cors::Any)
        .max_age(std::time::Duration::from_secs(3600))
}
fn create_compression_layer() -> tower_http::compression::CompressionLayer {
    tower_http::compression::CompressionLayer::new()
        .br(true)
        .deflate(true)
        .gzip(true)
        .zstd(true)
}

fn start_hdhomerun(cfg: &Arc<Config>, app_state: &Arc<AppState>, infos: &mut Vec<String>) {
    let host = cfg.api.host.to_string();
    if let Some(hdhomerun) = &cfg.hdhomerun {
        if hdhomerun.enabled {
            for device in &hdhomerun.devices {
                if device.t_enabled {
                    let app_data = Arc::clone(app_state);
                    let app_host = host.clone();
                    let port = device.port;
                    let device_clone = Arc::new(device.clone());
                    infos.push(format!("HdHomeRun Server '{}' running: http://{host}:{port}", device.name));
                    tokio::spawn(async move {
                        let router = axum::Router::<Arc<HdHomerunAppState>>::new()
                            .layer(create_cors_layer())
                            .layer(create_compression_layer())
                            // .layer(TraceLayer::new_for_http()) // `Logger::default()`
                            .merge(hdhr_api_register());

                        let router: axum::Router<()> = router.with_state(Arc::new(HdHomerunAppState {
                            app_state: Arc::clone(&app_data),
                            device: Arc::clone(&device_clone),
                        }));

                        match tokio::net::TcpListener::bind(format!("{}:{}", app_host.clone(), port)).await {
                            Ok(listener) => {
                                if let Err(err) = axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).into_future().await {
                                    error!("{err}");
                                }
                            }
                            Err(err) => error!("{err}"),
                        }
                    });
                }
            }
        }
    }
}

// async fn log_routes(request: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
//     println!("Route : {}", request.uri().path());
//     next.run(request).await
// }

pub async fn start_server(cfg: Arc<Config>, targets: Arc<ProcessTargets>) -> futures::io::Result<()> {
    let mut infos = Vec::new();
    let host = cfg.api.host.to_string();
    let port = cfg.api.port;
    let web_ui_enabled = cfg.web_ui_enabled;
    let web_dir_path = match get_web_dir_path(web_ui_enabled, cfg.api.web_root.as_str()) {
        Ok(result) => result,
        Err(err) => return Err(err)
    };
    if web_ui_enabled {
        infos.push(format!("Web root: {:?}", &web_dir_path));
    }
    let app_state = Arc::new(create_shared_data(&cfg));
    let shared_data = Arc::clone(&app_state);

    exec_scheduler(&Arc::clone(&shared_data.http_client), &cfg, &targets);
    exec_update_on_boot(Arc::clone(&shared_data.http_client), &cfg, &targets);
    let web_auth_enabled = is_web_auth_enabled(&cfg, web_ui_enabled);

    if cfg.t_api_proxy.read().await.is_some() {
        start_hdhomerun(&cfg, &app_state, &mut infos);
    }

    let web_ui_path = cfg.web_ui_path.as_ref().map(|p| format!("/{p}")).unwrap_or_default();

    infos.push(format!("Server running: http://{}:{}", &cfg.api.host, &cfg.api.port));
    for info in &infos {
        info!("{info}");
    }


    // Web Server
    let mut router = axum::Router::new()
        .layer(create_cors_layer())
        .layer(create_compression_layer())        // .layer(TraceLayer::new_for_http()) // `Logger::default()`
        .route("/healthcheck", axum::routing::get(healthcheck))
        .route(&format!("{web_ui_path}/status"), axum::routing::get(status));
    if web_ui_enabled {
        router = router
            .nest_service(&format!("{web_ui_path}/static"), tower_http::services::ServeDir::new(web_dir_path.join("static")))
            .nest_service(&format!("{web_ui_path}/assets"), tower_http::services::ServeDir::new(web_dir_path.join("assets")))
            .merge(v1_api_register(web_auth_enabled, Arc::clone(&shared_data), web_ui_path.as_str()));
        if !web_ui_path.is_empty() {
            router = router.merge(index_register_with_path(&web_dir_path, web_ui_path.as_str()));
        }
    }

    let mut api_router = axum::Router::new()
        .merge(xtream_api_register())
        .merge(m3u_api_register())
        .merge(xmltv_api_register())
        .merge(hls_api_register());
    let mut rate_limiting = false;
    if let Some(rate_limiter) = app_state.config.reverse_proxy.as_ref().and_then(|r| r.rate_limit.clone()) {
        rate_limiting = rate_limiter.enabled;
        api_router = add_rate_limiter(api_router, rate_limiter);
    }

    router = router.merge(api_router);

    if web_ui_enabled && web_ui_path.is_empty() {
        router = router.merge(index_register_without_path(&web_dir_path));
    }

    // router = router.layer(axum::middleware::from_fn(log_routes));

    let router: axum::Router<()> = router.with_state(shared_data.clone());
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    if rate_limiting {
        axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).into_future().await
    } else {
        axum::serve(listener, router).into_future().await
    }
}

fn add_rate_limiter(router: Router<Arc<AppState>>, rate_limit_cfg: RateLimitConfig) -> Router<Arc<AppState>> {
    if rate_limit_cfg.enabled {
        let governor_conf = Arc::new(tower_governor::governor::GovernorConfigBuilder::default()
            .key_extractor(SmartIpKeyExtractor)
            .per_millisecond(rate_limit_cfg.period_millis)
            .burst_size(rate_limit_cfg.burst_size)
            .finish()
            .unwrap());
        router.layer(tower_governor::GovernorLayer {
            config: governor_conf,
        })
    } else {
        router
    }
}
