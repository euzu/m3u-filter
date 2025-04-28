use crate::model::api_proxy::ProxyUserCredentials;
use std::str;
use crate::api::api_utils::{create_token_for_provider};
use crate::utils::constants::{CONSTANTS, HLS_PREFIX};

pub struct RewriteHlsProps<'a> {
    pub secret: &'a [u8;16],
    pub base_url: &'a str,
    pub content: &'a str,
    pub hls_url: String,
    pub virtual_id: u32,
    pub input_id: u16,
    pub provider_name: String,
    pub user_token: String,
}

fn rewrite_hls_url(input: &str, replacement: &str) -> String {
    if replacement.starts_with('/') {
        let parts = input.splitn(4, '/').collect::<Vec<&str>>();
        if parts.len() < 4 {
            return replacement.to_string(); // less than 3 Slashes → replace all
        }
        format!("{}/{}/{}{}", parts[0], parts[1], parts[2], replacement)
    } else {
        match input.rsplitn(2, '/').collect::<Vec<&str>>().as_slice() {
            [_after, before] => format!("{before}/{replacement}"),
            [_only] => replacement.to_string(),
            _ => input.to_string(),
        }
    }
}

fn rewrite_uri_attrib(line: &str, props: &RewriteHlsProps) -> String {
    if let Some(caps) = CONSTANTS.re_hls_uri.captures(line) {
        let uri = &caps[1];
        let target_url = &rewrite_hls_url(&props.hls_url, uri);
        if let Some(token) = create_token_for_provider(props.secret, &props.user_token, props.virtual_id, &props.provider_name, target_url) {
            return CONSTANTS.re_hls_uri.replace(line, format!(r#"URI="{token}""#)).to_string();
        }
    }
    line.to_string()
}

pub fn rewrite_hls(user: &ProxyUserCredentials, props: &RewriteHlsProps) -> String {
    let username = &user.username;
    let password = &user.password;
    let mut result = Vec::new();
    for line in props.content.lines() {
        // skip comments
        if line.starts_with('#') {
            let rewritten = rewrite_uri_attrib(line, props);
            result.push(rewritten);
            continue;
        }

        // target url
        let target_url = if line.starts_with("http") {
            line.to_string()
        } else {
            rewrite_hls_url(&props.hls_url, line)
        };
        if let Some(token) = create_token_for_provider(props.secret, &props.user_token, props.virtual_id, &props.provider_name, &target_url) {
            let url = format!(
                "{}/{HLS_PREFIX}/{}/{}/{}/{}/{}",
                props.base_url,
                username,
                password,
                props.input_id,
                props.virtual_id,
                token
            );
            result.push(url);
        }
    }
    result.push("\r\n".to_string());
    result.join("\r\n")
}
