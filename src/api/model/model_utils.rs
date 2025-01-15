use crate::debug_if_enabled;
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::{HttpResponseBuilder};
use reqwest::{Response, StatusCode};
use std::collections::{HashSet};
use std::str::FromStr;
use crate::utils::request_utils::mask_sensitive_info;

const MEDIA_STREAM_HEADERS: &[&str] = &["content-type", "content-length", "connection", "accept-ranges", "content-range", "vary"];

pub fn get_response_headers(response: &mut Response) -> Vec<(String, String)> {
    let response_headers: Vec<(String, String)> = response.headers_mut().iter()
        .filter(|(key, _)| MEDIA_STREAM_HEADERS.contains(&key.as_str()))
        .map(|(key, value)| (key.to_string(), value.to_str().unwrap().to_string())).collect();
    response_headers
}

pub fn get_stream_response_with_headers(custom: Option<(Vec<(String, String)>, StatusCode)>, stream_url: &str) -> HttpResponseBuilder {
    let mut headers = Vec::<(HeaderName, HeaderValue)>::with_capacity(12);
    let mut added_headers: HashSet<String> = HashSet::new();
    let mut status = 200_u16;
    if let Some((custom_headers, status_code)) = custom {
        status = status_code.as_u16();
        for header in custom_headers {
            headers.push((HeaderName::from_str(&header.0).unwrap(), HeaderValue::from_str(header.1.as_str()).unwrap()));
            added_headers.insert(header.0.to_string());
        }
    }

    let default_headers = vec![
        (actix_web::http::header::CONTENT_TYPE, HeaderValue::from_str("application/octet-stream").unwrap()),
        (actix_web::http::header::CONTENT_LENGTH, HeaderValue::from(0)),
        (actix_web::http::header::CONNECTION, HeaderValue::from_str("keep-alive").unwrap()),
        //(actix_web::http::header::CACHE_CONTROL, HeaderValue::from_str("no-cache").unwrap()),
        (actix_web::http::header::VARY, HeaderValue::from_str("accept-encoding").unwrap())
    ];

    for header in default_headers {
        if !added_headers.contains(header.0.as_str()) {
            headers.push(header);
        }
    }

    headers.push((actix_web::http::header::DATE, HeaderValue::from_str(&chrono::Utc::now().to_rfc2822()).unwrap()));

    let mut response_builder = actix_web::HttpResponse::build(actix_web::http::StatusCode::from_u16(status).unwrap());
    debug_if_enabled!("Opening stream {} with status {status}, headers {headers:?}", mask_sensitive_info(stream_url));
    for header in headers {
        response_builder.insert_header(header);
    }
    response_builder
}