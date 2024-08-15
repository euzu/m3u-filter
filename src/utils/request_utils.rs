use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Error, ErrorKind, Read};
use std::path::{PathBuf};
use log::{debug, error, Level, log_enabled};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use crate::create_m3u_filter_error_result;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{ConfigInput};
use crate::utils::file_utils::{get_file_path, open_file, persist_file};
use reqwest::header::CONTENT_ENCODING;
use flate2::read::{GzDecoder, ZlibDecoder};

fn is_gzip(bytes: &[u8]) -> bool {
    // Gzip files start with the bytes 0x1F 0x8B
    bytes.len() >= 2 && bytes[0] == 0x1F && bytes[1] == 0x8B
}

pub(crate) fn bytes_to_megabytes(bytes: u64) -> u64 {
    bytes / 1_048_576
}

pub(crate) async fn get_input_text_content(input: &ConfigInput, working_dir: &String, url_str: &str, persist_filepath: Option<PathBuf>) -> Result<String, M3uFilterError> {
    if log_enabled!(Level::Debug) {
        debug!("getting input text content working_dir: {}, url: {}", working_dir, url_str);
    }

    if url_str.parse::<url::Url>().is_ok() {
        match download_text_content(input, url_str, persist_filepath).await {
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

pub(crate) async fn download_text_content(input: &ConfigInput, url_str: &str, persist_filepath: Option<PathBuf>) -> Result<String, Error> {
    if let Ok(url) = url_str.parse::<url::Url>() {
        let request = get_client_request(Some(input), url, None);
        let result = match request.send().await {
            Ok(response) => {
                let is_success = response.status().is_success();
                if is_success {
                    let header_value = response.headers().get(CONTENT_ENCODING);
                    let mut encoding = if let Some(encoding_header) = header_value {
                        match encoding_header.to_str() {
                            Ok(value) => Some(value.to_string()),
                            Err(_) => None,
                        }
                    } else {
                        None
                    };
                    match response.bytes().await {
                        Ok(bytes) => {
                            if bytes.len() >= 2 && is_gzip(&bytes[0..2]) {
                                encoding = Some("gzip".to_string());
                            }
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
    } else {
        Err(std::io::Error::new(ErrorKind::Other, format!("Malformed URL {url_str}")))
    }
}

async fn download_json_content(input: &ConfigInput, url: &str, persist_filepath: Option<PathBuf>) -> Result<serde_json::Value, Error> {
    if log_enabled!(Level::Debug) {
        debug!("downloading json content from {url}");
    }
    match download_text_content(input, url, persist_filepath).await {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(value) => Ok(value),
                Err(err) => Err(Error::new(ErrorKind::Other, format!("Failed to parse json {err}")))
            }
        }
        Err(err) => Err(err)
    }
}

pub(crate) async fn get_input_json_content(input: &ConfigInput, url: &str, persist_filepath: Option<PathBuf>) -> Result<serde_json::Value, M3uFilterError> {
    match download_json_content(input, url, persist_filepath).await {
        Ok(content) => Ok(content),
        Err(e) => create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "cant download input url: {url}  => {}", e)
    }
}
