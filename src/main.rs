extern crate pest;
#[macro_use]
extern crate pest_derive;

use clap::ArgMatches;
use crate::config::Config;
use crate::mapping::Mappings;
use crate::service::get_playlist;

mod m3u;
mod config;
mod mapping;
mod m3u_processing;
mod utils;
mod api;
mod api_model;
mod service;
mod filter;
mod model;

fn main() {
    let args = get_arguments();
    let verbose = args.is_present("verbose");

    let default_config_path = utils::get_default_config_path();
    let config_file = args.value_of("config").unwrap_or(default_config_path.as_str());
    let mut cfg = read_config(config_file);

    let default_mappings_path = utils::get_default_mappings_path();
    let mappings_file = args.value_of("mapping").unwrap_or(default_mappings_path.as_str());

    let mappings = read_mapping(mappings_file);
    if verbose && mappings.is_none() { println!("no mapping loaded"); }
    cfg.set_mappings(mappings);

    if verbose { println!("working dir: {:?}", &cfg.working_dir); }

    if args.is_present("server") {
        if verbose { println!("web_root: {}", &cfg.api.web_root); }
        println!("server running: http://{}:{}", &cfg.api.host, &cfg.api.port);
        match api::start_server(cfg.clone()) {
            Ok(_) => {}
            Err(e) => panic!("cant start server: {}", e)
        };
    } else {
        m3u_processing::process_targets(&cfg, verbose)
    }
}

fn read_config(config_file: &str) -> Config {
    let mut cfg: config::Config = match serde_yaml::from_reader(utils::open_file(&std::path::PathBuf::from(config_file), true).unwrap()) {
        Ok(result) => result,
        Err(e) => panic!("cant read config file: {}", e)
    };
    cfg.prepare();
    cfg
}

fn read_mapping(mapping_file: &str) -> Option<Mappings> {
    match utils::open_file(&std::path::PathBuf::from(mapping_file), false) {
        Some(file) => {
            let mapping: Result<mapping::Mappings, _> = serde_yaml::from_reader(file);
            match mapping {
                Ok(mut result) => {
                    result.prepare();
                    Some(result)
                }
                Err(_) => {
                    //println!("cant read mapping file: {}", e);
                    None
                }
            }
        }
        _ => None
    }
}

fn get_arguments<'a>() -> ArgMatches<'a> {
    clap::App::new("m3u-filter")
        .version("0.9.5")
        .author("euzu")
        .about("Extended M3U playlist filter")
        .arg(clap::Arg::with_name("config")
            .short("c")
            .long("config")
            .takes_value(true)
            .help("The config file"))
        .arg(clap::Arg::with_name("mapping")
            .short("m")
            .long("mapping")
            .takes_value(true)
            .help("The mapping file"))
        .arg(clap::Arg::with_name("server")
            .short("s")
            .long("server")
            .takes_value(false)
            .help("Starts the api server for web-ui! All other arguments are ignored!"))
        .arg(clap::Arg::with_name("verbose")
            .short("v")
            .long("verbose")
            .takes_value(false)
            .help("Print  more log!"))
        .get_matches()
}

