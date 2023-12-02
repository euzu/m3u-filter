use std::collections::HashSet;
use std::str::FromStr;
use enum_iterator::Sequence;
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
        ProxyType::Reverse
    }
}

impl ToString for ProxyType {
    fn to_string(&self) -> String {
        match self {
            ProxyType::Reverse => "reverse".to_string(),
            ProxyType::Redirect => "redirect".to_string()
        }
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
    pub username: String,
    pub password: String,
    pub token: Option<String>,
    #[serde(default = "ProxyType::default")]
    pub proxy: ProxyType
}

impl UserCredentials {
    pub fn matches_token(&self, token: &str) -> bool {
        if let Some(tkn) = &self.token {
            return tkn.eq(token);
        }
        false
    }

    pub fn matches(&self, username: &str, password: &str) -> bool {
        self.username.eq(username) && self.password.eq(password)
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
pub(crate) struct ServerInfo {
    pub protocol: String,
    pub ip: String,
    pub http_port: String,
    #[serde(default = "default_as_443")]
    pub https_port: String,
    #[serde(default = "default_as_1935")]
    pub rtmp_port: String,
    pub timezone: String,
    pub message: String,
}

impl ServerInfo {
    pub fn is_valid(&mut self) -> bool {
        self.protocol = self.protocol.trim().to_string();
        if self.protocol.is_empty() {
            return false;
        }
        self.ip = self.ip.trim().to_string();
        if self.ip.is_empty() {
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
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ApiProxyConfig {
    pub server: ServerInfo,
    pub user: Vec<TargetUser>,
    #[serde(skip_serializing, skip_deserializing)]
    pub _file_path: String,
}

impl ApiProxyConfig {
    pub fn prepare(&mut self) -> Result<(), M3uFilterError> {
        let mut usernames = HashSet::new();
        let mut tokens = HashSet::new();
        let mut errors = Vec::new();
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