mod m3u;
mod config;
mod m3u_processing;
mod utils;

fn main() {
    let matches = clap::App::new("m3u-filter")
        .version("0.1.0")
        .author("euzu")
        .about("Extended M3U playlist filter")
        .arg(clap::Arg::with_name("config")
            .short("c")
            .long("config")
            .takes_value(true)
            .help("the config file"))
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
        .get_matches();

    let default_path = utils::get_default_config_path();
    let config_file = matches.value_of("config").unwrap_or(default_path.as_str());

    let mut cfg: config::Config = match serde_yaml::from_reader(utils::open_file(config_file)) {
        Ok(result) => result,
        Err(e) => {
            println!("cant read config file: {}", e);
            std::process::exit(1);
        }
    };
    cfg.prepare();
    let input_arg = matches.value_of("input");
    let url_str = input_arg.unwrap_or(if cfg.input.url.is_empty() { "playlist.m3u" } else { cfg.input.url.as_str() });
    let persist_file = match input_arg {
        Some(_) => matches.value_of("persist").map_or(None, |p| utils::prepare_persist_path(p)),
        None => if cfg.input.persist.is_empty() { None } else { utils::prepare_persist_path(cfg.input.persist.as_str()) },
    };
    let lines: Vec<String> = utils::get_input_content(url_str, persist_file);
    let result = m3u::decode(&lines);
    m3u_processing::write_m3u(&result, &cfg);
}

