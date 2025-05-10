use crate::tuliprox_error::{create_tuliprox_error, create_tuliprox_error_result, handle_tuliprox_error_result, info_err, str_to_io_error, to_io_error, TuliProxError, TuliProxErrorKind};
use crate::model::ApiProxyConfig;
use crate::model::{Config, ConfigDto, ConfigInput, ConfigInputAlias, InputType};
use crate::model::Mappings;
use crate::utils::CONSTANTS;
use crate::utils::env_resolving_reader::EnvResolvingReader;
use crate::utils::file_utils::{file_reader, resolve_relative_path};
use crate::utils::{file_utils, multi_file_reader};
use crate::utils::request::{get_credentials_from_url, get_local_file_content};
use crate::utils::sys_utils::exit;
use chrono::Local;
use log::{debug, error, info, warn};
use serde::Serialize;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, Error, Read};
use std::path::PathBuf;
use url::Url;

enum EitherReader<L, R> {
    Left(L),
    Right(R),
}

// `Read`-Trait für Either implementieren
impl<L: Read, R: Read> Read for EitherReader<L, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            EitherReader::Left(reader) => reader.read(buf),
            EitherReader::Right(reader) => reader.read(buf),
        }
    }
}

pub fn config_file_reader(file: File, resolve_env: bool) -> impl Read
{
    if resolve_env {
        EitherReader::Left(EnvResolvingReader::new(file_reader(file)))
    } else {
        EitherReader::Right(BufReader::new(file))
    }
}

pub fn read_mappings(args_mapping: Option<String>, cfg: &mut Config, resolve_env: bool) -> Result<Option<String>, TuliProxError> {
    let mappings_file: String = args_mapping.unwrap_or_else(|| file_utils::get_default_mappings_path(cfg.t_config_path.as_str()));

    match read_mapping(mappings_file.as_str(), resolve_env) {
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

pub async fn read_api_proxy_config(args_api_proxy_config: Option<String>, cfg: &mut Config) -> Result<Option<String>, TuliProxError> {
    let api_proxy_config_file: String = args_api_proxy_config.unwrap_or_else(|| file_utils::get_default_api_proxy_config_path(cfg.t_config_path.as_str()));
    api_proxy_config_file.clone_into(&mut cfg.t_api_proxy_file_path);
    let api_proxy_config = read_api_proxy(cfg, api_proxy_config_file.as_str(), true);
    match api_proxy_config {
        None => {
            warn!("cant read api_proxy_config file: {}", api_proxy_config_file.as_str());
            Ok(None)
        }
        Some(config) => {
            cfg.set_api_proxy(Some(config)).await?;
            Ok(Some(api_proxy_config_file))
        }
    }
}

pub fn read_config(config_path: &str, config_file: &str, sources_file: &str, include_computed: bool) -> Result<Config, TuliProxError> {
    let files = vec![std::path::PathBuf::from(config_file), std::path::PathBuf::from(sources_file)];
    match multi_file_reader::MultiFileReader::new(&files) {
        Ok(reader) => {
            match serde_yaml::from_reader::<_, Config>(reader) {
                Ok(mut result) => {
                    result.t_config_path = config_path.to_string();
                    result.t_config_file_path = config_file.to_string();
                    result.t_sources_file_path = sources_file.to_string();
                    match result.prepare(include_computed) {
                        Err(err) => Err(err),
                        _ => Ok(result),
                    }
                }
                Err(e) => {
                    create_tuliprox_error_result!(TuliProxErrorKind::Info, "cant read config file: {}", e)
                }
            }
        }
        Err(err) => create_tuliprox_error_result!(TuliProxErrorKind::Info, "{}", err)
    }
}

pub fn read_mapping(mapping_file: &str, resolve_var: bool) -> Result<Option<Mappings>, TuliProxError> {
    let mapping_file = std::path::PathBuf::from(mapping_file);
    if let Ok(file) = file_utils::open_file(&mapping_file) {
        let mapping: Result<Mappings, _> = serde_yaml::from_reader(config_file_reader(file, resolve_var));
        return match mapping {
            Ok(mut result) => {
                handle_tuliprox_error_result!(TuliProxErrorKind::Info, result.prepare());
                Ok(Some(result))
            }
            Err(err) => {
                Err(info_err!(err.to_string()))
            }
        };
    }
    warn!("cant read mapping file: {}", mapping_file.to_str().unwrap_or("?"));
    Ok(None)
}

pub fn read_api_proxy(config: &Config, api_proxy_file: &str, resolve_env: bool) -> Option<ApiProxyConfig> {
    file_utils::open_file(&std::path::PathBuf::from(api_proxy_file)).map_or(None, |file| {
        let mapping: Result<ApiProxyConfig, _> = serde_yaml::from_reader(config_file_reader(file, resolve_env));
        match mapping {
            Ok(mut result) => {
                match result.prepare(config) {
                    Err(err) => {
                        exit!("cant read api-proxy-config file: {err}");
                    }
                    _ => {
                        Some(result)
                    }
                }
            }
            Err(err) => {
                error!("cant read api-proxy-config file: {err}");
                None
            }
        }
    })
}

fn write_config_file<T>(file_path: &str, backup_dir: &str, config: &T, default_name: &str) -> Result<(), TuliProxError>
where
    T: ?Sized + Serialize,
{
    let path = PathBuf::from(file_path);
    let filename = path.file_name().map_or(default_name.to_string(), |f| f.to_string_lossy().to_string());
    let backup_path = PathBuf::from(backup_dir).join(format!("{filename}_{}", Local::now().format("%Y%m%d_%H%M%S")));


    match std::fs::copy(&path, &backup_path) {
        Ok(_) => {}
        Err(err) => { error!("Could not backup file {}:{}", &backup_path.to_str().unwrap_or("?"), err) }
    }
    info!("Saving file to {}", &path.to_str().unwrap_or("?"));

    File::create(&path)
        .and_then(|f| serde_yaml::to_writer(f, &config).map_err(to_io_error))
        .map_err(|err| create_tuliprox_error!(TuliProxErrorKind::Info, "Could not write file {}: {}", &path.to_str().unwrap_or("?"), err))
}

pub fn save_api_proxy(file_path: &str, backup_dir: &str, config: &ApiProxyConfig) -> Result<(), TuliProxError> {
    write_config_file(file_path, backup_dir, config, "api-proxy.yml")
}

pub fn save_main_config(file_path: &str, backup_dir: &str, config: &ConfigDto) -> Result<(), TuliProxError> {
    write_config_file(file_path, backup_dir, config, "config.yml")
}

pub fn resolve_env_var(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    CONSTANTS.re_env_var.replace_all(value, |caps: &regex::Captures| {
        let var_name = &caps["var"];
        env::var(var_name).unwrap_or_else(|e| {
            error!("Could not resolve env var '{var_name}': {e}");
            format!("${{env:{var_name}}}")
        })
    }).to_string()
}

const CSV_SEPARATOR: char = ';';
const HEADER_PREFIX: char = '#';
const FIELD_MAX_CON: &str = "max_connections";
const FIELD_PRIO: &str = "priority";
const FIELD_URL: &str = "url";
const FIELD_NAME: &str = "name";
const FIELD_USERNAME: &str = "username";
const FIELD_PASSWORD: &str = "password";
const FIELD_UNKNOWN: &str = "?";
const DEFAULT_COLUMNS: &[&str] = &[FIELD_URL, FIELD_MAX_CON, FIELD_PRIO, FIELD_NAME, FIELD_USERNAME, FIELD_PASSWORD];

fn csv_assign_mandatory_fields(alias: &mut ConfigInputAlias, input_type: InputType) {
    if !alias.url.is_empty() {
        match Url::parse(alias.url.as_str()) {
            Ok(url) => {
                let (username, password) = get_credentials_from_url(&url);
                if username.is_none() || password.is_none() {
                    // xtream url
                    if input_type == InputType::XtreamBatch {
                        alias.url = url.origin().ascii_serialization().to_string();
                    } else if input_type == InputType::M3uBatch && alias.username.is_some() && alias.password.is_some() {
                        alias.url = format!("{}/get_php?username={}&password={}&type=m3u_plus",
                                            url.origin().ascii_serialization(),
                                            alias.username.as_deref().unwrap_or(""),
                                            alias.password.as_deref().unwrap_or("")
                        );
                    }
                } else {
                    if input_type == InputType::XtreamBatch {
                        alias.url = url.origin().ascii_serialization().to_string();
                    }
                    // m3u url
                    alias.username = username;
                    alias.password = password;
                }

                if alias.name.is_empty() {
                    let username = alias.username.as_deref().unwrap_or_default();
                    let domain: Vec<&str> = url.domain().unwrap_or_default().split('.').collect();
                    if domain.len() > 1 {
                        alias.name = format!("{}_{username}", domain[domain.len() - 2]);
                    } else {
                        alias.name = username.to_string();
                    }
                }
            }
            Err(_err) => {}
        }
    }
}

fn csv_assign_config_input_column(config_input: &mut ConfigInputAlias, header: &str, raw_value: &str) -> Result<(), io::Error> {
    let value = raw_value.trim();
    if !value.is_empty() {
        match header {
            FIELD_URL => {
                let url = Url::parse(value.trim()).map_err(to_io_error)?;
                config_input.url = url.to_string();
            }
            FIELD_MAX_CON => {
                let max_connections = value.parse::<u16>().unwrap_or(1);
                config_input.max_connections = max_connections;
            }
            FIELD_PRIO => {
                let priority = value.parse::<i16>().unwrap_or(0);
                config_input.priority = priority;
            }
            FIELD_NAME => {
                config_input.name = value.to_string();
            }
            FIELD_USERNAME => {
                config_input.username = Some(value.to_string());
            }
            FIELD_PASSWORD => {
                config_input.password = Some(value.to_string());
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn csv_read_inputs_from_reader(batch_input_type: InputType, reader: impl BufRead) -> Result<Vec<ConfigInputAlias>, io::Error> {
    let input_type = match batch_input_type {
        InputType::M3uBatch | InputType::M3u => InputType::M3uBatch,
        InputType::XtreamBatch | InputType::Xtream => InputType::XtreamBatch
    };
    let mut result = vec![];
    let mut default_columns = vec![];
    default_columns.extend_from_slice(DEFAULT_COLUMNS);
    let mut header_defined = false;
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        if line.starts_with(HEADER_PREFIX) {
            if !header_defined {
                header_defined = true;
                default_columns = line[1..].split(CSV_SEPARATOR).map(|s| {
                    match s {
                        FIELD_URL => FIELD_URL,
                        FIELD_MAX_CON => FIELD_MAX_CON,
                        FIELD_PRIO => FIELD_PRIO,
                        FIELD_NAME => FIELD_NAME,
                        FIELD_USERNAME => FIELD_USERNAME,
                        FIELD_PASSWORD => FIELD_PASSWORD,
                        _ => {
                            error!("Field {s} is unsupported for csv input");
                            FIELD_UNKNOWN
                        }
                    }
                }).collect();
            }
            continue;
        }

        let mut config_input = ConfigInputAlias {
            id: 0,
            name: String::new(),
            url: String::new(),
            username: None,
            password: None,
            priority: 0,
            max_connections: 1,
            t_base_url: String::default(),
        };

        let columns: Vec<&str> = line.split(CSV_SEPARATOR).collect();
        for (&header, &value) in default_columns.iter().zip(columns.iter()) {
            if let Err(err) = csv_assign_config_input_column(&mut config_input, header, value) {
                error!("Could not parse input line: {line} err: {err}");
            }
        }
        csv_assign_mandatory_fields(&mut config_input, input_type);
        result.push(config_input);
    }
    Ok(result)
}


pub fn csv_read_inputs(input: &ConfigInput) -> Result<Vec<ConfigInputAlias>, io::Error> {
    let file_uri = input.url.to_string();
    let file_path = get_csv_file_path(&file_uri)?;
    match get_local_file_content(&file_path) {
        Ok(content) => {
            csv_read_inputs_from_reader(input.input_type, EnvResolvingReader::new(file_reader(Cursor::new(content))))
        }
        Err(err) => {
            Err(err)
        }
    }
}

fn get_csv_file_path(file_uri: &String) -> Result<PathBuf, Error> {
    if file_uri.contains("://") {
        if let Ok(url) = file_uri.parse::<url::Url>() {
            if url.scheme() == "file" {
                return match url.to_file_path() {
                    Ok(path) => Ok(path),
                    Err(()) => Err(str_to_io_error(&format!("Could not open {file_uri}"))),
                };
            }
        }
        Err(str_to_io_error(&format!("Only file:// is supported {file_uri}")))
    } else {
        resolve_relative_path(file_uri)
    }
}

#[cfg(test)]
mod tests {
    use crate::model::InputType;
    use crate::utils::config_reader::{csv_read_inputs_from_reader, resolve_env_var};
    use std::io::{BufReader, Cursor};
    const M3U_BATCH: &str = r#"
#url;name;max_connections;priority
http://hd.providerline.com:8080/get.php?username=user1&password=user1&type=m3u_plus;input_1
http://hd.providerline.com/get.php?username=user2&password=user2&type=m3u_plus;input_2;1;2
http://hd.providerline.com/get.php?username=user3&password=user3&type=m3u_plus;input_3;1;2
http://hd.providerline.com/get.php?username=user4&password=user4&type=m3u_plus;input_4
"#;

    const XTREAM_BATCH: &str = r#"
#name;username;password;url;max_connections
input_1;desanocra;eyCG8SN523KQ;http://provider_1.tv:80;1
input_2;desanocra;eyCG8SN523KQ;http://provider_2.tv:8080;1
"#;

    #[test]
    fn test_read_inputs_xtream_as_m3u() {
        let reader = BufReader::new(Cursor::new(XTREAM_BATCH));
        let result = csv_read_inputs_from_reader(InputType::M3uBatch, reader);
        assert_eq!(result.is_ok(), true);
        let aliases = result.unwrap();
        assert_eq!(aliases.is_empty(), false);
        for config in aliases {
            assert_eq!(config.url.contains("username"), true);
        }
    }

    #[test]
    fn test_read_inputs_m3u_as_m3u() {
        let reader = BufReader::new(Cursor::new(M3U_BATCH));
        let result = csv_read_inputs_from_reader(InputType::M3uBatch, reader);
        assert_eq!(result.is_ok(), true);
        let aliases = result.unwrap();
        assert_eq!(aliases.is_empty(), false);
        for config in aliases {
            assert_eq!(config.url.contains("username"), true);
        }
    }

    #[test]
    fn test_read_inputs_xtream_as_xtream() {
        let reader = BufReader::new(Cursor::new(XTREAM_BATCH));
        let result = csv_read_inputs_from_reader(InputType::XtreamBatch, reader);
        assert_eq!(result.is_ok(), true);
        let aliases = result.unwrap();
        assert_eq!(aliases.is_empty(), false);
        for config in aliases {
            assert_eq!(config.url.contains("username"), false);
        }
    }

    #[test]
    fn test_read_inputs_m3u_as_xtream() {
        let reader = BufReader::new(Cursor::new(M3U_BATCH));
        let result = csv_read_inputs_from_reader(InputType::XtreamBatch, reader);
        assert_eq!(result.is_ok(), true);
        let aliases = result.unwrap();
        assert_eq!(aliases.is_empty(), false);
        for config in aliases {
            assert_eq!(config.url.contains("username"), false);
        }
    }

    #[test]
    fn test_resolve() {
        let resolved = resolve_env_var("${env:HOME}");
        assert_eq!(resolved, std::env::var("HOME").unwrap());
    }
}