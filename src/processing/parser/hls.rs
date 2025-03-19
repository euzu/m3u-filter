use crate::model::api_proxy::ProxyUserCredentials;
use crate::model::config::TargetType;
use crate::model::hls::HlsEntry;
use std::collections::HashMap;
use std::str;
use tokio::time::Instant;
use crate::utils::string_utils::replace_after_last_slash;

pub const HLS_PREFIX: &str = "hls";

pub fn rewrite_hls(base_url: &str, content: &str, hls_url: &str, virtual_id: u32,
                   token: u32,
                   user: &ProxyUserCredentials,
                   target_type: &TargetType, input_id: u16) -> (HlsEntry, String) {
    let username = &user.username;
    let password = &user.password;
    let mut chunk: u32 = 1;
    let mut chunks = HashMap::new();
    let mut result = Vec::new();
    for line in content.lines() {
        if line.starts_with('#') {
            result.push(line.to_string());
        } else {
            let url = if line.starts_with("http") {
                line.to_string()
            } else {
                replace_after_last_slash(hls_url, line)
            };
            chunks.insert(chunk, url);
            result.push(format!("{base_url}/{HLS_PREFIX}/{token}/{username}/{password}/{virtual_id}/{chunk}"));
            chunk += 1;
        }
    }

    let hls = HlsEntry {
        ts: Instant::now(),
        token,
        target_type: target_type.clone(),
        input_id,
        virtual_id,
        chunk,
        chunks,
    };
    (hls, result.join("\r\n"))
}
