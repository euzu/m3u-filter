use std::collections::HashSet;
use std::env;
use std::fmt::Display;
use std::str::FromStr;

use enum_iterator::Sequence;
use log::info;
use regex::Regex;

use crate::create_m3u_filter_error_result;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence, PartialEq)]
pub(crate) enum ProxyType {
    #[serde(rename = "reverse")]
    Reverse,
    #[serde(rename = "redirect")]
    Redirect,
}

impl ProxyType {
    fn default() -> ProxyType {
        ProxyType::Redirect
    }
}

impl Display for ProxyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            ProxyType::Reverse => "reverse".to_string(),
            ProxyType::Redirect => "redirect".to_string()
        };
        write!(f, "{}", str)
    }
}

impl FromStr for ProxyType {
    type Err = M3uFilterError;

    fn from_str(s: &str) -> Result<Self, M3uFilterError> {
        if s.eq("reverse") {
            Ok(ProxyType::Reverse)
        } else if s.eq("redirect") {
            Ok(ProxyType::Redirect)
        } else {
            create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unknown ProxyType: {}", s)
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct UserCredentials {
    username: String,
    password: String,
    token: Option<String>,
    #[serde(default = "ProxyType::default")]
    pub proxy: ProxyType,
    pub server: Option<String>,
}

impl UserCredentials {
    pub fn matches_token(&self, token: &str) -> bool {
        if let Some(tkn) = self.get_token() {
            return tkn.eq(token);
        }
        false
    }

    pub fn matches(&self, username: &str, password: &str) -> bool {
        self.get_username().eq(username) && self.get_password().eq(password)
    }

    pub fn trim(&mut self) {
        self.username = self.username.trim().to_string();
        self.password = self.password.trim().to_string();
        match &self.token {
            None => {}
            Some(tkn) => {
                self.token = Some(tkn.trim().to_string());
            }
        }
    }

    fn resolve_env_var(content: &str) -> String {
        let pattern = Regex::new(r#"\$\{env:(?P<var>[a-zA-Z_][a-zA-Z0-9_]*)}"#).unwrap();
        if let Some(caps) = pattern.captures(content) {
            if let Some(var) = caps.name("var") {
                let var_name = var.as_str();
                return match env::var(var_name) {
                    Ok(val) =>  val, // If environment variable found, replace with its value
                    Err(_) => content.to_string()                }
            }
        }
        content.to_string()
    }

    pub fn get_username(&self) -> String {
        UserCredentials::resolve_env_var(&self.username)
    }

    pub fn get_password(&self) -> String {
        UserCredentials::resolve_env_var(&self.password)
    }

    pub fn get_token(&self) -> Option<String> {
        if let Some(tkn) = &self.token {
            Some(UserCredentials::resolve_env_var(tkn))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TargetUser {
    pub target: String,
    pub credentials: Vec<UserCredentials>,
}

impl TargetUser {
    pub fn get_target_name(&self, username: &str, password: &str) -> Option<(&UserCredentials, &str)> {
        self.credentials.iter().find(|c| c.matches(username, password))
            .map(|credentials| (credentials, self.target.as_str()))
    }
    pub fn get_target_name_by_token(&self, token: &str) -> Option<(&UserCredentials, &str)> {
        self.credentials.iter().find(|c| c.matches_token(token))
            .map(|credentials| (credentials, self.target.as_str()))
    }
}

fn default_as_443() -> String { "443".to_string() }

fn default_as_1935() -> String { "1935".to_string() }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ApiProxyServerInfo {
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub http_port: String,
    #[serde(default = "default_as_443")]
    pub https_port: String,
    #[serde(default = "default_as_1935")]
    pub rtmp_port: String,
    pub timezone: String,
    pub message: String,
}

impl ApiProxyServerInfo {
    pub fn is_valid(&mut self) -> bool {
        self.protocol = self.protocol.trim().to_string();
        if self.protocol.is_empty() {
            return false;
        }
        self.host = self.host.trim().to_string();
        if self.host.is_empty() {
            return false;
        }
        self.http_port = self.http_port.trim().to_string();
        if self.http_port.is_empty() {
            self.http_port = "80".to_string();
        } else if self.http_port.parse::<u16>().is_err() {
            return false;
        }
        self.https_port = self.https_port.trim().to_string();
        if self.https_port.is_empty() {
            self.https_port = "443".to_string();
        } else if self.https_port.parse::<u16>().is_err() {
            return false;
        }
        self.rtmp_port = self.rtmp_port.trim().to_string();
        if self.rtmp_port.is_empty() {
            self.rtmp_port = "1953".to_string();
        } else if self.rtmp_port.parse::<u16>().is_err() {
            return false;
        }
        self.timezone = self.timezone.trim().to_string();
        if self.timezone.is_empty() {
            self.timezone = "UTC".to_string();
        }
        if self.message.is_empty() {
            self.message = "Welcome to m3u-filter".to_string();
        }

        true
    }

    pub(crate) fn get_base_url(&self) -> String {
        if self.protocol.eq("http") {
            format!("http://{}:{}", self.host, self.http_port)
        } else {
            format!("https://{}:{}", self.host, self.https_port)
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ApiProxyConfig {
    pub server: Vec<ApiProxyServerInfo>,
    pub user: Vec<TargetUser>,
    #[serde(skip_serializing, skip_deserializing)]
    pub _file_path: String,
}

impl ApiProxyConfig {
    pub fn prepare(&mut self) -> Result<(), M3uFilterError> {
        let mut usernames = HashSet::new();
        let mut tokens = HashSet::new();
        let mut errors = Vec::new();
        if self.server.is_empty() {
            errors.push("No serverinfo defined".to_string());
        } else {
            let mut name_set = HashSet::new();
            for server in &self.server {
                if server.name.trim().is_empty() {
                    errors.push("Server info name is empty ".to_owned());
                } else if name_set.contains(server.name.as_str()) {
                    errors.push(format!("Non unique server info name found {}", &server.name));
                } else {
                    name_set.insert(server.name.clone());
                }
            }
        }
        for target_user in &mut self.user {
            for user in &mut target_user.credentials {
                if usernames.contains(&user.username) {
                    errors.push(format!("Non unique username found {}", &user.username));
                } else {
                    usernames.insert(user.username.to_string());
                }
                if let Some(token) = &user.token {
                    if token.is_empty() {
                        user.token = None
                    } else if tokens.contains(token) {
                        errors.push(format!("Non unique token found {}", &user.username));
                    } else {
                        tokens.insert(token.to_string());
                    }
                }

                if let Some(server_info_name) = &user.server {
                    if !&self.server.iter().any(|server_info| server_info.name.eq(server_info_name)) {
                        errors.push(format!("No server info with name {} found for user {}", server_info_name, &user.username));
                    }
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(M3uFilterError::new(M3uFilterErrorKind::Info, errors.join("\n")))
        }
    }

    pub fn get_target_name(&self, username: &str, password: &str) -> Option<(UserCredentials, String)> {
        for target_user in &self.user {
            if let Some((credentials, target_name)) = target_user.get_target_name(username, password) {
                return Some((credentials.clone(), target_name.to_string()));
            };
        }
        None
    }

    pub fn get_target_name_by_token(&self, token: &str) -> Option<(UserCredentials, String)> {
        for target_user in &self.user {
            if let Some((credentials, target_name)) = target_user.get_target_name_by_token(token) {
                return Some((credentials.clone(), target_name.to_string()));
            };
        }
        None
    }
}