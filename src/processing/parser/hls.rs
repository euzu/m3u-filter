use crate::model::api_proxy::ProxyUserCredentials;
use crate::utils::crypto_utils::encrypt_text;
use std::str;

pub const HLS_PREFIX: &str = "hls";


pub struct RewriteHlsProps<'a> {
    pub secret: &'a [u8;16],
    pub base_url: &'a str,
    pub content: &'a str,
    pub hls_url: String,
    pub virtual_id: u32,
    pub input_id: u16,
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

// TODO # line can have URI parts whcih shuld rewritten too

pub fn rewrite_hls(user: &ProxyUserCredentials, props: &RewriteHlsProps) -> String {
    let username = &user.username;
    let password = &user.password;
    let mut result = Vec::new();
    for line in props.content.lines() {
        if line.starts_with('#') {
            result.push(line.to_string());
        } else if let Ok(token) = if line.starts_with("http") {
            encrypt_text(props.secret, line)
        } else {
            encrypt_text(props.secret, &rewrite_hls_url(&props.hls_url, line))
        } {
            result.push(format!("{}/{HLS_PREFIX}/{username}/{password}/{}/{}/{token}", props.base_url, props.input_id, props.virtual_id));
        }
    }
    result.join("\r\n")
}
