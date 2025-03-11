use crate::utils::debug_if_enabled;
use reqwest::{StatusCode};
use std::collections::{HashSet};
use std::str::FromStr;
use reqwest::header::HeaderMap;
use crate::utils::network::request::sanitize_sensitive_info;

const MEDIA_STREAM_HEADERS: &[&str] = &["accept", "content-type", "content-length", "connection", "accept-ranges", "content-range", "vary", "transfer-encoding", "access-control-allow-origin", "access-control-allow-credentials", "icy-metadata"];

pub fn get_response_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    let response_headers: Vec<(String, String)> = headers.iter()
        .filter(|(key, _)| MEDIA_STREAM_HEADERS.contains(&key.as_str()))
        .map(|(key, value)| (key.to_string(), value.to_str().unwrap().to_string())).collect();
    response_headers
}

pub fn get_stream_response_with_headers(custom: Option<(Vec<(String, String)>, StatusCode)>, stream_url: &str) ->  (axum::http::StatusCode, axum::http::HeaderMap) {
    let mut headers = HeaderMap::new();
    let mut added_headers: HashSet<String> = HashSet::new();
    let mut status = StatusCode::OK;

    if let Some((custom_headers, status_code)) = custom {
        status = status_code;
        for (key, value) in custom_headers {
            if let (Ok(name), Ok(val)) = (axum::http::HeaderName::from_str(&key), axum::http::HeaderValue::from_str(&value)) {
                headers.insert(name.clone(), val);
                added_headers.insert(key);
            }
        }
    }

    let default_headers = vec![
        ("content-type", "application/octet-stream"),
        ("connection", "keep-alive"),
    ];

    for (key, value) in default_headers {
        if !added_headers.contains(key) {
            if let (Ok(name), Ok(val)) = (axum::http::HeaderName::from_str(key), axum::http::HeaderValue::from_str(value)) {
                headers.insert(name, val);
            }
        }
    }

    // Füge das aktuelle Datum hinzu
    if let Ok(date_header) = axum::http::HeaderValue::from_str(&chrono::Utc::now().to_rfc2822()) {
        headers.insert(axum::http::HeaderName::from_static("date"), date_header);
    }

    debug_if_enabled!(
        "Responding stream request {} with status {}, headers {:?}",
        sanitize_sensitive_info(stream_url),
        status,
        headers
    );

    (status, headers)
}