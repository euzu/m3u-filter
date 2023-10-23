extern crate env_logger;
extern crate pest;
#[macro_use]
extern crate pest_derive;

use std::sync::Arc;

use clap::Parser;
use env_logger::Builder;
use log::{debug, error, info, LevelFilter};

use crate::config_reader::{read_api_proxy_config, read_config, read_mappings};
use crate::model::config::{Config, ProcessTargets, validate_targets};
use crate::processing::playlist_processor::exec_processing;

mod m3u_filter_error;
mod config_reader;
mod model;
mod filter;
mod repository;
mod download;
mod utils;
mod messaging;
mod test;
mod api;
mod processing;

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
    let mut cfg = read_config(config_file.as_str()).unwrap_or_else(|err| exit!("{}", err));
    let targets = validate_targets(&args.target, &cfg.sources).unwrap_or_else(|err| exit!("{}", err));

    info!("working dir: {:?}", &cfg.working_dir);

    if let Err(err) = read_mappings(args.mapping, &mut cfg) {
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
    exec_processing(cfg, targets);
}

fn start_in_server_mode(cfg: Arc<Config>, targets: Arc<ProcessTargets>) {
    debug!("web_root: {}", &cfg.api.web_root);
    info!("server running: http://{}:{}", &cfg.api.host, &cfg.api.port);
    match api::main_api::start_server(cfg, targets) {
        Ok(_) => {}
        Err(e) => {
            exit!("cant start server: {}", e);
        }
    };
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


