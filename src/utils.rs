use url::{Url};
use std::io::{BufRead, Write};
use std::path::{PathBuf};

pub fn get_default_config_path() -> String {
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

pub fn open_file(file_name: &str) -> std::fs::File {
    let file = match std::fs::File::open(file_name) {
        Ok(file) => file,
        Err(_) => {
            println!("cant open file: {}", file_name);
            std::process::exit(1);
        }
    };
    file
}

pub fn get_input_content(url_str: &str, persist_file: Option<PathBuf>) -> Vec<String> {
    match url_str.parse::<Url>() {
        Ok(url) => match download_content(url, persist_file) {
            Ok(content) => content,
            Err(e) => {
                println!("cant download input url: {}  => {}", url_str, e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            let file = std::path::Path::new(url_str);
            if file.exists() {
                if persist_file.is_some() {
                    let to_file = &persist_file.unwrap();
                    match std::fs::copy(file, to_file) {
                        Ok(_) => {},
                        Err(e) => println!("cant persist to: {}  => {}", to_file.to_str().unwrap_or("?"), e),
                    }
                } ;
                std::io::BufReader::new(open_file(url_str)).lines(). map(|l| l.unwrap()).collect()
            } else {
                println!("cant read input url: {}  => {}", url_str, e);
                std::process::exit(1);
            }
        }
    }
}

fn download_content(url: Url, persist_file: Option<PathBuf>) -> Result<Vec<String>, String> {
    match reqwest::blocking::get(url) {
        Ok(response) => {
            if response.status().is_success() {
                match response.text_with_charset("utf8") {
                    Ok(text) => {
                        persists_playlist(persist_file, &text);
                        let result= text.lines().map(|l| String::from(l)).collect();
                        Ok(result)
                    },
                    Err(e) => Err(e.to_string())
                }
            } else {
                Err(format!("Request failed: {}", response.status()))
            }
        },
        Err(e) => Err(e.to_string())
    }
}

fn persists_playlist(persist_file: Option<PathBuf>, text: &String) {
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
            },
            None => {}
    }
}
