use crate::model::api_proxy::{ApiProxyServerInfo, ProxyUserCredentials, ProxyUserStatus};
use chrono::{Duration, Local};
use serde::Serialize;

#[derive(Serialize)]
pub struct XtreamUserInfoResponse {
    pub password: String,
    pub username: String,
    pub active_cons: String,
    pub allowed_output_formats: Vec<String>, //["ts"],
    pub auth: u16, // 0 | 1
    pub created_at: i64, //1623429679,
    pub exp_date: i64, //1628755200,
    pub is_trial: String, // 0 | 1
    pub max_connections: String,
    pub message: String,
    pub status: String, // "Active"
}

#[derive(Serialize)]
pub struct XtreamServerInfoResponse {
    pub url: String,
    pub port: String,
    pub https_port: String,
    pub server_protocol: String, // http, https
    pub rtmp_port: String,
    pub timezone: String,
    pub timestamp_now: i64,
    pub time_now: String, //"2021-06-28 17:07:37"
}

#[derive(Serialize)]
pub struct XtreamAuthorizationResponse {
    pub user_info: XtreamUserInfoResponse,
    pub server_info: XtreamServerInfoResponse,
}

// #[derive(Serialize)]
// pub struct XtreamServerInfoDto {
//     pub url: String,
//     pub port: String,
//     pub path: Option<String>,
//     pub protocol: String, // http, https
//     pub timezone: String,
//     pub timestamp_now: i64,
//     pub time_now: String, //"2021-06-28 17:07:37"
// }

impl XtreamAuthorizationResponse {
    pub fn new(server_info: &ApiProxyServerInfo, user: &ProxyUserCredentials, active_connections: u32) -> Self {
        let now = Local::now();
        let created_at = user.created_at.as_ref().map_or_else(|| (now - Duration::days(365)).timestamp(), |d| *d);
        let exp_date = user.exp_date.as_ref().map_or_else(|| (now + Duration::days(365)).timestamp(), |d| *d);

        let is_expired = (exp_date - now.timestamp()) < 0;
        let is_trial = user.status.as_ref().map_or("0", |s| if *s == ProxyUserStatus::Trial { "1" } else { "0" }).to_string();
        let max_connections = user.max_connections.as_ref().map_or("1".to_string(), |s| format!("{s}"));
        let current_status = user.status.as_ref().unwrap_or(&ProxyUserStatus::Active);
        let user_status =  match current_status {
            ProxyUserStatus::Active | ProxyUserStatus::Trial => if is_expired { &ProxyUserStatus::Expired } else { current_status },
            _ => current_status
        };

        Self {
            user_info: XtreamUserInfoResponse {
                active_cons: format!("{active_connections}"),
                allowed_output_formats: Vec::from(["ts".to_string()]),
                auth: 1,
                created_at,
                exp_date,
                is_trial,
                max_connections,
                message: server_info.message.to_string(),
                password: user.password.to_string(),
                username: user.username.to_string(),
                status: user_status.to_string(),
            },
            server_info: XtreamServerInfoResponse {
                url: server_info.host.clone(),
                port: if server_info.protocol == "http" { server_info.port.clone() } else { String::from("80") },
                https_port: if server_info.protocol == "https" { server_info.port.clone() } else { String::from("443") },
                server_protocol: server_info.protocol.clone(),
                rtmp_port: String::new(),
                timezone: server_info.timezone.to_string(),
                timestamp_now: now.timestamp(),
                time_now: now.format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        }
    }
}
