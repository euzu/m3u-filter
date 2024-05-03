use std::fs;
use std::io::{Write};
use std::path::{Path, PathBuf};
use log::{debug, error};
use path_absolutize::*;

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

fn get_default_path(file: &str) -> String {
    let path: PathBuf = get_exe_path();
    let default_path = path.join(file);
    String::from(if default_path.exists() {
        default_path.to_str().unwrap_or(file)
    } else {
        file
    })
}

pub(crate) fn get_default_file_path(config_path: &str, file: &str) -> String {
    let path: PathBuf = PathBuf::from(config_path);
    let default_path = path.join(file);
    String::from(if default_path.exists() {
        default_path.to_str().unwrap_or(file)
    } else {
        file
    })
}

pub(crate) fn get_default_user_file_path(config_path: &str) -> String {
    get_default_file_path(config_path, "user.txt")
}

pub(crate) fn get_default_config_path() -> String {
    get_default_path("config")
}

pub(crate) fn get_default_config_file_path(config_path: &str) -> String {
    get_default_file_path(config_path, "config.yml")
}

pub(crate) fn get_default_sources_file_path(config_path: &str) -> String {
    get_default_file_path(config_path, "source.yml")
}

pub(crate) fn get_default_mappings_path(config_path: &str) -> String {
    get_default_file_path(config_path, "mapping.yml")
}

pub(crate) fn get_default_api_proxy_config_path(config_path: &str) -> String {
    get_default_file_path(config_path, "api-proxy.yml")
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

pub(crate) fn open_file(file_name: &Path) -> Result<fs::File, std::io::Error> {
    fs::File::open(file_name)
}

pub(crate) fn persist_file(persist_file: Option<PathBuf>, text: &String) {
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

pub(crate) fn add_prefix_to_filename(path: &Path, prefix: &str, ext: Option<&str>) -> PathBuf {
    let file_name = path.file_name().unwrap_or_default();
    let new_file_name = format!("{}{}", prefix, file_name.to_string_lossy());
    let result = path.with_file_name(new_file_name);
    match ext {
        None => result,
        Some(extension) => result.with_extension(extension)
    }
}

pub(crate) fn path_exists(file_path: &Path) -> bool {
    if let Ok(metadata) = fs::metadata(file_path) {
        return metadata.is_file();
    }
    false
}
