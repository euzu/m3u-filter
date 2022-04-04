extern crate pest;
#[macro_use]
extern crate pest_derive;
use clap::ArgMatches;
use crate::config::Config;
use crate::service::get_playlist;

mod m3u;
mod config;
mod m3u_processing;
mod utils;
mod api;
mod api_model;
mod service;
mod filter;

fn main() {
    let args = get_arguments();

    let default_path = utils::get_default_config_path();
    let config_file = args.value_of("config").unwrap_or(default_path.as_str());

    let cfg = read_config(config_file);
    let verbose = args.is_present("verbose");
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
    let mut cfg: config::Config = match serde_yaml::from_reader(utils::open_file(&std::path::PathBuf::from(config_file))) {
        Ok(result) => result,
        Err(e) => panic!("cant read config file: {}", e)
    };
    cfg.prepare();
    cfg
}

fn get_arguments<'a>() -> ArgMatches<'a> {
    clap::App::new("m3u-filter")
        .version("0.5.0")
        .author("euzu")
        .about("Extended M3U playlist filter")
        .arg(clap::Arg::with_name("config")
            .short("c")
            .long("config")
            .takes_value(true)
            .help("The config file"))
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

