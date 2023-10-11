extern crate pest;
#[macro_use]
extern crate pest_derive;
extern crate env_logger;
use env_logger::{Builder};
use log::{debug, error, info, LevelFilter, warn};

use clap::Parser;
use crate::model::api_proxy::ApiProxyConfig;
use crate::model::config::{Config, validate_targets};
use crate::model::mapping::Mappings;

mod model;
mod filter;
mod m3u_parser;
mod playlist_processor;
mod repository;
mod download;
mod utils;
mod messaging;
mod xtream_parser;
mod test;
mod api;

#[derive(Parser)]
#[command(name = "m3u-filter")]
#[command(author = "euzu <euzu@github.com>")]
#[command(version)]
#[command(about = "Extended M3U playlist filter", long_about = None)]
struct Args {
    /// The config file
    #[arg(short, long)]
    config: Option<String>,

    /// The target to process
    #[arg(short, long)]
    target: Option<Vec<String>>,

    /// The mapping file
    #[arg(short, long)]
    mapping: Option<String>,

    /// The user file
    #[arg(short, long = "api-proxy")]
    api_proxy: Option<String>,

    /// Run in server mode
    #[arg(short, long, default_value_t = false, default_missing_value = "true")]
    server: bool,

    /// log level
    #[arg(short, long = "log-level", default_missing_value = "info")]
    log_level: Option<String>,
}

fn main() {
    let args = Args::parse();
    init_logger(&args.log_level.unwrap_or("info".to_string()));

    let default_config_path = utils::get_default_config_path();
    let config_file: String = args.config.unwrap_or(default_config_path);
    let mut cfg = read_config(config_file.as_str());
    let targets = validate_targets(&args.target, &cfg.sources);


    info!("working dir: {:?}", &cfg.working_dir);

    read_mappings(args.mapping, &mut cfg);

    if args.server {
        read_api_proxy_config(args.api_proxy, &mut cfg);
        debug!("web_root: {}", &cfg.api.web_root);
        info!("server running: http://{}:{}", &cfg.api.host, &cfg.api.port);
        match api::main_api::start_server(cfg, targets) {
            Ok(_) => {}
            Err(e) => {
                exit!("cant start server: {}", e);
            }
        };
    } else {
        playlist_processor::process_sources(cfg, &targets);
    }
}

fn init_logger(log_level: &str) {
    let mut log_builder = Builder::new();
    // Set the log level based on the parsed value
    match log_level.to_lowercase().as_str() {
        "trace" => log_builder.filter_level(LevelFilter::Trace),
        "debug" => log_builder.filter_level(LevelFilter::Debug),
        "info" => log_builder.filter_level(LevelFilter::Info),
        "warn" => log_builder.filter_level(LevelFilter::Warn),
        "error" => log_builder.filter_level(LevelFilter::Error),
        _ => log_builder.filter_level(LevelFilter::Info),
    };
    log_builder.init();
}

fn read_mappings(args_mapping: Option<String>, cfg: &mut Config) {
    let mappings_file: String = args_mapping.unwrap_or(utils::get_default_mappings_path());

    let mappings = read_mapping(mappings_file.as_str());
    if mappings.is_none() { debug!("no mapping loaded"); }
    cfg.set_mappings(mappings);
}

fn read_api_proxy_config(args_api_proxy_config: Option<String>, cfg: &mut Config) {
    let api_proxy_config_file: String = args_api_proxy_config.unwrap_or(utils::get_default_api_proxy_config_path());

    let api_proxy_config = read_api_proxy(api_proxy_config_file.as_str());
    if api_proxy_config.is_none() {
        if cfg.has_published_targets() {
            exit!("cant read api_proxy_config file: {}", api_proxy_config_file.as_str());
        } else {
            warn!("cant read api_proxy_config file: {}", api_proxy_config_file.as_str());
        }
    } else {
        cfg.set_api_proxy(api_proxy_config);
    }
}


fn read_config(config_file: &str) -> Config {
    let mut cfg: Config = match serde_yaml::from_reader(utils::open_file(&std::path::PathBuf::from(config_file), true).unwrap()) {
        Ok(result) => result,
        Err(e) => {
            exit!("cant read config file: {}", e);
        }
    };
    cfg.prepare();
    cfg
}

fn read_mapping(mapping_file: &str) -> Option<Mappings> {
    match utils::open_file(&std::path::PathBuf::from(mapping_file), false) {
        Some(file) => {
            let mapping: Result<Mappings, _> = serde_yaml::from_reader(file);
            match mapping {
                Ok(mut result) => {
                    result.prepare();
                    Some(result)
                }
                Err(err) => {
                    error!("cant read mapping file: {}", err);
                    None
                }
            }
        }
        _ => None
    }
}

fn read_api_proxy(api_proxy_file: &str) -> Option<ApiProxyConfig> {
    match utils::open_file(&std::path::PathBuf::from(api_proxy_file), false) {
        Some(file) => {
            let mapping: Result<ApiProxyConfig, _> = serde_yaml::from_reader(file);
            match mapping {
                Ok(result) => {
                    result.prepare();
                    Some(result)
                }
                Err(err) => {
                    error!("cant read api-proxy-config file: {}", err);
                    None
                }
            }
        }
        _ => None
    }
}
