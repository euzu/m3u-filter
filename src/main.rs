#![warn(clippy::all,clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::missing_errors_doc)]

#[macro_use]
mod modules;

include_modules!();

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use crate::auth::password::generate_password;
use crate::model::config::{validate_targets, Config, HealthcheckConfig, LogLevelConfig, ProcessTargets};
use crate::model::healthcheck::Healthcheck;
use crate::processing::processor::playlist;
use utils::file::config_reader;
use crate::utils::file::file_utils;
use crate::utils::network::request::set_sanitize_sensitive_info;
use clap::Parser;
use env_logger::Builder;
use log::{error, info, LevelFilter};
use crate::utils::file::config_reader::config_file_reader;

const LOG_ERROR_LEVEL_MOD: &[&str] = &[
    "reqwest::async_impl::client",
    "reqwest::connect",
    "hyper_util::client",
];


#[derive(Parser)]
#[command(name = "m3u-filter")]
#[command(author = "euzu <euzu@proton.me>")]
#[command(version)]
#[command(about = "Extended M3U playlist filter", long_about = None)]
struct Args {
    /// The config directory
    #[arg(short = 'p', long = "config-path")]
    config_path: Option<String>,

    /// The config file
    #[arg(short = 'c', long = "config")]
    config_file: Option<String>,

    /// The source config file
    #[arg(short = 'i', long = "source")]
    source_file: Option<String>,

    /// The mapping file
    #[arg(short = 'm', long = "mapping")]
    mapping_file: Option<String>,

    /// The target to process
    #[arg(short = 't', long)]
    target: Option<Vec<String>>,

    /// The user file
    #[arg(short = 'a', long = "api-proxy")]
    api_proxy: Option<String>,

    /// Run in server mode
    #[arg(short = 's', long, default_value_t = false, default_missing_value = "true")]
    server: bool,

    /// log level
    #[arg(short = 'l', long = "log-level", default_missing_value = "info")]
    log_level: Option<String>,

    #[arg(short = None, long = "genpwd", default_value_t = false, default_missing_value = "true")]
    genpwd: bool,

    #[arg(short = None, long = "healthcheck", default_value_t = false, default_missing_value = "true"
    )]
    healthcheck: bool,
}


const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_TIMESTAMP:&str = env!("VERGEN_BUILD_TIMESTAMP");

// #[cfg(not(target_env = "msvc"))]
// #[global_allocator]
// static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
//
// #[allow(non_upper_case_globals)]
// #[export_name = "malloc_conf"]
// pub static malloc_conf: &[u8] = b"lg_prof_interval:25,prof:true,prof_leak:true,prof_active:true,prof_prefix:/tmp/jeprof\0";

fn main() {
    let args = Args::parse();

    if args.genpwd {
        match generate_password() {
            Ok(pwd) => println!("{pwd}"),
            Err(err) => eprintln!("{err}"),
        }
        return;
    }

    let config_path: String = args.config_path.unwrap_or_else(file_utils::get_default_config_path);
    let config_file: String = args.config_file.unwrap_or_else(|| file_utils::get_default_config_file_path(&config_path));

    let env_log_level = std::env::var("M3U_FILTER_LOG");
    init_logger(args.log_level.as_ref(), env_log_level.ok(), config_file.as_str());

    if args.healthcheck {
        healthcheck(config_file.as_str());
    }

    let sources_file: String = args.source_file.unwrap_or_else(|| file_utils::get_default_sources_file_path(&config_path));
    let mut cfg = config_reader::read_config(config_path.as_str(), config_file.as_str(), sources_file.as_str()).unwrap_or_else(|err| exit!("{}", err));

    set_sanitize_sensitive_info(cfg.log.as_ref().is_none_or(|l| l.sanitize_sensitive_info));

    let temp_path = PathBuf::from(&cfg.working_dir).join("tmp");
    create_directories(&cfg, &temp_path);
    let _ = tempfile::env::override_temp_dir(&temp_path);

    let targets = validate_targets(args.target.as_ref(), &cfg.sources).unwrap_or_else(|err| exit!("{}", err));

    info!("Version: {VERSION}");
    if let Some(bts) = BUILD_TIMESTAMP.to_string().parse::<DateTime<Utc>>().ok().map(|datetime| datetime.format("%Y-%m-%d %H:%M:%S %Z").to_string()) {
        info!("Build time: {bts}");
    }
    info!("Current time: {}", chrono::offset::Local::now().format("%Y-%m-%d %H:%M:%S"));
    info!("Working dir: {:?}", &cfg.working_dir);
    info!("Config dir: {:?}", &cfg.t_config_path);
    info!("Config file: {config_file:?}");
    info!("Source file: {sources_file:?}");
    info!("Temp dir: {temp_path:?}");
    if let Some(cache) = cfg.reverse_proxy.as_ref().and_then(|r| r.cache.as_ref()) {
        if cache.enabled {
            info!("Cache dir: {:?}", cache.dir.as_ref().unwrap_or(&String::new()));
        }
    }

    match config_reader::read_mappings(args.mapping_file, &mut cfg, true) {
        Ok(Some(mapping_file)) => {
            info!("Mapping file: {mapping_file:?}");
        }
        Ok(None) => {}
        Err(err) => exit!("{err}"),
    }

    // if cfg.t_channel_unavailable_video.is_some() {
    //     info!("Channel unavailable video loaded from {:?}", cfg.channel_unavailable_file.as_ref().map_or("?", |v| v.as_str()));
    // }

    let rt  = tokio::runtime::Runtime::new().unwrap();
    let () = rt.block_on(async {
        if args.server {
            match config_reader::read_api_proxy_config(args.api_proxy, &mut cfg).await {
                Ok(Some(api_proxy_file)) => {
                    info!("Api Proxy File: {api_proxy_file:?}");
                },
                Ok(None) => {}
                Err(err) => exit!("{err}"),
            }
            start_in_server_mode(Arc::new(cfg), Arc::new(targets)).await;
        } else {
            start_in_cli_mode(Arc::new(cfg), Arc::new(targets)).await;
        }
    });
}

fn create_directories(cfg: &Config, temp_path: &Path) {
    // Collect the paths into a vector.
    let paths_strings = [
        Some(cfg.working_dir.clone()),
        cfg.backup_dir.clone(),
        cfg.user_config_dir.clone(),
        cfg.video.as_ref().and_then(|v| v.download.as_ref()).and_then(|d| d.directory.clone()),
        cfg.reverse_proxy.as_ref().and_then(|r| r.cache.as_ref().and_then(|c| if c.enabled { c.dir.clone() } else { None }))
    ];

    let mut paths: Vec<PathBuf> = paths_strings.iter()
        .filter_map(|opt| opt.as_ref()) // Get rid of the `Option`
        .map(PathBuf::from).collect();
    paths.push(temp_path.to_path_buf());

    // Iterate over the paths, filter out `None` values, and process the `Some(path)` values.
    for path in &paths {
        if !path.exists() {
            // Create the directory tree if it doesn't exist
            let path_value = path.to_str().unwrap_or("?");
            if let Err(e) = std::fs::create_dir_all(path) {
                error!("Failed to create directory {path_value}: {e}");
            } else {
                info!("Created directory: {path_value}");
            }
        }
    }
}

async fn start_in_cli_mode(cfg: Arc<Config>, targets: Arc<ProcessTargets>) {
    let client = Arc::new(reqwest::Client::new());
    playlist::exec_processing(client, cfg, targets).await;
}

async fn start_in_server_mode(cfg: Arc<Config>, targets: Arc<ProcessTargets>) {
    if let Err(err) = api::main_api::start_server(cfg, targets).await {
        exit!("Can't start server: {err}");
    }
}

fn get_log_level(log_level: &str) -> LevelFilter {
    match log_level.to_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        // "info" => LevelFilter::Info,
        _ => LevelFilter::Info,
    }
}

fn init_logger(user_log_level: Option<&String>, env_log_level: Option<String>, config_file: &str) {
    let mut log_builder = Builder::from_default_env();

    // priority  CLI-Argument, Env-Var, Config, Default
    let log_level = user_log_level
        .map(std::string::ToString::to_string) // cli-argument
        .or(env_log_level) // env
        .or_else(|| {               // config
            File::open(config_file).ok()
                .and_then(|file| serde_yaml::from_reader::<_, LogLevelConfig>(config_file_reader(file, true)).ok())
                .and_then(|cfg| cfg.log.and_then(|l| l.log_level))
        })
        .unwrap_or_else(|| "info".to_string()); // Default

    if log_level.contains('=') {
        for pair in log_level.split(',').filter(|s| s.contains('=')) {
            let mut kv_iter = pair.split('=').map(str::trim);
            if let (Some(module), Some(level)) = (kv_iter.next(), kv_iter.next()) {
                log_builder.filter_module(module, get_log_level(level));
            }
        }
    } else {
        // Set the log level based on the parsed value
        log_builder.filter_level(get_log_level(&log_level));
    }
    for module in LOG_ERROR_LEVEL_MOD {
        log_builder.filter_module(module, LevelFilter::Error);
    }
    log_builder.init();
    info!("Log Level {}", get_log_level(&log_level));
}

fn healthcheck(config_file: &str) {
    let path = std::path::PathBuf::from(config_file);
    let file = File::open(path).expect("Failed to open config file");
    let config: HealthcheckConfig = serde_yaml::from_reader(config_file_reader(file, true)).expect("Failed to parse config file");

    if let Ok(response) = reqwest::blocking::get(format!("http://localhost:{}/healthcheck", config.api.port)) {
        if let Ok(check) = response.json::<Healthcheck>() {
            if check.status == "ok" {
                std::process::exit(0);
            }
        }
    }

    std::process::exit(1);
}
