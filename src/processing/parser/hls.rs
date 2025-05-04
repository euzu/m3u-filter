use crate::model::ProxyUserCredentials;
use crate::utils::{CONSTANTS, HLS_PREFIX};
use crate::utils::{deobfuscate_text, obfuscate_text, u32_to_base64, base64_to_u32};
use std::str;

fn create_hls_session_token_and_url(secret: &[u8], session_token: u32, stream_url: &str) -> Option<String> {
    let token = u32_to_base64(session_token);
    if let Ok(cookie_value) = obfuscate_text(secret, &format!("{token}{stream_url}")) {
        return Some(cookie_value);
    }
    None
}

const TOKEN_LEN: usize = 6;
pub fn get_hls_session_token_and_url_from_token(secret: &[u8], token: &str) -> Option<(Option<u32>, String)> {
    if let Ok(decrypted) = deobfuscate_text(secret, token) {
        let session_token: String = decrypted.chars().take(TOKEN_LEN).collect();
        let stream_url: String = decrypted.chars().skip(TOKEN_LEN).collect();
        return Some((base64_to_u32(&session_token), stream_url));
    }
    None
}


pub struct RewriteHlsProps<'a> {
    pub secret: &'a [u8; 16],
    pub base_url: &'a str,
    pub content: &'a str,
    pub hls_url: String,
    pub virtual_id: u32,
    pub input_id: u16,
    pub user_token: Option<u32>,
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
        if let Some(user_token) = &props.user_token {
            if let Some(token) = create_hls_session_token_and_url(props.secret, *user_token, target_url) {
                return CONSTANTS.re_hls_uri.replace(line, format!(r#"URI="{token}""#)).to_string();
            }
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
        if let Some(user_token) = &props.user_token {
            if let Some(token) = create_hls_session_token_and_url(props.secret, *user_token, &target_url) {
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
    }
    result.push("\r\n".to_string());
    result.join("\r\n")
}

#[cfg(test)]
mod test {
    use rand::RngCore;
    use crate::utils::u32_to_base64;

    #[test]
    fn test_token_size() {
        for _i in 0..10_000 {
            let session_token = rand::rng().next_u32();
            assert_eq!(u32_to_base64(session_token).len(), 6);
        }
    }

}