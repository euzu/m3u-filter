use std::env;
use std::fs::File;
use std::path::PathBuf;

use chrono::Local;
use log::{debug, error, info, warn};
use regex::Regex;
use serde::Serialize;

use crate::{create_m3u_filter_error_result, handle_m3u_filter_error_result};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::api_proxy::ApiProxyConfig;
use crate::model::config::{Config, ConfigDto};
use crate::model::mapping::Mappings;
use crate::utils::{file_utils, multi_file_reader};

pub(crate) fn read_mappings(args_mapping: Option<String>, cfg: &mut Config) -> Result<(), M3uFilterError> {
    let mappings_file: String = args_mapping.unwrap_or(file_utils::get_default_mappings_path(cfg._config_path.as_str()));

    match read_mapping(mappings_file.as_str()) {
        Ok(mappings) => {
            info!("Mappings File: {}", &mappings_file);
            if mappings.is_none() { debug!("no mapping loaded"); }
            handle_m3u_filter_error_result!(M3uFilterErrorKind::Info, cfg.set_mappings(mappings));
            Ok(())
        }
        Err(err) => Err(err),
    }
}

pub(crate) fn read_api_proxy_config(args_api_proxy_config: Option<String>, cfg: &mut Config) {
    let api_proxy_config_file: String = args_api_proxy_config.unwrap_or(file_utils::get_default_api_proxy_config_path(cfg._config_path.as_str()));
    cfg._api_proxy_file_path = api_proxy_config_file.to_owned();
    let api_proxy_config = read_api_proxy(api_proxy_config_file.as_str(), true);
    match api_proxy_config {
        None => {
            warn!("cant read api_proxy_config file: {}", api_proxy_config_file.as_str());
        }
        Some(config) => {
            info!("Api Proxy File: {}", &api_proxy_config_file);
            cfg.set_api_proxy(Some(config));
        }
    }
}

pub(crate) fn read_config(config_path: &str, config_file: &str, sources_file: &str) -> Result<Config, M3uFilterError> {
    let files = vec![std::path::PathBuf::from(config_file), std::path::PathBuf::from(sources_file)];
    match multi_file_reader::MultiFileReader::new(&files) {
        Ok(file) => {
            match serde_yaml::from_reader::<_, Config>(file) {
                Ok(mut result) => {
                    result._config_path = config_path.to_string();
                    result._config_file_path = config_file.to_string();
                    result._sources_file_path = sources_file.to_string();
                    match result.prepare(true) {
                        Ok(_) => Ok(result),
                        Err(err) => Err(err)
                    }
                }
                Err(e) => {
                    create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant read config file: {}", e)
                }
            }
        }
        Err(err) => create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "{}", err)
    }
}

pub(crate) fn read_mapping(mapping_file: &str) -> Result<Option<Mappings>, M3uFilterError> {
    let mapping_file = std::path::PathBuf::from(mapping_file);
    match file_utils::open_file(&mapping_file) {
        Ok(file) => {
            let mapping: Result<Mappings, _> = serde_yaml::from_reader(file);
            match mapping {
                Ok(mut result) => {
                    handle_m3u_filter_error_result!(M3uFilterErrorKind::Info, result.prepare());
                    Ok(Some(result))
                }
                Err(err) => {
                    error!("cant read mapping file: {}", err);
                    Ok(None)
                }
            }
        }
        _ => {
            warn!("cant read mapping file: {}", mapping_file.to_str().unwrap_or("?"));
            Ok(None)
        }
    }
}

pub(crate) fn read_api_proxy(api_proxy_file: &str, resolve_var: bool) -> Option<ApiProxyConfig> {
    match file_utils::open_file(&std::path::PathBuf::from(api_proxy_file)) {
        Ok(file) => {
            let mapping: Result<ApiProxyConfig, _> = serde_yaml::from_reader(file);
            match mapping {
                Ok(mut result) => {
                    match result.prepare(resolve_var) {
                        Ok(_) => {
                            Some(result)
                        }
                        Err(err) => {
                            error!("cant read api-proxy-config file: {}", err);
                            None
                        }
                    }
                }
                Err(err) => {
                    error!("cant read api-proxy-config file: {}", err);
                    None
                }
            }
        }
        _ => None
    }
}

fn write_config_file<T>(file_path: &str, backup_dir: &str, config: &T, default_name: &str) -> Result<(), M3uFilterError>
    where
        T: ?Sized + Serialize {
    let path = PathBuf::from(file_path);
    let filename = path.file_name().map_or(default_name.to_string(), |f| f.to_string_lossy().to_string());
    let backup_path = PathBuf::from(backup_dir).join(format!("{}_{}", filename, Local::now().format("%Y%m%d_%H%M%S")));


    match std::fs::copy(&path, &backup_path) {
        Ok(_) => {}
        Err(err) => { error!("Could not backup file {}:{}", &backup_path.to_str().unwrap_or("?"), err) }
    }
    info!("Saving file to {}", &path.to_str().unwrap_or("?"));
    match File::create(&path) {
        Ok(f) => {
            serde_yaml::to_writer(f, &config).unwrap();
            Ok(())
        }
        Err(err) => create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Could not write file {}: {}", &path.to_str().unwrap_or("?"), err)
    }
}

pub(crate) fn save_api_proxy(file_path: &str, backup_dir: &str, config: &ApiProxyConfig) -> Result<(), M3uFilterError> {
    write_config_file(file_path, backup_dir, config, "api-proxy.yml")
}

pub(crate) fn save_main_config(file_path: &str, backup_dir: &str, config: &ConfigDto) -> Result<(), M3uFilterError> {
    write_config_file(file_path, backup_dir, config, "config.yml")
}

pub(crate) fn resolve_env_var(value: &str) -> String {
    if !value.trim().is_empty() {
        let pattern = Regex::new(r#"\$\{env:(?P<var>[a-zA-Z_][a-zA-Z0-9_]*)}"#).unwrap();
        if let Some(caps) = pattern.captures(value) {
            if let Some(var) = caps.name("var") {
                let var_name = var.as_str();
                return match env::var(var_name) {
                    Ok(resolved_val) => resolved_val, // If environment variable found, replace with its value
                    Err(_) => value.to_string()
                };
            }
        }
    }
    value.to_string()
}