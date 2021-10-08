use std::io::{BufRead, Write};

pub(crate) fn get_default_config_path() -> String {
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

pub(crate) fn open_file(file_name: &str) -> std::fs::File {
    let file = match std::fs::File::open(file_name) {
        Ok(file) => file,
        Err(_) => {
            println!("cant open file: {}", file_name);
            std::process::exit(1);
        }
    };
    file
}

pub(crate) fn get_input_content(url_str: &str, persist_file: Option<std::path::PathBuf>) -> Option<Vec<String>> {
    match url_str.parse::<url::Url>() {
        Ok(url) => match download_content(url, persist_file) {
            Ok(content) => Some(content),
            Err(e) => {
                println!("cant download input url: {}  => {}", url_str, e);
                None
            }
        }
        Err(e) => {
            let file = std::path::Path::new(url_str);
            if file.exists() {
                if persist_file.is_some() {
                    let to_file = &persist_file.unwrap();
                    match std::fs::copy(file, to_file) {
                        Ok(_) => {}
                        Err(e) => println!("cant persist to: {}  => {}", to_file.to_str().unwrap_or("?"), e),
                    }
                };
                Some(std::io::BufReader::new(open_file(url_str)).lines().map(|l| l.unwrap()).collect())
            } else {
                println!("cant read input url: {}  => {}", url_str, e);
                None
            }
        }
    }
}

fn download_content(url: url::Url, persist_file: Option<std::path::PathBuf>) -> Result<Vec<String>, String> {
    match reqwest::blocking::get(url) {
        Ok(response) => {
            if response.status().is_success() {
                match response.text_with_charset("utf8") {
                    Ok(text) => {
                        persist_playlist(persist_file, &text);
                        let result = text.lines().map(|l| String::from(l)).collect();
                        Ok(result)
                    }
                    Err(e) => Err(e.to_string())
                }
            } else {
                Err(format!("Request failed: {}", response.status()))
            }
        }
        Err(e) => Err(e.to_string())
    }
}

fn persist_playlist(persist_file: Option<std::path::PathBuf>, text: &String) {
    match persist_file {
        Some(path_buf) => {
            let filename = &path_buf.to_str().unwrap_or("?");
            match std::fs::File::create(&path_buf) {
                Ok(mut file) => match file.write_all(text.as_bytes()) {
                    Ok(_) => println!("persisted: {}", filename),
                    Err(e) => println!("failed to persist file {}, {}", filename, e)
                },
                Err(e) => println!("failed to persist file {}, {}", filename, e)
            }
        }
        None => {}
    }
}

pub(crate) fn prepare_persist_path(file_name: &str) -> Option<std::path::PathBuf> {
    let now = chrono::Local::now();
    let filename = file_name.replace("{}", now.format("%Y%m%d_%H%M%S").to_string().as_str());
    Some(std::path::PathBuf::from(filename))
}

