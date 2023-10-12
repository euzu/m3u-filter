use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use log::{debug, error};
use path_absolutize::*;
use reqwest::header;
use reqwest::header::{HeaderName, HeaderValue};
use crate::model::config::{Config, ConfigInput};
use crate::messaging::send_message;

#[macro_export]
macro_rules! exit {
    ($($arg:tt)*) => {{
        error!($($arg)*);
        std::process::exit(1);
    }};
}


pub(crate) fn get_exe_path() -> PathBuf {
    let default_path = std::path::PathBuf::from("./");
    let current_exe = std::env::current_exe();
    match current_exe {
        Ok(exe) => {
            match fs::read_link(&exe) {
                Ok(f) => f.parent().map_or(default_path, |p| p.to_path_buf()),
                Err(_) => return exe.parent().map_or(default_path, |p| p.to_path_buf())
            }
        }
        Err(_) => default_path
    }
}

fn get_default_file_path(file: String) -> String {
    let path: PathBuf = get_exe_path();
    let working_dir_file = format!("./{}", file);
    let config_path = path.join(file);
    String::from(if config_path.exists() {
        config_path.to_str().unwrap_or(working_dir_file.as_str())
    } else {
        working_dir_file.as_str()
    })
}


pub(crate) fn get_default_config_path() -> String {
    get_default_file_path("config.yml".to_string())
}

pub(crate) fn get_default_mappings_path() -> String {
    get_default_file_path("mapping.yml".to_string())
}

pub(crate) fn get_default_api_proxy_config_path() -> String {
    get_default_file_path("api-proxy.yml".to_string())
}

pub(crate) fn get_working_path(wd: &String) -> String {
    let current_dir = std::env::current_dir().unwrap();
    if wd.is_empty() {
        String::from(current_dir.to_str().unwrap_or("."))
    } else {
        let work_path = std::path::PathBuf::from(wd);
        let wdpath = match fs::metadata(&work_path) {
            Ok(md) => {
                if md.is_dir() && !md.permissions().readonly() {
                    match work_path.canonicalize() {
                        Ok(ap) => Some(ap),
                        Err(_) => None
                    }
                } else {
                    error!("Path not found {:?}", &work_path);
                    None
                }
            }
            Err(_) => None,
        };
        let rp: PathBuf = match wdpath {
            Some(d) => d,
            None => current_dir.join(wd)
        };
        match rp.canonicalize() {
            Ok(ap) => String::from(ap.to_str().unwrap_or("./")),
            Err(_) => {
                error!("Path not found {:?}", &rp);
                String::from("./")
            }
        }
    }
}

pub(crate) fn open_file(file_name: &PathBuf) -> Result<fs::File, std::io::Error> {
    fs::File::open(file_name)
}

pub(crate) fn get_input_content(cfg: &Config, working_dir: &String, url_str: &str, persist_file: Option<PathBuf>) -> Option<Vec<String>> {
    match url_str.parse::<url::Url>() {
        Ok(url) => match download_content(url, persist_file) {
            Ok(content) => Some(content),
            Err(e) => {
                error!("cant download input url: {}  => {}", url_str, e);
                send_message(&cfg.messaging, format!("Failed to download: {}", url_str).as_str());
                None
            }
        }
        Err(_) => {
            let file_path = get_file_path(working_dir, Some(PathBuf::from(url_str)));
            let result = match &file_path {
                Some(file) => {
                    if file.exists() {
                        if let Some(persist_file_value) = persist_file {
                            let to_file = &persist_file_value;
                            match fs::copy(file, to_file) {
                                Ok(_) => {}
                                Err(e) => error!("cant persist to: {}  => {}", to_file.to_str().unwrap_or("?"), e),
                            }
                        };
                        match open_file(file) {
                            Ok(content) => Some(std::io::BufReader::new(content).lines().map(|l| l.unwrap()).collect()),
                            Err(err) => {
                                error!("cant read: {}", err);
                                None
                            },
                        }
                    } else {
                        None
                    }
                }
                None => None
            };
            match result {
                Some(file) => Some(file),
                None => {
                    let msg = format!("cant read input url: {:?}", &file_path.unwrap());
                    error!("{}", msg);
                    send_message(&cfg.messaging, msg.as_str());
                    None
                }
            }
        }
    }
}

fn download_content(url: url::Url, persist_file: Option<PathBuf>) -> Result<Vec<String>, String> {
    match reqwest::blocking::get(url) {
        Ok(response) => {
            if response.status().is_success() {
                match response.text_with_charset("utf8") {
                    Ok(text) => {
                        if persist_file.is_some() {
                            persist_playlist(persist_file, &text);
                        }
                        let result = text.lines().map(String::from).collect();
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

fn persist_playlist(persist_file: Option<PathBuf>, text: &String) {
    if let Some(path_buf) = persist_file {
        let filename = &path_buf.to_str().unwrap_or("?");
        match fs::File::create(&path_buf) {
            Ok(mut file) => match file.write_all(text.as_bytes()) {
                Ok(_) => debug!("persisted: {}", filename),
                Err(e) => error!("failed to persist file {}, {}", filename, e)
            },
            Err(e) => error!("failed to persist file {}, {}", filename, e)
        }
    }
}

pub(crate) fn prepare_persist_path(file_name: &str, date_prefix: &str) -> Option<PathBuf> {
    let now = chrono::Local::now();
    let filename = file_name.replace("{}", format!("{}{}", date_prefix, now.format("%Y%m%d_%H%M%S").to_string().as_str()).as_str());
    Some(std::path::PathBuf::from(filename))
}

pub(crate) fn get_file_path(wd: &String, path: Option<PathBuf>) -> Option<PathBuf> {
    match path {
        Some(p) => {
            if p.is_relative() {
                let pb = PathBuf::from(wd);
                match pb.join(&p).absolutize() {
                    Ok(os) => Some(PathBuf::from(os)),
                    Err(e) => {
                        error!("path is not relative {:?}", e);
                        Some(p)
                    }
                }
            } else {
                Some(p)
            }
        }
        None => None
    }
}

fn download_json_content(input: &ConfigInput, url: url::Url, persist_file: Option<PathBuf>) -> Result<serde_json::Value, String> {
    let mut request = reqwest::blocking::Client::new().get(url);
    if input.headers.is_empty() {
        let mut headers = header::HeaderMap::new();
        for (key, value) in &input.headers {
            headers.insert(
                HeaderName::from_bytes(key.as_bytes()).unwrap(),
                HeaderValue::from_bytes(value.as_bytes()).unwrap(),
            );
        }
        debug!("Request with headers{:?}", &headers);
        request = request.headers(headers);
    }
    match request.send() {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<serde_json::Value>() {
                    Ok(content) => {
                        if persist_file.is_some() {
                            persist_playlist(persist_file, &serde_json::to_string(&content).unwrap());
                        }
                        Ok(content)
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

pub(crate) fn get_input_json_content(input: &ConfigInput, url_str: &String, persist_file: Option<PathBuf>) -> Option<serde_json::Value> {
    match url_str.parse::<url::Url>() {
        Ok(url) => match download_json_content(input, url, persist_file) {
            Ok(content) => Some(content),
            Err(e) => {
                error!("cant download input url: {}  => {}", url_str, e);
                None
            }
        },
        Err(_) => {
            error!("malformed input url: {}", url_str);
            None
        }
    }
}