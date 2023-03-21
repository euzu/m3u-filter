extern crate pest;
#[macro_use]
extern crate pest_derive;

use std::collections::HashMap;
use std::sync::Arc;
use clap::Parser;
use crate::config::{Config, ConfigSource, ProcessTargets};
use crate::mapping::Mappings;

mod m3u;
mod m3u_processor;
mod xtream_processor;
mod config;
mod mapping;
mod m3u_processing;
mod utils;
mod api;
mod api_model;
mod service;
mod filter;
mod model;

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

    /// Run in server mode
    #[arg(short, long, default_value_t = false, default_missing_value = "true")]
    server: bool,

    /// Print more info
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

fn validate_targets(target_args: &Option<Vec<String>>, sources: &Vec<ConfigSource>) -> ProcessTargets {

    let mut enabled = true;
    let mut inputs: Vec<u16> = vec![];
    let mut targets: Vec<u16> = vec![];
    if let Some(user_targets) = target_args {
        let mut check_targets: HashMap<String, u16> =   user_targets.iter().map(|t| (t.to_lowercase(), 0)).collect();
        for source in sources {
            let mut target_added = false;
            for target in &source.targets {
                for user_target  in user_targets {
                    let key = user_target.to_lowercase();
                    if target.name.eq_ignore_ascii_case(key.as_str()) {
                        targets.push(target.id);
                        target_added = true;
                        if let Some(value) = check_targets.get(key.as_str()) {
                            check_targets.insert(key, value+1);
                        }
                    }
                }
            }
            if target_added {
                inputs.push(source.input.id);
            }
        }

        let missing_targets: Vec<String> = check_targets.iter().filter(|&(_, v)|  *v == 0).map(|(k, _)| k.to_string()).collect();
        if !missing_targets.is_empty() {
            println!("No target found for {}", missing_targets.join(", "));
            std::process::exit(1);
        }
        let processing_targets: Vec<String> = check_targets.iter().filter(|&(_, v)|  *v != 0).map(|(k, _)| k.to_string()).collect();
        println!("Processing targets {}", processing_targets.join(", "));

    } else {
        enabled = false;
    }

   ProcessTargets {
       enabled,
       inputs,
       targets,
   }
}

fn main() {
    //let args = get_arguments();
    let args = Args::parse();
    let verbose = args.verbose;

    let default_config_path = utils::get_default_config_path();
    let config_file: String = args.config.unwrap_or(default_config_path);
    let mut cfg = read_config(config_file.as_str(), verbose);
    let targets = validate_targets(&args.target, &cfg.sources);

    let default_mappings_path = utils::get_default_mappings_path();
    let mappings_file: String = args.mapping.unwrap_or(default_mappings_path);

    let mappings = read_mapping(mappings_file.as_str(), verbose);
    if verbose && mappings.is_none() { println!("no mapping loaded"); }
    cfg.set_mappings(mappings);
    if verbose { println!("working dir: {:?}", &cfg.working_dir); }

    if args.server {
        if verbose { println!("web_root: {}", &cfg.api.web_root); }
        println!("server running: http://{}:{}", &cfg.api.host, &cfg.api.port);
        match api::start_server(cfg) {
            Ok(_) => {}
            Err(e) => {
                println!("cant start server: {}", e);
                std::process::exit(1);
            }
        };
    } else {
        let config = Arc::new(cfg);
        m3u_processing::process_sources(config, &targets, verbose);
    }
}

fn read_config(config_file: &str, verbose: bool) -> Config {
    let mut cfg: config::Config = match serde_yaml::from_reader(utils::open_file(&std::path::PathBuf::from(config_file), true).unwrap()) {
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
            let mapping: Result<mapping::Mappings, _> = serde_yaml::from_reader(file);
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

