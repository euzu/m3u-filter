use std::io::BufRead;

mod m3u;
mod config;
mod processor;

fn open_file(file_name: &str) -> std::fs::File {
    let file = match std::fs::File::open(file_name) {
        Ok(file) => file,
        Err(_) => {
            println!("cant open file: {}", file_name);
            std::process::exit(1);
        }
    };
    file
}

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
        .get_matches();

    let default_path = get_default_config_path();
    let config_file = matches.value_of("config").unwrap_or(default_path.as_str());

    let mut cfg: config::Config = match serde_yaml::from_reader(open_file(config_file)) {
        Ok(result) => result,
        Err(e) => {
            println!("cant read config file: {}", e);
            std::process::exit(1);
        }
    };
    cfg.prepare();
    let file_name = matches.value_of("input").unwrap_or(if cfg.input.filename.is_empty() { "playlist.m3u" } else { cfg.input.filename.as_str() });
    let reader: Vec<String> = std::io::BufReader::new(open_file(file_name)).lines().map(|l| l.expect("Could not parse line")).collect();
    let result = m3u::decode(&reader);

    processor::write_m3u(&result, &cfg);
}

fn get_default_config_path() -> String {
    let default_path = std::path::Path::new("./");
    let current_exe = std::env::current_exe();
    let path: &std::path::Path = match current_exe {
        Ok(ref exe) => exe.parent().unwrap_or(default_path),
        Err(_) => default_path
    };
    let config_path = path.join("config.yml");
    String::from(if config_path.exists() {
        config_path.to_str().unwrap_or("./config.yml")
    } else {
        "./config.yml"
    })
}

