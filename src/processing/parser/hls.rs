use crate::model::api_proxy::ProxyUserCredentials;
use crate::model::config::TargetType;
use crate::model::hls::HlsEntry;
use std::collections::HashMap;
use std::str;
use tokio::time::Instant;
use crate::utils::string_utils::replace_after_last_slash;

pub const HLS_PREFIX: &str = "hls";


pub struct RewriteHlsProps<'a> {
    pub base_url: &'a str,
    pub content: &'a str,
    pub hls_url: &'a str,
    pub virtual_id: u32,
    pub token: u32,
    pub target_type: TargetType,
    pub input_id: u16
}

pub fn rewrite_hls(user: &ProxyUserCredentials, props: &RewriteHlsProps ) -> (HlsEntry, String) {
    let username = &user.username;
    let password = &user.password;
    let mut chunk: u32 = 1;
    let mut chunks = HashMap::new();
    let mut result = Vec::new();
    for line in props.content.lines() {
        if line.starts_with('#') {
            result.push(line.to_string());
        } else {
            let url = if line.starts_with("http") {
                line.to_string()
            } else {
                replace_after_last_slash(props.hls_url, line)
            };
            chunks.insert(chunk, url);
            result.push(format!("{}/{HLS_PREFIX}/{}/{username}/{password}/{}/{chunk}", props.base_url, props.token, props.virtual_id));
            chunk += 1;
        }
    }

    let hls = HlsEntry {
        ts: Instant::now(),
        token: props.token,
        target_type: props.target_type,
        input_id: props.input_id,
        virtual_id: props.virtual_id,
        chunk,
        chunks,
    };
    (hls, result.join("\r\n"))
}
