extern crate pest;
#[macro_use]
extern crate pest_derive;

use clap::Parser;
use crate::config::{Config, validate_targets};
use crate::mapping::Mappings;
use crate::user::User;

mod api;
mod config;
mod filter;
mod m3u_parser;
mod m3u_repository;
mod mapping;
mod model_api;
mod model_config;
mod model_m3u;
mod playlist_processor;
mod repository;
mod download;
mod utils;
mod messaging;
mod xtream_parser;
mod test;
mod xtream_player_api;
mod user;

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
    #[arg(short, long)]
    user: Option<String>,

    /// Run in server mode
    #[arg(short, long, default_value_t = false, default_missing_value = "true")]
    server: bool,

    /// Print more info
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();
    let verbose = args.verbose;

    let default_config_path = utils::get_default_config_path();
    let config_file: String = args.config.unwrap_or(default_config_path);
    let mut cfg = read_config(config_file.as_str(), verbose);
    let targets = validate_targets(&args.target, &cfg.sources);

    if verbose { println!("working dir: {:?}", &cfg.working_dir); }

    read_mappings(args.mapping, &mut cfg, verbose);

    if args.server {
        read_users(args.user, &mut cfg, verbose);
        if verbose { println!("web_root: {}", &cfg.api.web_root); }
        println!("server running: http://{}:{}", &cfg.api.host, &cfg.api.port);
        match api::start_server(cfg, targets, verbose) {
            Ok(_) => {}
            Err(e) => {
                println!("cant start server: {}", e);
                std::process::exit(1);
            }
        };
    } else {
        playlist_processor::process_sources(cfg, &targets, verbose);
    }
}

fn read_mappings(args_mapping: Option<String>, cfg: &mut Config, verbose: bool) {
    let mappings_file: String = args_mapping.unwrap_or(utils::get_default_mappings_path());

    let mappings = read_mapping(mappings_file.as_str(), verbose);
    if verbose && mappings.is_none() { println!("no mapping loaded"); }
    cfg.set_mappings(mappings);
}

fn read_users(args_user: Option<String>, cfg: &mut Config, verbose: bool) {
    let user_file: String = args_user.unwrap_or(utils::get_default_user_path());

    let user = read_user(user_file.as_str(), verbose);
    if user.is_none() {
        println!("cant read user file: {}", user_file.as_str());
        //std::process::exit(1);
    }
    cfg.set_user(user);
}


fn read_config(config_file: &str, verbose: bool) -> Config {
    let mut cfg: Config = match serde_yaml::from_reader(utils::open_file(&std::path::PathBuf::from(config_file), true).unwrap()) {
        Ok(result) => result,
        Err(e) => {
            println!("cant read config file: {}", e);
            std::process::exit(1);
        }
    };
    cfg.prepare(verbose);
    cfg
}

fn read_mapping(mapping_file: &str, verbose: bool) -> Option<Mappings> {
    match utils::open_file(&std::path::PathBuf::from(mapping_file), false) {
        Some(file) => {
            let mapping: Result<Mappings, _> = serde_yaml::from_reader(file);
            match mapping {
                Ok(mut result) => {
                    result.prepare(verbose);
                    Some(result)
                }
                Err(err) => {
                    println!("cant read mapping file: {}", err);
                    None
                }
            }
        }
        _ => None
    }
}

fn read_user(user_file: &str, _verbose: bool) -> Option<User> {
    match utils::open_file(&std::path::PathBuf::from(user_file), false) {
        Some(file) => {
            let mapping: Result<User, _> = serde_yaml::from_reader(file);
            match mapping {
                Ok(result) => {
                    result.prepare(verbose);
                    Some(result)
                }
                Err(err) => {
                    println!("cant read user file: {}", err);
                    None
                }
            }
        }
        _ => None
    }
}
