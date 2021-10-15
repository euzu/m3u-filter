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
        let input_arg = args.value_of("input");
        let url_str = input_arg.unwrap_or(if cfg.input.url.is_empty() { "playlist.m3u" } else { cfg.input.url.as_str() });
        let input_perists = || -> Option<std::path::PathBuf> {
            if cfg.input.persist.is_empty() { None } else { utils::prepare_persist_path(cfg.input.persist.as_str()) }
        };
        let persist_file = match input_arg {
            Some(_) => if args.is_present("persist") { utils::prepare_persist_path("playlist_{}.m3u") } else { input_perists() },
            None => input_perists(),
        };
        let file_path = utils::get_file_path(&cfg.working_dir, persist_file);
        if verbose { println!("persist file: {:?}", &file_path); }
        let result = get_playlist(&cfg.working_dir, url_str, file_path);
        if result.is_some() {
            match m3u_processing::write_m3u(&result.unwrap(), &cfg) {
                Ok(_) => (),
                Err(e) => println!("Failed to write file: {}", e)
            }
        }
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
        .arg(clap::Arg::with_name("input")
            .short("i")
            .long("input")
            .takes_value(true)
            .help("Input filename, overrides config input"))
        .arg(clap::Arg::with_name("persist")
            .short("p")
            .long("persist")
            .takes_value(false)
            .help("Persists the input file on disk, if the input parameter is missing it will be ignored!"))
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

