use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::{PathBuf};
use log::{debug, error, Level, log_enabled};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use crate::create_m3u_filter_error_result;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{ConfigInput};
use crate::utils::file_utils::{get_file_path, open_file, persist_file};
use reqwest::header::CONTENT_ENCODING;
use flate2::read::{GzDecoder, ZlibDecoder};

pub(crate) fn bytes_to_megabytes(bytes: u64) -> u64 {
    bytes / 1_048_576
}

pub(crate) async fn get_input_text_content(input: &ConfigInput, working_dir: &String, url_str: &str, persist_filepath: Option<PathBuf>) -> Result<String, M3uFilterError> {
    if log_enabled!(Level::Debug) {
        debug!("getting input text content working_dir: {}, url: {}", working_dir, url_str);
    }

    if let Ok(url) = url_str.parse::<url::Url>() {
        match download_text_content(input, url, persist_filepath).await {
            Ok(content) => Ok(content),
            Err(e) => {
                error!("cant download input url: {}  => {}", url_str, e);
                create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Failed to download")
            }
        }
    } else {
        let result = match get_file_path(working_dir, Some(PathBuf::from(url_str))) {
            Some(filepath) => {
                if filepath.exists() {
                    if let Some(persist_file_value) = persist_filepath {
                        let to_file = &persist_file_value;
                        match fs::copy(&filepath, to_file) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("cant persist to: {}  => {}", to_file.to_str().unwrap_or("?"), e);
                                return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Failed to persist: {}  => {}", to_file.to_str().unwrap_or("?"), e);
                            }
                        }
                    };
                    match open_file(&filepath) {
                        Ok(file) => {
                            let mut content = String::new();
                            match std::io::BufReader::new(file).read_to_string(&mut content) {
                                Ok(_) => Some(content),
                                Err(err) => {
                                    let file_str = &filepath.to_str().unwrap_or("?");
                                    error!("cant read file: {} {}", file_str,  err);
                                    return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Cant open file : {}  => {}", file_str,  err);
                                }
                            }
                        }
                        Err(err) => {
                            let file_str = &filepath.to_str().unwrap_or("?");
                            error!("cant read file: {} {}", file_str,  err);
                            return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Cant open file : {}  => {}", file_str,  err);
                        }
                    }
                } else {
                    None
                }
            }
            None => None
        };
        if let Some(content) = result {
            Ok(content)
        } else {
            let msg = format!("cant read input url: {url_str:?}");
            error!("{}", msg);
            create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "{}", msg)
        }
    }
}

pub(crate) fn get_client_request(input: Option<&ConfigInput>, url: url::Url, custom_headers: Option<&HashMap<&str, &[u8]>>) -> reqwest::RequestBuilder {
    let mut request = reqwest::Client::new().get(url);
    let headers = get_request_headers(input.map_or(&HashMap::new(), |i| &i.headers), custom_headers);
    request = request.headers(headers);
    request
}

pub(crate) fn get_request_headers(defined_headers: &HashMap<String, String>, custom_headers: Option<&HashMap<&str, &[u8]>>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (key, value) in defined_headers {
        headers.insert(
            HeaderName::from_bytes(key.as_bytes()).unwrap(),
            HeaderValue::from_bytes(value.as_bytes()).unwrap());
    }
    if let Some(custom) = custom_headers {
        let header_keys: HashSet<String> = headers.keys().map(|k| k.as_str().to_lowercase()).collect();
        for (key, value) in custom {
            let key_lc = key.to_lowercase();
            if !("host" == key_lc || header_keys.contains(key_lc.as_str())) {
                headers.insert(
                    HeaderName::from_bytes(key.as_bytes()).unwrap(),
                    HeaderValue::from_bytes(value).unwrap());
            } else if log_enabled!(Level::Debug) {
                debug!("Ignoring request header {}={}", key_lc, String::from_utf8_lossy(value));
            }
        }
    }
    if log_enabled!(Level::Debug) {
        let he: HashMap<String, String> = headers.iter().map(|(k, v)| (k.to_string(), String::from_utf8_lossy(v.as_bytes()).to_string())).collect();
        debug!("Request headers {:?}", he);
    }
    headers
}

async fn download_json_content(input: &ConfigInput, url: url::Url, persist_filepath: Option<PathBuf>) -> Result<serde_json::Value, String> {
    if log_enabled!(Level::Debug) {
        debug!("downloading json content from {}", url.to_string());
    }
    let request = get_client_request(Some(input), url, None);
    match request.send().await {
        Ok(response) => {
            if log_enabled!(Level::Debug) {
                debug!("downloading json content response code: {}", response.status().as_str());
            }
            if response.status().is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(content) => {
                        if persist_filepath.is_some() {
                            persist_file(persist_filepath, &serde_json::to_string(&content).unwrap());
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

pub(crate) async fn get_input_json_content(input: &ConfigInput, url_str: &str, persist_filepath: Option<PathBuf>) -> Result<serde_json::Value, M3uFilterError> {
    match url_str.parse::<url::Url>() {
        Ok(url) => match download_json_content(input, url, persist_filepath).await {
            Ok(content) => Ok(content),
            Err(e) => create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "cant download input url: {}  => {}", url_str, e)
        },
        Err(_) => create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "malformed input url: {}", url_str)
    }
}

async fn download_text_content(input: &ConfigInput, url: url::Url, persist_filepath: Option<PathBuf>) -> Result<String, std::io::Error> {
    let request = get_client_request(Some(input), url, None);
    let result = match request.send().await {
        Ok(response) => {
            let is_success = response.status().is_success();
            if is_success {
                let header_value = response.headers().get(CONTENT_ENCODING);
                let encoding = if let Some(encoding_header) = header_value {
                    match encoding_header.to_str() {
                        Ok(value) => Some(value.to_string()),
                        Err(_) => None,
                    }
                } else {
                    None
                };
                match response.bytes().await {
                    Ok(bytes) => {
                        let mut decode_buffer = String::new();
                        if let Some(encoding_type) = encoding {
                            match encoding_type.as_str() {
                                "gzip" => {
                                    let mut decoder = GzDecoder::new(&bytes[..]);
                                    match decoder.read_to_string(&mut decode_buffer) {
                                        Ok(_) => {}
                                        Err(err) => return Err(std::io::Error::new(ErrorKind::Other, format!("failed to decode gzip content {err}")))
                                    };
                                }
                                "deflate" => {
                                    let mut decoder = ZlibDecoder::new(&bytes[..]);
                                    match decoder.read_to_string(&mut decode_buffer) {
                                        Ok(_) => {}
                                        Err(err) => return Err(std::io::Error::new(ErrorKind::Other, format!("failed to decode zlib content {err}")))
                                    }
                                }
                                _ => {}
                            };
                        }

                        if decode_buffer.is_empty() {
                            match String::from_utf8(bytes.to_vec()) {
                                Ok(decoded_content) => Ok(decoded_content),
                                Err(err) => Err(std::io::Error::new(ErrorKind::Other, format!("failed to plain text content {err}")))
                            }
                        } else {
                            Ok(decode_buffer)
                        }
                    }
                    Err(err) => Err(std::io::Error::new(ErrorKind::Other, format!("failed to read response {err}")))
                }
            } else {
                Err(std::io::Error::new(ErrorKind::Other, format!("Request failed with status {}", response.status())))
            }
        }
        Err(err) => Err(std::io::Error::new(ErrorKind::Other, format!("Request failed {err}")))
    };
    match result {
        Ok(content) => {
            if persist_filepath.is_some() {
                persist_file(persist_filepath, &content);
            }
            Ok(content)
        }
        Err(err) => Err(err)
    }
}