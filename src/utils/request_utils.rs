use crate::Arc;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Error, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Instant;

use flate2::read::{GzDecoder, ZlibDecoder};
use futures::StreamExt;
use log::{debug, error, log_enabled, trace, Level};
use regex::Regex;
use reqwest::header::CONTENT_ENCODING;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use url::Url;

use crate::m3u_filter_error::{str_to_io_error, M3uFilterError, M3uFilterErrorKind};
use crate::model::config::ConfigInput;
use crate::model::stats::format_elapsed_time;
use crate::repository::storage::get_input_storage_path;
use crate::repository::xtream_repository::FILE_EPG;
use crate::utils::compression_utils::{is_deflate, is_gzip, ENCODING_DEFLATE, ENCODING_GZIP};
use crate::utils::file_utils::{get_file_path, persist_file};
use crate::{create_m3u_filter_error_result, debug_if_enabled};

pub const fn bytes_to_megabytes(bytes: u64) -> u64 {
    bytes / 1_048_576
}

pub async fn get_input_text_content_as_file(client: Arc<reqwest::Client>, input: &ConfigInput, working_dir: &str, url_str: &str, persist_filepath: Option<PathBuf>) -> Result<PathBuf, M3uFilterError> {
    debug_if_enabled!("getting input text content working_dir: {}, url: {}", working_dir, mask_sensitive_info(url_str));
    if url_str.parse::<url::Url>().is_ok() {
        match download_text_content_as_file(client, input, url_str, working_dir, persist_filepath).await {
            Ok(content) => Ok(content),
            Err(e) => {
                error!("cant download input url: {}  => {}", mask_sensitive_info(url_str), mask_sensitive_info(e.to_string().as_str()));
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

                    if filepath.exists() {
                        Some(filepath)
                    } else {
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Failed: file does not exists {filepath:?}");
                    }
                } else {
                    None
                }
            }
            None => None
        };

        result.map_or_else(|| {
            let msg = format!("cant read input url: {}", mask_sensitive_info(url_str));
            error!("{}", msg);
            create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "{}", msg)
        }, Ok)
    }
}


pub async fn get_input_text_content(client: Arc<reqwest::Client>, input: &ConfigInput, working_dir: &str, url_str: &str, persist_filepath: Option<PathBuf>) -> Result<String, M3uFilterError> {
    debug_if_enabled!("getting input text content working_dir: {}, url: {}", working_dir, mask_sensitive_info(url_str));

    if url_str.parse::<url::Url>().is_ok() {
        match download_text_content(client, input, url_str, persist_filepath).await {
            Ok(content) => Ok(content),
            Err(e) => {
                error!("cant download input url: {}  => {}", mask_sensitive_info(url_str), mask_sensitive_info(e.to_string().as_str()));
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

                    match get_local_file_content(&filepath) {
                        Ok(content) => Some(content),
                        Err(err) => {
                            return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Failed : {}", err);
                        }
                    }
                } else {
                    None
                }
            }
            None => None
        };
        result.map_or_else(|| {
            let msg = format!("cant read input url: {}", mask_sensitive_info(url_str));
            error!("{}", msg);
            create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "{}", msg)
        }, Ok)
    }
}

pub fn get_client_request(client: &Arc<reqwest::Client>,
                          headers: Option<&HashMap<String, String>>,
                          url: &Url,
                          custom_headers: Option<&HashMap<String, Vec<u8>>>) -> reqwest::RequestBuilder {
    let request = client.get(url.clone());
    let headers = get_request_headers(headers, custom_headers);
    request.headers(headers)
}

pub fn get_request_headers(defined_headers: Option<&HashMap<String, String>>, custom_headers: Option<&HashMap<String, Vec<u8>>>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(def_headers) = defined_headers {
        for (key, value) in def_headers {
            headers.insert(
                HeaderName::from_bytes(key.as_bytes()).unwrap(),
                HeaderValue::from_bytes(value.as_bytes()).unwrap());
        }
    }
    if let Some(custom) = custom_headers {
        let header_keys: HashSet<String> = headers.keys().map(|k| k.as_str().to_lowercase()).collect();
        for (key, value) in custom {
            let key_lc = key.to_lowercase();
            if "host" == key_lc || header_keys.contains(key_lc.as_str()) {
                // debug_if_enabled!("Ignoring request header '{}={}'", key_lc, String::from_utf8_lossy(value));
            } else {
                headers.insert(
                    HeaderName::from_bytes(key.as_bytes()).unwrap(),
                    HeaderValue::from_bytes(value).unwrap());
            }
        }
    }
    if log_enabled!(Level::Trace) {
        let he: HashMap<String, String> = headers.iter().map(|(k, v)| (k.to_string(), String::from_utf8_lossy(v.as_bytes()).to_string())).collect();
        if !he.is_empty() {
            trace!("Request headers {:?}", he);
        }
    }
    headers
}

fn get_local_file_content(file_path: &PathBuf) -> Result<String, Error> {
    // Check if the file is accessible
    if file_path.exists() && file_path.is_file() {
        if let Ok(content) = fs::read(file_path) {
            if content.len() >= 2 && is_gzip(&content[0..2]) {
                let mut decoder = GzDecoder::new(&content[..]);
                let mut decode_buffer = String::new();
                match decoder.read_to_string(&mut decode_buffer) {
                    Ok(_) => return Ok(decode_buffer),
                    Err(err) => return Err(str_to_io_error(&format!("failed to decode gzip content {err}")))
                };
            }
            return Ok(String::from_utf8_lossy(&content).parse().unwrap());
        }
    }
    let file_str = file_path.to_str().unwrap_or("?");
    Err(Error::new(ErrorKind::InvalidData, format!("Cant find file {file_str}")))
}


async fn get_remote_content_as_file(client: Arc<reqwest::Client>, input: &ConfigInput, url: &Url, file_path: &Path) -> Result<PathBuf, std::io::Error> {
    let start_time = Instant::now();
    let request = get_client_request(&client, Some(&input.headers), url, None);
    match request.send().await {
        Ok(response) => {
            if response.status().is_success() {
                // Open a file in write mode
                let mut file = BufWriter::with_capacity(8192, File::create(file_path)?);
                // Stream the response body in chunks
                let mut stream = response.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(bytes) => {
                            file.write_all(&bytes)?;
                        }
                        Err(err) => {
                            return Err(str_to_io_error(&format!("Failed to read chunk: {err}")));
                        }
                    }
                }

                file.flush()?;
                let elapsed = start_time.elapsed().as_secs();
                debug!("File downloaded successfully to {file_path:?}, took:{}", format_elapsed_time(elapsed));
                Ok(file_path.to_path_buf())
            } else {
                Err(str_to_io_error(&format!("Request failed with status {} {}", response.status(), mask_sensitive_info(url.as_str()))))
            }
        }
        Err(err) => Err(str_to_io_error(&format!("Request failed: {} {err}", mask_sensitive_info(url.as_str())))),
    }
}

async fn get_remote_content(client: Arc<reqwest::Client>, input: &ConfigInput, url: &Url) -> Result<String, Error> {
    let start_time = Instant::now();
    let request = get_client_request(&client, Some(&input.headers), url, None);
    match request.send().await {
        Ok(response) => {
            let is_success = response.status().is_success();
            if is_success {
                let header_value = response.headers().get(CONTENT_ENCODING);
                let mut encoding = header_value.and_then(|encoding_header| encoding_header.to_str().map_or(None, |value| Some(value.to_string())));
                match response.bytes().await {
                    Ok(bytes) => {
                        if bytes.len() >= 2 {
                            if is_gzip(&bytes[0..2]) {
                                encoding = Some(ENCODING_GZIP.to_string());
                            } else if is_deflate(&bytes[0..2]) {
                                encoding = Some(ENCODING_DEFLATE.to_string());
                            }
                        }

                        let mut decode_buffer = String::new();
                        if let Some(encoding_type) = encoding {
                            match encoding_type.as_str() {
                                ENCODING_GZIP => {
                                    let mut decoder = GzDecoder::new(&bytes[..]);
                                    match decoder.read_to_string(&mut decode_buffer) {
                                        Ok(_) => {}
                                        Err(err) => return Err(str_to_io_error(&format!("failed to decode gzip content {err}")))
                                    };
                                }
                                ENCODING_DEFLATE => {
                                    let mut decoder = ZlibDecoder::new(&bytes[..]);
                                    match decoder.read_to_string(&mut decode_buffer) {
                                        Ok(_) => {}
                                        Err(err) => return Err(str_to_io_error(&format!("failed to decode zlib content {err}")))
                                    }
                                }
                                _ => {}
                            };
                        }

                        if decode_buffer.is_empty() {
                            match String::from_utf8(bytes.to_vec()) {
                                Ok(decoded_content) => {
                                    debug_if_enabled!("Request took:{} {}", format_elapsed_time(start_time.elapsed().as_secs()), mask_sensitive_info(url.as_str()));
                                    Ok(decoded_content)
                                }
                                Err(err) => Err(str_to_io_error(&format!("failed to plain text content {err}")))
                            }
                        } else {
                            debug_if_enabled!("Request took:{},  {}", format_elapsed_time(start_time.elapsed().as_secs()), mask_sensitive_info(url.as_str()));
                            Ok(decode_buffer)
                        }
                    }
                    Err(err) => Err(str_to_io_error(&format!("failed to read response {} {err}", mask_sensitive_info(url.as_str()))))
                }
            } else {
                Err(str_to_io_error(&format!("Request failed with status {} {}", response.status(), mask_sensitive_info(url.as_str()))))
            }
        }
        Err(err) => Err(str_to_io_error(&format!("Request failed {} {err}", mask_sensitive_info(url.as_str()))))
    }
}

pub async fn download_text_content_as_file(client: Arc<reqwest::Client>, input: &ConfigInput, url_str: &str, working_dir: &str, persist_filepath: Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Ok(url) = url_str.parse::<url::Url>() {
        if url.scheme() == "file" {
            url.to_file_path().map_or_else(|()| Err(Error::new(ErrorKind::Unsupported, format!("Unknown file {}", mask_sensitive_info(url_str)))), |file_path| if file_path.exists() {
                Ok(file_path)
            } else {
                Err(Error::new(ErrorKind::NotFound, format!("Unknown file {file_path:?}")))
            })
        } else {
            let file_path = persist_filepath.map_or_else(|| match get_input_storage_path(input, working_dir) {
                Ok(download_path) => {
                    Ok(download_path.join(FILE_EPG))
                }
                Err(err) => Err(err)
            }, Ok);
            match file_path {
                Ok(persist_path) => get_remote_content_as_file(client, input, &url, &persist_path).await,
                Err(err) => Err(err)
            }
        }
    } else {
        Err(std::io::Error::new(ErrorKind::Unsupported, format!("Malformed URL {}", mask_sensitive_info(url_str))))
    }
}


pub async fn download_text_content(client: Arc<reqwest::Client>, input: &ConfigInput, url_str: &str, persist_filepath: Option<PathBuf>) -> Result<String, Error> {
    if let Ok(url) = url_str.parse::<url::Url>() {
        let result = if url.scheme() == "file" {
            url.to_file_path().map_or_else(|()| Err(str_to_io_error(&format!("Unknown file {}", mask_sensitive_info(url_str)))), |file_path| get_local_file_content(&file_path))
        } else {
            get_remote_content(client, input, &url).await
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
        Err(str_to_io_error(&format!("Malformed URL {}", mask_sensitive_info(url_str))))
    }
}

async fn download_json_content(client: Arc<reqwest::Client>, input: &ConfigInput, url: &str, persist_filepath: Option<PathBuf>) -> Result<serde_json::Value, Error> {
    debug_if_enabled!("downloading json content from {}", mask_sensitive_info(url));
    match download_text_content(client, input, url, persist_filepath).await {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(value) => Ok(value),
                Err(err) => Err(str_to_io_error(&format!("Failed to parse json {err}")))
            }
        }
        Err(err) => Err(err)
    }
}

pub async fn get_input_json_content(client: Arc<reqwest::Client>, input: &ConfigInput, url: &str, persist_filepath: Option<PathBuf>) -> Result<serde_json::Value, M3uFilterError> {
    match download_json_content(client, input, url, persist_filepath).await {
        Ok(content) => Ok(content),
        Err(e) => create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "cant download input url: {}  => {}", mask_sensitive_info(url), mask_sensitive_info(e.to_string().as_str()))
    }
}
//
// pub fn get_base_url(url: &str) -> Option<String> {
//     if let Some((scheme_end, rest)) = url.split_once("://") {
//         let scheme = scheme_end;
//         if let Some(authority_end) = rest.find('/') {
//             let authority = &rest[..authority_end];
//             return Some(format!("{}://{}", scheme, authority));
//         }
//         return Some(format!("{}://{}", scheme, rest));
//     }
//     None
// }

static USERNAME_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| Regex::new(r"(username=)[^&]*").unwrap());
static PASSWORD_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| Regex::new(r"(password=)[^&]*").unwrap());
static TOKEN_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| Regex::new(r"(token=)[^&]*").unwrap());
static STREAM_URL: LazyLock<regex::Regex> = LazyLock::new(|| Regex::new(r"(.*://).*/(live|video|movie|series|m3u-stream)/\w+/\w+").unwrap());

pub fn mask_sensitive_info(query: &str) -> String {
    // Replace with "***"
    let masked_query = USERNAME_REGEX.replace_all(query, "$1***");
    let masked_query = PASSWORD_REGEX.replace_all(&masked_query, "$1***");
    let masked_query = TOKEN_REGEX.replace_all(&masked_query, "$1***");
    let masked_query =STREAM_URL.replace_all(&masked_query, "$1***/$2/***");
    masked_query.to_string()
}

pub fn extract_extension_from_url(url: &str) -> Option<&str> {
    if let Some(protocol_pos) = url.find("://") {
        if let Some(last_slash_pos) = url[protocol_pos + 3..].rfind('/') {
            let path = &url[protocol_pos + 3 + last_slash_pos + 1..];
            if let Some(last_dot_pos) = path.rfind('.') {
                return Some(&path[last_dot_pos..]);
            }
        }
    } else if let Some(last_dot_pos) = url.rfind('.') {
        if last_dot_pos > url.rfind('/').unwrap_or(0) {
            return Some(&url[last_dot_pos..]);
        }
    }
    None
}


#[cfg(test)]
mod tests {
    use crate::utils::request_utils::STREAM_URL;

    #[test]
    fn test_url_mask() {
        // Replace with "***"
        let masked_query = "https://bubblegum.tv/live/username/password/2344.ts";
        let masked_query = STREAM_URL.replace_all(&masked_query, "$1***/$2/***");
        println!("{masked_query}")
    }
}