use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use path_absolutize::*;
use reqwest::header;
use reqwest::header::{HeaderName, HeaderValue};
use crate::config::ConfigInput;

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

pub(crate) fn get_default_config_path() -> String {
    let path: PathBuf = get_exe_path();
    let config_path = path.join("config.yml");
    String::from(if config_path.exists() {
        config_path.to_str().unwrap_or("./config.yml")
    } else {
        "./config.yml"
    })
}

pub(crate) fn get_default_mappings_path() -> String {
    let path: PathBuf = get_exe_path();
    let mappings_path = path.join("mapping.yml");
    String::from(if mappings_path.exists() {
        mappings_path.to_str().unwrap_or("./mapping.yml")
    } else {
        "./mapping.yml"
    })
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
                    println!("Path not found {:?}", &work_path);
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
                println!("Path not found {:?}", &rp);
                String::from("./")
            }
        }
    }
}

pub(crate) fn open_file(file_name: &PathBuf, mandatory: bool) -> Option<fs::File> {
    match fs::File::open(file_name) {
        Ok(file) => Some(file),
        Err(_) => {
            if mandatory {
                println!("cant open file: {:?}", file_name);
                std::process::exit(1);
            }
            None
        }
    }
}

pub(crate) fn get_input_content(working_dir: &String, url_str: &str, persist_file: Option<PathBuf>, verbose: bool) -> Option<Vec<String>> {
    match url_str.parse::<url::Url>() {
        Ok(url) => match download_content(url, persist_file, verbose) {
            Ok(content) => Some(content),
            Err(e) => {
                println!("cant download input url: {}  => {}", url_str, e);
                None
            }
        }
        Err(_) => {
            let file_path = get_file_path(working_dir, Some(PathBuf::from(url_str)));
            let result = match &file_path {
                Some(file) => {
                    if file.exists() {
                        if let Some(..) = persist_file {
                            let to_file = &persist_file.unwrap();
                            match fs::copy(file, to_file) {
                                Ok(_) => {}
                                Err(e) => println!("cant persist to: {}  => {}", to_file.to_str().unwrap_or("?"), e),
                            }
                        };
                        Some(std::io::BufReader::new(open_file(file, true).unwrap()).lines().map(|l| l.unwrap()).collect())
                    } else {
                        None
                    }
                }
                None => None
            };
            match result {
                Some(file) => Some(file),
                None => {
                    println!("cant read input url: {:?}", &file_path.unwrap());
                    None
                }
            }
        }
    }
}

fn download_content(url: url::Url, persist_file: Option<PathBuf>, verbose: bool) -> Result<Vec<String>, String> {
    match reqwest::blocking::get(url) {
        Ok(response) => {
            if response.status().is_success() {
                match response.text_with_charset("utf8") {
                    Ok(text) => {
                        persist_playlist(persist_file, &text, verbose);
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

fn persist_playlist(persist_file: Option<PathBuf>, text: &String, verbose: bool) {
    if let Some(path_buf) = persist_file {
        let filename = &path_buf.to_str().unwrap_or("?");
        match fs::File::create(&path_buf) {
            Ok(mut file) => match file.write_all(text.as_bytes()) {
                Ok(_) => if verbose { println!("persisted: {}", filename) },
                Err(e) => println!("failed to persist file {}, {}", filename, e)
            },
            Err(e) => println!("failed to persist file {}, {}", filename, e)
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
                        println!("path is not relative {:?}", e);
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

fn download_json_content(input: &ConfigInput, url: url::Url, persist_file: Option<PathBuf>, verbose: bool) -> Result<serde_json::Value, String> {
    let mut request = reqwest::blocking::Client::new().get(url);
    if input.headers.is_empty() {
        let mut headers = header::HeaderMap::new();
        for (key, value) in &input.headers {
            headers.insert(
                HeaderName::from_bytes(key.as_bytes()).unwrap(),
                HeaderValue::from_bytes(value.as_bytes()).unwrap(),
            );
        }
        if verbose { println!("Request with headers{:?}", &headers); }
        request = request.headers(headers);
    }
    match request.send() {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<serde_json::Value>() {
                    Ok(content) => {
                        persist_playlist(persist_file, &serde_json::to_string(&content).unwrap(), verbose);
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

pub(crate) fn get_input_json_content(input: &ConfigInput, url_str: &String, persist_file: Option<PathBuf>, verbose: bool) -> Option<serde_json::Value> {
    match url_str.parse::<url::Url>() {
        Ok(url) => match download_json_content(input, url, persist_file, verbose) {
            Ok(content) => Some(content),
            Err(e) => {
                println!("cant download input url: {}  => {}", url_str, e);
                None
            }
        },
        Err(_) => {
            println!("malformed input url: {}", url_str);
            None
        }
    }
}