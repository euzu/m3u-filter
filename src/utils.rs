use std::io::{BufRead, Write};
use std::path::PathBuf;
use path_absolutize::*;

pub(crate) fn get_exe_path() -> std::path::PathBuf {
    let default_path = std::path::PathBuf::from("./");
    let current_exe = std::env::current_exe();
    let path: std::path::PathBuf = match current_exe {
        Ok(exe) => exe.parent().map_or(default_path, |p| p.to_path_buf()),
        Err(_) => default_path
    };
    path
}

pub(crate) fn get_default_config_path() -> String {
    let path: std::path::PathBuf = get_exe_path();
    let config_path = path.join("config.yml");
    String::from(if config_path.exists() {
        config_path.to_str().unwrap_or("./config.yml")
    } else {
        "./config.yml"
    })
}

pub(crate) fn get_working_path(wd: &String) -> String {
    let current_dir = std::env::current_dir().unwrap();
    if wd.is_empty() {
        String::from(current_dir.to_str().unwrap_or("."))
    } else {
        let work_path = std::path::PathBuf::from(wd);
        let wdpath = match std::fs::metadata(&work_path) {
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
        match rp.canonicalize()  {
            Ok(ap) => String::from(ap.to_str().unwrap_or("./")),
            Err(_) => {
                println!("Path not found {:?}", &rp);
                String::from("./")
            }
        }
    }
}

pub(crate) fn open_file(file_name: &PathBuf) -> std::fs::File {
    let file = match std::fs::File::open(file_name) {
        Ok(file) => file,
        Err(_) => {
            println!("cant open file: {:?}", file_name);
            std::process::exit(1);
        }
    };
    file
}

pub(crate) fn get_input_content(working_dir: &String, url_str: &str, persist_file: Option<std::path::PathBuf>) -> Option<Vec<String>> {
    match url_str.parse::<url::Url>() {
        Ok(url) => match download_content(url, persist_file) {
            Ok(content) => Some(content),
            Err(e) => {
                println!("cant download input url: {}  => {}", url_str, e);
                None
            }
        }
        Err(_) => {
            let file_path = get_file_path(&working_dir, Some(PathBuf::from(url_str)));
            let result = match &file_path {
                Some(file) => {
                    if file.exists() {
                        if persist_file.is_some() {
                            let to_file = &persist_file.unwrap();
                            match std::fs::copy(file, to_file) {
                                Ok(_) => {}
                                Err(e) => println!("cant persist to: {}  => {}", to_file.to_str().unwrap_or("?"), e),
                            }
                        };
                        Some(std::io::BufReader::new(open_file(file)).lines().map(|l| l.unwrap()).collect())
                    } else {
                        None
                    }
                },
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

fn download_content(url: url::Url, persist_file: Option<PathBuf>) -> Result<Vec<String>, String> {
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

fn persist_playlist(persist_file: Option<PathBuf>, text: &String) {
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