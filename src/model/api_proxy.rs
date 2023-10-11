use std::collections::{HashSet};
use crate::exit;
use log::{error};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct UserCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TargetUser {
    pub target: String,
    pub credentials: Vec<UserCredentials>,
}

impl TargetUser {

    pub fn get_target_name(&self, username: &str, password: &str) -> Option<String> {
        if self.credentials.iter().any(|c| c.username.eq_ignore_ascii_case(username) && c.password.eq(password)) {
            return Some(self.target.clone());
        }
        None
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ServerInfo {
    pub protocol: String,
    pub ip: String,
    pub http_port: u16,
    pub https_port: u16,
    pub rtmp_port: u16,
    pub timezone: String,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ApiProxyConfig {
    pub server: ServerInfo,
    pub user: Vec<TargetUser>,
}

impl ApiProxyConfig {

    pub fn prepare(&self) {
        let mut usernames = HashSet::new();
        for target_user in &self.user {
            for user in &target_user.credentials {
                if usernames.contains(&user.username) {
                    exit!("Non unique username found {}", &user.username);
                } else {
                    usernames.insert(user.username.to_string());
                }
            }
        }
    }

    pub fn get_target_name(&self, username: &str, password: &str) -> Option<String> {
        for target_user in &self.user {
            if let Some(target_name) = target_user.get_target_name(username, password) {
                return Some(target_name);
            };
        }
        None
    }
}