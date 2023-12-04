use std::fs::{File};
use std::path::PathBuf;
use chrono::Local;
use log::{debug, error, info, warn};
use crate::model::api_proxy::ApiProxyConfig;
use crate::model::config::Config;
use crate::model::mapping::Mappings;
use crate::{create_m3u_filter_error_result, handle_m3u_filter_error_result, utils};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::multi_file_reader::MultiFileReader;

pub(crate) fn read_mappings(args_mapping: Option<String>, cfg: &mut Config) -> Result<(), M3uFilterError> {
    let mappings_file: String = args_mapping.unwrap_or(utils::get_default_mappings_path());

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
    let api_proxy_config_file: String = args_api_proxy_config.unwrap_or(utils::get_default_api_proxy_config_path());
    let api_proxy_config = read_api_proxy(api_proxy_config_file.as_str());
    match api_proxy_config {
        None => {
            warn!("cant read api_proxy_config file: {}", api_proxy_config_file.as_str());
        }
        Some(mut config) => {
            info!("Api Proxy File: {}", &api_proxy_config_file);
            config._file_path = api_proxy_config_file;
            cfg.set_api_proxy(Some(config));
        }
    }
}

pub(crate) fn read_config(config_file: &str, sources_file: &str) -> Result<Config, M3uFilterError> {
    let files = vec![std::path::PathBuf::from(config_file), std::path::PathBuf::from(sources_file)];
    match MultiFileReader::new(&files) {
        Ok(file) => {
            let mut cfg: Config = match serde_yaml::from_reader(file) {
                Ok(result) => result,
                Err(e) => {
                    return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant read config file: {}", e);
                }
            };
            match cfg.prepare() {
                Ok(_) => Ok(cfg),
                Err(err) => Err(err)
            }
        }
        Err(err) => create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "{}", err)
    }
}

pub(crate) fn read_mapping(mapping_file: &str) -> Result<Option<Mappings>, M3uFilterError> {
    let mapping_file = std::path::PathBuf::from(mapping_file);
    match utils::open_file(&mapping_file) {
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

pub(crate) fn read_api_proxy(api_proxy_file: &str) -> Option<ApiProxyConfig> {
    match utils::open_file(&std::path::PathBuf::from(api_proxy_file)) {
        Ok(file) => {
            let mapping: Result<ApiProxyConfig, _> = serde_yaml::from_reader(file);
            match mapping {
                Ok(mut result) => {
                    match result.prepare() {
                        Ok(_) => Some(result),
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

pub(crate) fn save_api_proxy(config: &ApiProxyConfig) -> Result<(), M3uFilterError>{
    let path = PathBuf::from(&config._file_path);
    let backup_path = PathBuf::from(format!("{}_{}", &config._file_path, Local::now().format("%Y%m%d_%H%M%S")));

    match std::fs::copy(&path, &backup_path) {
        Ok(_) => {}
        Err(err) => {error!("Could not backup file {}:{}", &backup_path.to_str().unwrap_or("?"), err)}
    }
    info!("Saving api proxy to {}", &path.to_str().unwrap_or("?"));
    match File::create(&path) {
        Ok(f) => {
            serde_yaml::to_writer(f, &config).unwrap();
            Ok(())
        }
        Err(err) => create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Could not write file {}: {}", &path.to_str().unwrap_or("?"), err)
    }

}
