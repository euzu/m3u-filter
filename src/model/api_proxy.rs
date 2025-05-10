use crate::api::model::app_state::AppState;
use crate::m3u_filter_error::{create_m3u_filter_error_result, info_err, M3uFilterError, M3uFilterErrorKind};
use crate::model::{Config, ClusterFlags};
use crate::repository::user_repository::{backup_api_user_db_file, get_api_user_db_path, load_api_user, merge_api_user};
use crate::utils::default_as_true;
use crate::utils::config_reader;
use chrono::Local;
use enum_iterator::Sequence;
use log::debug;
use std::cmp::PartialEq;
use std::collections::HashSet;
use std::fmt::Display;
use std::fs;
use std::str::FromStr;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use crate::model::PlaylistItemType;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum UserConnectionPermission {
    Exhausted,
    Allowed,
    GracePeriod,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyType {
    Reverse(Option<ClusterFlags>),
    Redirect,
}

impl Default for ProxyType {
    fn default() -> Self {
        Self::Redirect
    }
}

impl ProxyType {
    const REVERSE: &'static str = "reverse";
    const REDIRECT: &'static str = "redirect";

    pub fn is_redirect(&self, item_type: PlaylistItemType) -> bool {
        match self {
            ProxyType::Reverse(Some(flags)) => {
                if flags.is_empty() {
                    return false;
                }
                if flags.has_cluster(item_type) {
                    return false;
                }
                true
            },
            ProxyType::Reverse(None) => false,
            ProxyType::Redirect => true
        }
    }

    pub fn is_reverse(&self, item_type: PlaylistItemType) -> bool {
        !self.is_redirect(item_type)
    }
}

impl Display for ProxyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reverse(force_redirect) => {
                if let Some(force) = force_redirect {
                    write!(f, "{}{force:?}", Self::REVERSE)
                } else {
                    write!(f, "{}", Self::REVERSE)
                }
            }
            Self::Redirect => write!(f, "{}", Self::REDIRECT),
        }
    }
}

impl FromStr for ProxyType {
    type Err = M3uFilterError;

    fn from_str(s: &str) -> Result<Self, M3uFilterError> {
        if s == Self::REDIRECT {
            return Ok(Self::Redirect);
        }
        if s == Self::REVERSE {
            return Ok(Self::Reverse(None));
        }

        if s.starts_with(Self::REVERSE) {
            let suffix = s.strip_prefix(Self::REVERSE).unwrap();
            if let Ok(force_redirect) = ClusterFlags::try_from(suffix) {
                if force_redirect.has_full_flags() {
                    return Ok(ProxyType::Reverse(None));
                }
                return Ok(Self::Reverse(Some(force_redirect)));
            }
        }

        create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unknown ProxyType: {}", s)
    }
}

impl<'de> Deserialize<'de> for ProxyType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw: String = Deserialize::deserialize(deserializer)?;
        if raw == ProxyType::REDIRECT {
            return Ok(ProxyType::Redirect);
        } else if raw.starts_with(ProxyType::REVERSE) {
            return ProxyType::from_str(raw.as_str()).map_err(serde::de::Error::custom);
        }
        Err(serde::de::Error::custom("Unknown proxy type"))
    }
}

impl Serialize for ProxyType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            ProxyType::Redirect => serializer.serialize_str(ProxyType::REDIRECT),
            ProxyType::Reverse(None) => serializer.serialize_str(ProxyType::REVERSE),
            ProxyType::Reverse(Some(ref force_redirect)) => {
                serializer.serialize_str(&format!("{}{}", ProxyType::REVERSE, force_redirect))
            },
        }
    }
}


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence, PartialEq, Eq)]
pub enum ProxyUserStatus {
    Active, // The account is in good standing and can stream content
    Expired, // The account can no longer access content unless it is renewed.
    Banned, // The account is temporarily or permanently disabled. Typically used for users who violate terms of service or abuse the system.
    Trial, // The account is marked as a trial account.
    Disabled, // The account is inactive or deliberately disabled by the administrator.
    Pending,
}


impl Default for ProxyUserStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl ProxyUserStatus {
    const ACTIVE: &'static str = "Active";
    const EXPIRED: &'static str = "Expired";
    const BANNED: &'static str = "Banned";
    const TRIAL: &'static str = "Trial";
    const DISABLED: &'static str = "Disabled";
    const PENDING: &'static str = "Pending";
}

impl Display for ProxyUserStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Active => Self::ACTIVE,
            Self::Expired => Self::EXPIRED,
            Self::Banned => Self::BANNED,
            Self::Trial => Self::TRIAL,
            Self::Disabled => Self::DISABLED,
            Self::Pending => Self::PENDING,
        })
    }
}

impl FromStr for ProxyUserStatus {
    type Err = M3uFilterError;

    fn from_str(s: &str) -> Result<Self, M3uFilterError> {
        match s {
            Self::EXPIRED => Ok(Self::Expired),
            Self::BANNED => Ok(Self::Banned),
            Self::TRIAL => Ok(Self::Trial),
            Self::DISABLED => Ok(Self::Disabled),
            Self::PENDING => Ok(Self::Pending),
            _ => create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unknown ProxyType: {}", s)
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProxyUserCredentials {
    pub username: String,
    pub password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default = "ProxyType::default")]
    pub proxy: ProxyType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epg_timeshift: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp_date: Option<i64>,
    #[serde(default)]
    pub max_connections: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ProxyUserStatus>,
    #[serde(default = "default_as_true")]
    pub ui_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl ProxyUserCredentials {
    pub fn prepare(&mut self) {
        self.trim();
    }

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

    pub fn validate(&self) -> Result<(), M3uFilterError> {
        if self.username.is_empty() {
            return Err(M3uFilterError::new(M3uFilterErrorKind::Info, "Username required".to_string()));
        }
        if self.password.is_empty() {
            return Err(M3uFilterError::new(M3uFilterErrorKind::Info, "Password required".to_string()));
        }
        Ok(())
    }

    pub fn has_permissions(&self, app_state: &AppState) -> bool {
        if app_state.config.user_access_control {
            if let Some(exp_date) = self.exp_date.as_ref() {
                let now = Local::now();
                if (exp_date - now.timestamp()) < 0 {
                    debug!("User access denied, expired: {}", self.username);
                    return false;
                }
            }

            if let Some(status) = &self.status {
                if !matches!(status, ProxyUserStatus::Active | ProxyUserStatus::Trial) {
                    debug!("User access denied, status invalid: {status} for user: {}", self.username);
                    return false;
                }
            } // NO STATUS SET, ok admins fault, we take this as a valid status
        }
        true
    }

    #[inline]
    pub fn permission_denied(&self, app_state: &AppState) -> bool {
        !self.has_permissions(app_state)
    }

    pub async fn connection_permission(&self, app_state: &AppState) -> UserConnectionPermission {
        if self.max_connections > 0 && app_state.config.user_access_control {
            // we allow requests with max connection reached, but we should block streaming after grace period
            return app_state.get_connection_permission(&self.username, self.max_connections).await;
        }
        UserConnectionPermission::Allowed
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TargetUser {
    pub target: String,
    pub credentials: Vec<ProxyUserCredentials>,
}

impl TargetUser {
    pub fn get_target_name(
        &self,
        username: &str,
        password: &str,
    ) -> Option<(&ProxyUserCredentials, &str)> {
        self.credentials
            .iter()
            .find(|c| c.matches(username, password))
            .map(|credentials| (credentials, self.target.as_str()))
    }
    pub fn get_target_name_by_token(&self, token: &str) -> Option<(&ProxyUserCredentials, &str)> {
        self.credentials
            .iter()
            .find(|c| c.matches_token(token))
            .map(|credentials| (credentials, self.target.as_str()))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiProxyServerInfo {
    pub name: String,
    pub protocol: String,
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    pub timezone: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl ApiProxyServerInfo {

   pub fn prepare(&mut self ) -> Result<(), M3uFilterError> {
       self.name = self.name.trim().to_string();
       if self.name.is_empty() {
           return Err(info_err!("Server info name is empty ".to_string()));
       }
       self.protocol = self.protocol.trim().to_string();
       if self.protocol.is_empty() {
           return Err(info_err!("protocol cant be empty for api server config".to_string()));
       }
       self.host = self.host.trim().to_string();
       if self.host.is_empty() {
           return Err(info_err!("host cant be empty for api server config".to_string()));
       }
       if let Some(port)= self.port.as_ref() {
           let port = port.trim().to_string();
           if port.is_empty() {
               self.port = None;
           } else if port.parse::<u16>().is_err() {
               return Err(info_err!("invalid port for api server config".to_string()));
           } else {
               self.port = Some(port);
           }
       }

       self.timezone = self.timezone.trim().to_string();
       if self.timezone.is_empty() {
           self.timezone = "UTC".to_string();
       }
       if self.message.is_empty() {
           self.message = "Welcome to tuliprox".to_string();
       }
       if let Some(path) = &self.path {
           if path.trim().is_empty() {
               self.path = None;
           }
       }

       if let Some(path) = &self.path {
           let trimmed_path = path.trim();
           if trimmed_path.is_empty() {
               self.path = None;
           } else {
               self.path = Some(trimmed_path.to_string());
           }
       }

       Ok(())
   }
    pub fn validate(&mut self) -> bool {
        self.prepare().is_ok()
    }

    pub fn get_base_url(&self) -> String {
        let base_url = if let Some(port) = self.port.as_ref() {
            format!("{}://{}:{port}", self.protocol, self.host)
        } else {
            format!("{}://{}", self.protocol, self.host)
        };

        match &self.path {
            None => base_url,
            Some(path) => format!("{base_url}/{}", path.trim_matches('/'))
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiProxyConfig {
    pub server: Vec<ApiProxyServerInfo>,
    pub user: Vec<TargetUser>,
    #[serde(default)]
    pub use_user_db: bool,
}

impl ApiProxyConfig {
    // we have the option to store user in the config file or in the user_db
    // When we switch from one to other we need to migrate the existing data.
    fn migrate_api_user(&mut self, cfg: &Config, errors: &mut Vec<String>) {
        if self.use_user_db {
            // we have user defined in config file.
            // we migrate them to the db and delete them from the config file
            if !&self.user.is_empty() {
                if let Err(err) = merge_api_user(cfg, &self.user) {
                    errors.push(err.to_string());
                } else {
                    let api_proxy_file = cfg.t_api_proxy_file_path.as_str();
                    let backup_dir = cfg.backup_dir.as_ref().unwrap().as_str();
                    self.user = vec![];
                    if let Err(err) = config_reader::save_api_proxy(api_proxy_file, backup_dir, self) {
                        errors.push(format!("Error saving api proxy file: {err}"));
                    }
                }
            }
            match load_api_user(cfg) {
                Ok(users) => {
                    self.user = users;
                }
                Err(err) => {
                    println!("{err}");
                    errors.push(err.to_string());
                }
            }
        } else {
            let user_db_path = get_api_user_db_path(cfg);
            if user_db_path.exists() {
                // we cant have user defined in db file.
                // we need to load them and save them into the config file
                if let Ok(stored_users) = load_api_user(cfg) {
                    for stored_user in stored_users {
                        if let Some(target_user) = self.user.iter_mut().find(|t| t.target == stored_user.target) {
                            for stored_credential in &stored_user.credentials {
                                if !target_user.credentials.iter().any(|c| c.username == stored_credential.username) {
                                    target_user.credentials.push(stored_credential.clone());
                                }
                            }
                        } else {
                            self.user.push(stored_user);
                        }
                    }
                }
                let api_proxy_file = cfg.t_api_proxy_file_path.as_str();
                let backup_dir = cfg.backup_dir.as_ref().unwrap().as_str();
                if let Err(err) = config_reader::save_api_proxy(api_proxy_file, backup_dir, self) {
                    errors.push(format!("Error saving api proxy file: {err}"));
                } else {
                    backup_api_user_db_file(cfg, &user_db_path);
                    let _ = fs::remove_file(&user_db_path);
                }
            }
        }
    }

    fn prepare_server_config(&mut self, errors: &mut Vec<String>) {
        let mut name_set = HashSet::new();
        for server in &mut self.server {
            if let Err(err) = server.prepare() {
                errors.push(err.to_string());
            }
            if name_set.contains(server.name.as_str()) {
                errors.push(format!("Non-unique server info name found {}", &server.name));
            } else {
                name_set.insert(server.name.clone());
            }
        }
    }

    fn prepare_target_user(&mut self, errors: &mut Vec<String>) {
        let mut usernames = HashSet::new();
        let mut tokens = HashSet::new();
        for target_user in &mut self.user {
            for user in &mut target_user.credentials {
                user.prepare();
                if usernames.contains(&user.username) {
                    errors.push(format!("Non unique username found {}", &user.username));
                } else {
                    usernames.insert(user.username.to_string());
                }
                if let Some(token) = &user.token {
                    if token.is_empty() {
                        user.token = None;
                    } else if tokens.contains(token) {
                        errors.push(format!("Non unique token found {}", &user.username));
                    } else {
                        tokens.insert(token.to_string());
                    }
                }

                if let Some(server_info_name) = &user.server {
                    if !&self.server.iter()
                        .any(|server_info| server_info.name.eq(server_info_name))
                    {
                        errors.push(format!(
                            "No server info with name {} found for user {}",
                            server_info_name, &user.username
                        ));
                    }
                }
            }
        }
    }

    pub fn prepare(&mut self, cfg: &Config) -> Result<(), M3uFilterError> {
        let mut errors = Vec::new();
        if self.server.is_empty() {
            errors.push("No server info defined".to_string());
        } else {
            self.prepare_server_config(&mut errors);
        }
        self.prepare_target_user(&mut errors);
        self.migrate_api_user(cfg, &mut errors);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(info_err!(errors.join("\n")))
        }
    }

    pub fn get_target_name(
        &self,
        username: &str,
        password: &str,
    ) -> Option<(ProxyUserCredentials, String)> {
        for target_user in &self.user {
            if let Some((credentials, target_name)) =
                target_user.get_target_name(username, password)
            {
                return Some((credentials.clone(), target_name.to_string()));
            }
        }
        debug!("Could not find any target for user {username}");
        None
    }

    pub fn get_target_name_by_token(&self, token: &str) -> Option<(ProxyUserCredentials, String)> {
        for target_user in &self.user {
            if let Some((credentials, target_name)) = target_user.get_target_name_by_token(token) {
                return Some((credentials.clone(), target_name.to_string()));
            }
        }
        None
    }

    pub fn get_user_credentials(&self, username: &str) -> Option<ProxyUserCredentials> {
        let result = self.user.iter()
            .flat_map(|target_user| &target_user.credentials)
            .find(|credential| credential.username == username)
            .cloned();
        if result.is_none() && username != "test" {
            debug!("Could not find any user {username}");
        }
        result
    }
}
