use crate::model::api_proxy::ProxyUserCredentials;
use std::str;
use crate::model::config::TargetType;

// /hlsr/{token}/{username}/{password}/{channel}/{hash}/{chunk}
#[derive(Debug)]
pub struct HlsrPath {
    token: String,
    // username: String,
    // password: String,
    // channel: String,
    hash: String,
    chunk: String,
}
fn parse_hlsr_path(input: &str) -> Option<HlsrPath> {
    let parts: Vec<&str> = input.split('/').collect();

    if parts.len() != 8 || !parts[0].is_empty() || parts[1] != "hlsr" {
        return None;
    }

    Some(HlsrPath {
        token: parts[2].to_string(),
        // username: parts[3].to_string(),
        // password: parts[4].to_string(),
        // channel: parts[5].to_string(),
        hash: parts[6].to_string(),
        chunk: parts[7].to_string(),
    })
}

pub const M3U_HLSR_PREFIX: &str = "mhlsr";

pub fn rewrite_hls_url(stream_id: u32, username: &str, password: &str, hlsr: &HlsrPath, target_type: &TargetType) -> String {
    let prefix = if *target_type == TargetType::Xtream { "hlsr" } else { M3U_HLSR_PREFIX };
    format!("/{prefix}/{}/{username}/{password}/{stream_id}/{}/{}", hlsr.token, hlsr.hash, hlsr.chunk)
}

pub fn rewrite_hls(content: &str, virtual_id: u32, user: &ProxyUserCredentials, target_type: &TargetType) -> String {
    content.lines().map(|line| {
        if line.starts_with('#') {
            line.to_string()
        } else {
            match parse_hlsr_path(line) {
                None => line.to_string(),
                Some(hlsr) => rewrite_hls_url(virtual_id, &user.username, &user.password, &hlsr, target_type)
            }
        }
    }).collect::<Vec<_>>()
        .join("\r\n")
}
