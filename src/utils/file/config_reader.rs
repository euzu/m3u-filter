use std::env;
use std::fs::File;
use std::path::PathBuf;
use std::sync::LazyLock;
use chrono::Local;
use log::{debug, error, info, warn};
use regex::Regex;
use serde::Serialize;
use crate::utils::sys_utils::exit;
use crate::m3u_filter_error::{to_io_error, M3uFilterError, M3uFilterErrorKind, create_m3u_filter_error, create_m3u_filter_error_result, info_err, handle_m3u_filter_error_result};
use crate::model::api_proxy::ApiProxyConfig;
use crate::model::config::{Config, ConfigDto};
use crate::model::mapping::Mappings;
use crate::utils::file::{file_utils, multi_file_reader};

pub fn read_mappings(args_mapping: Option<String>, cfg: &mut Config) -> Result<Option<String>, M3uFilterError> {
    let mappings_file: String = args_mapping.unwrap_or_else(|| file_utils::get_default_mappings_path(cfg.t_config_path.as_str()));

    match read_mapping(mappings_file.as_str()) {
        Ok(mappings) => {
            match mappings {
                None => {
                    debug!("no mapping loaded");
                    Ok(Some(mappings_file))
                }
                Some(mappings_cfg) => {
                    cfg.set_mappings(&mappings_cfg);
                    Ok(None)
                }
            }
        }
        Err(err) => Err(err),
    }
}

pub fn read_api_proxy_config(args_api_proxy_config: Option<String>, cfg: &mut Config) -> Option<String> {
    let api_proxy_config_file: String = args_api_proxy_config.unwrap_or_else(|| file_utils::get_default_api_proxy_config_path(cfg.t_config_path.as_str()));
    api_proxy_config_file.clone_into(&mut cfg.t_api_proxy_file_path);
    let api_proxy_config = read_api_proxy(cfg, api_proxy_config_file.as_str(), true);
    match api_proxy_config {
        None => {
            warn!("cant read api_proxy_config file: {}", api_proxy_config_file.as_str());
            None
        }
        Some(config) => {
            cfg.set_api_proxy(Some(config));
            Some(api_proxy_config_file)
        }
    }
}

pub fn read_config(config_path: &str, config_file: &str, sources_file: &str) -> Result<Config, M3uFilterError> {
    let files = vec![std::path::PathBuf::from(config_file), std::path::PathBuf::from(sources_file)];
    match multi_file_reader::MultiFileReader::new(&files) {
        Ok(file) => {
            match serde_yaml::from_reader::<_, Config>(file) {
                Ok(mut result) => {
                    result.t_config_path = config_path.to_string();
                    result.t_config_file_path = config_file.to_string();
                    result.t_sources_file_path = sources_file.to_string();
                    match result.prepare(true) {
                        Err(err) => Err(err),
                        _ => Ok(result),
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

pub fn read_mapping(mapping_file: &str) -> Result<Option<Mappings>, M3uFilterError> {
    let mapping_file = std::path::PathBuf::from(mapping_file);
    if let Ok(file) = file_utils::open_file(&mapping_file) {
        let mapping: Result<Mappings, _> = serde_yaml::from_reader(file);
        match mapping {
            Ok(mut result) => {
                handle_m3u_filter_error_result!(M3uFilterErrorKind::Info, result.prepare());
                return Ok(Some(result));
            },
            Err(err) => {
                return Err(info_err!(err.to_string()));
            }
        }
    }
    warn!("cant read mapping file: {}", mapping_file.to_str().unwrap_or("?"));
    Ok(None)
}

pub fn read_api_proxy(config: &Config, api_proxy_file: &str, resolve_var: bool) -> Option<ApiProxyConfig> {
    file_utils::open_file(&std::path::PathBuf::from(api_proxy_file)).map_or(None, |file| {
            let mapping: Result<ApiProxyConfig, _> = serde_yaml::from_reader(file);
            match mapping {
                Ok(mut result) => {
                    match result.prepare(config, resolve_var) {
                        Err(err) => {
                            exit!("cant read api-proxy-config file: {}", err);
                        }
                        _ => {
                            Some(result)
                        }
                    }
                }
                Err(err) => {
                    error!("cant read api-proxy-config file: {}", err);
                    None
                }
            }
        })
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

    File::create(&path)
        .and_then(|f| serde_yaml::to_writer(f, &config).map_err(to_io_error))
        .map_err(|err| create_m3u_filter_error!(M3uFilterErrorKind::Info, "Could not write file {}: {}", &path.to_str().unwrap_or("?"), err))


}

pub fn save_api_proxy(file_path: &str, backup_dir: &str, config: &ApiProxyConfig) -> Result<(), M3uFilterError> {
    write_config_file(file_path, backup_dir, config, "api-proxy.yml")
}

pub fn save_main_config(file_path: &str, backup_dir: &str, config: &ConfigDto) -> Result<(), M3uFilterError> {
    write_config_file(file_path, backup_dir, config, "config.yml")
}

static ENV_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| Regex::new(r"\$\{env:(?P<var>[a-zA-Z_][a-zA-Z0-9_]*)}").unwrap());

pub fn resolve_env_var(value: &str) -> String {
    ENV_REGEX.replace_all(value, |caps: &regex::Captures| {
        let var_name = &caps["var"];
        env::var(var_name).unwrap_or_else(|_| format!("${{env:{var_name}}}"))
    }).to_string()
}

#[cfg(test)]
mod tests {
    use crate::utils::file::config_reader::resolve_env_var;

    #[test]
    fn test_resolve() {
       let resolved =  resolve_env_var("${env:HOME}");
        assert_eq!(resolved, std::env::var("HOME").unwrap());
    }
}