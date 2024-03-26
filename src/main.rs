extern crate env_logger;
extern crate pest;
#[macro_use]
extern crate pest_derive;

use std::sync::Arc;
use actix_rt::System;

use clap::Parser;
use env_logger::Builder;
use log::{error, info, LevelFilter};

use crate::model::config::{Config, ProcessTargets, validate_targets};
use crate::processing::playlist_processor::exec_processing;
use crate::util::config_reader::{read_api_proxy_config, read_config, read_mappings};
use crate::util::utils;
use crate::utils::file_utils;

mod m3u_filter_error;
mod model;
mod filter;
mod repository;
mod download;
mod messaging;
mod test;
mod api;
mod processing;
mod util;
mod multi_file_reader;
mod utils;

#[derive(Parser)]
#[command(name = "m3u-filter")]
#[command(author = "euzu <euzu@github.com>")]
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
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args = Args::parse();
    init_logger(args.log_level.as_ref().unwrap_or(&"info".to_string()));

    let config_path: String = args.config_path.unwrap_or(file_utils::get_default_config_path());
    let config_file: String = args.config_file.unwrap_or(file_utils::get_default_config_file_path(&config_path));
    let sources_file: String = args.source_file.unwrap_or(file_utils::get_default_sources_file_path(&config_path));


    let mut cfg = read_config(config_path.as_str(), config_file.as_str(), sources_file.as_str()).unwrap_or_else(|err| exit!("{}", err));

    if args.log_level.is_none() {
         if let Some(log_level) =  &cfg.log_level {
             info!("Setting log level to: {}", get_log_level(log_level.as_str()));
             log::set_max_level(get_log_level(log_level.as_str()));
         }
    }

    let targets = validate_targets(&args.target, &cfg.sources).unwrap_or_else(|err| exit!("{}", err));

    info!("Version: {}", VERSION);
    info!("Current time: {}", chrono::offset::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    info!("Working dir: {:?}", &cfg.working_dir);
    info!("Config dir: {:?}", &cfg._config_path);
    info!("Config file: {}", &config_file);
    info!("Source file: {}", &sources_file);

    if let Err(err) = read_mappings(args.mapping_file, &mut cfg) {
        exit!("{}", err);
    }

    if args.server {
        read_api_proxy_config(args.api_proxy, &mut cfg);
        start_in_server_mode(Arc::new(cfg), Arc::new(targets));
    } else {
        start_in_cli_mode(Arc::new(cfg), Arc::new(targets))
    }
}

fn start_in_cli_mode(cfg: Arc<Config>, targets: Arc<ProcessTargets>) {
    System::new().block_on(async { exec_processing(cfg, targets).await });
}

fn start_in_server_mode(cfg: Arc<Config>, targets: Arc<ProcessTargets>) {
    info!("Web root: {}", &cfg.api.web_root);
    info!("Server running: http://{}:{}", &cfg.api.host, &cfg.api.port);
    match api::main_api::start_server(cfg, targets) {
        Ok(_) => {}
        Err(e) => {
            exit!("Can't start server: {}", e);
        }
    };
}

fn get_log_level(log_level: &str) -> LevelFilter {
    match log_level.to_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    }
}

fn init_logger(log_level: &str) {
    let mut log_builder = Builder::from_default_env();
    // Set the log level based on the parsed value
    log_builder.filter_level(get_log_level(log_level));
    log_builder.init();
}
