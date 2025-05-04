#![allow(clippy::struct_excessive_bools)]
use enum_iterator::Sequence;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use log::{debug, error, warn};
use path_clean::PathClean;
use rand::Rng;

use crate::foundation::filter::{prepare_templates,  PatternTemplate};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::{ApiProxyConfig, ApiProxyServerInfo, ProxyUserCredentials, Mappings};
use crate::utils::{default_as_default, default_connect_timeout_secs, default_grace_period_millis, default_grace_period_timeout_secs};
use crate::utils::file_lock_manager::FileLockManager;
use crate::utils::file_utils;
use crate::utils::{parse_size_base_2, parse_to_kbps};
use crate::utils::exit;
use crate::m3u_filter_error::{ create_m3u_filter_error_result};
use crate::model::{ConfigInput, ConfigInputOptions, ConfigIpCheck, ConfigProxy, ConfigSource, ConfigTarget, HdHomeRunConfig, LogConfig, MessagingConfig, ProcessTargets, TargetOutput, VideoConfig, WebUiConfig};

const STREAM_QUEUE_SIZE: usize = 1024; // mpsc channel holding messages. with 8192byte chunks and 2Mbit/s approx 8MB


fn generate_secret() -> [u8; 32] {
    let mut rng = rand::rng();
    let mut secret = [0u8; 32];
    rng.fill(&mut secret);
    secret
}

#[macro_export]
macro_rules! valid_property {
  ($key:expr, $array:expr) => {{
        $array.contains(&$key)
    }};
}
pub use valid_property;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence, Eq, PartialEq)]
pub enum ItemField {
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "name")]
    Name,
    #[serde(rename = "title")]
    Title,
    #[serde(rename = "url")]
    Url,
    #[serde(rename = "input")]
    Input,
    #[serde(rename = "type")]
    Type,
    #[serde(rename = "caption")]
    Caption,
}

impl ItemField {
    const GROUP: &'static str = "Group";
    const NAME: &'static str = "Name";
    const TITLE: &'static str = "Title";
    const URL: &'static str = "Url";
    const INPUT: &'static str = "Input";
    const TYPE: &'static str = "Type";
    const CAPTION: &'static str = "Caption";
}

impl Display for ItemField {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", match *self {
            Self::Group => Self::GROUP,
            Self::Name => Self::NAME,
            Self::Title => Self::TITLE,
            Self::Url => Self::URL,
            Self::Input => Self::INPUT,
            Self::Type => Self::TYPE,
            Self::Caption => Self::CAPTION,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FilterMode {
    #[serde(rename = "discard")]
    Discard,
    #[serde(rename = "include")]
    Include,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigApi {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub web_root: String,
}

impl ConfigApi {
    pub fn prepare(&mut self) {
        if self.web_root.is_empty() {
            self.web_root = String::from("./web");
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigDto {
    #[serde(default)]
    pub threads: u8,
    pub api: ConfigApi,
    #[serde(default)]
    pub working_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<VideoConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedules: Option<Vec<ScheduleConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messaging: Option<MessagingConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<LogConfig>,
    #[serde(default)]
    pub update_on_boot: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_ui: Option<WebUiConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverse_proxy: Option<ReverseProxyConfig>,
}

impl ConfigDto {
    pub fn is_valid(&self) -> bool {
        if self.api.host.is_empty() {
            return false;
        }

        if let Some(video) = &self.video {
            if let Some(download) = &video.download {
                if let Some(episode_pattern) = &download.episode_pattern {
                    if !episode_pattern.is_empty() {
                        let re = regex::Regex::new(episode_pattern);
                        if re.is_err() {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }
}


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ScheduleConfig {
    #[serde(default)]
    pub schedule: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CacheConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    #[serde(skip)]
    pub t_size: usize,
}

impl CacheConfig {
    fn prepare(&mut self, working_dir: &str) {
        if self.enabled {
            let work_path = PathBuf::from(working_dir);
            if self.dir.is_none() {
                self.dir = Some(work_path.join("cache").to_string_lossy().to_string());
            } else {
                let mut cache_dir = self.dir.as_ref().unwrap().to_string();
                if PathBuf::from(&cache_dir).is_relative() {
                    cache_dir = work_path.join(&cache_dir).clean().to_string_lossy().to_string();
                }
                self.dir = Some(cache_dir.to_string());
            }
            match self.size.as_ref() {
                None => self.t_size = 1024,
                Some(val) => match parse_size_base_2(val) {
                    Ok(size) => self.t_size = usize::try_from(size).unwrap_or(0),
                    Err(err) => { exit!("{err}") }
                }
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct StreamBufferConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub size: usize,
}

impl StreamBufferConfig {
    fn prepare(&mut self) {
        if self.enabled && self.size == 0 {
            self.size = STREAM_QUEUE_SIZE;
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct StreamConfig {
    #[serde(default)]
    pub retry: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer: Option<StreamBufferConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub throttle: Option<String>,
    #[serde(default = "default_grace_period_millis")]
    pub grace_period_millis: u64,
    #[serde(default = "default_grace_period_timeout_secs")]
    pub grace_period_timeout_secs: u64,
    #[serde(default)]
    pub forced_retry_interval_secs: u32,
    #[serde(default, skip)]
    pub throttle_kbps: u64,
}

impl StreamConfig {
    fn prepare(&mut self) -> Result<(), M3uFilterError> {
        if let Some(buffer) = self.buffer.as_mut() {
            buffer.prepare();
        }
        if let Some(throttle) = &self.throttle {
            self.throttle_kbps = parse_to_kbps(throttle).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, err))?;
        }

        if self.grace_period_millis > 0 {
            if self.grace_period_timeout_secs == 0 {
                let triple_ms = self.grace_period_millis * 3;
                self.grace_period_timeout_secs = std::cmp::max(1, triple_ms.div_ceil(1000));
            } else if self.grace_period_millis / 1000 > self.grace_period_timeout_secs {
                return Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Grace time period timeout {} sec should be more than grace time period {} ms", self.grace_period_timeout_secs, self.grace_period_millis)));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub period_millis: u64,
    pub burst_size: u32,
}

impl RateLimitConfig {
    fn prepare(&self) -> Result<(), M3uFilterError> {
        if self.period_millis == 0 {
            return Err(M3uFilterError::new(M3uFilterErrorKind::Info, "Rate limiter period can't be 0".to_string()));
        }
        if self.burst_size == 0 {
            return Err(M3uFilterError::new(M3uFilterErrorKind::Info, "Rate limiter bust can't be 0".to_string()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ReverseProxyConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<StreamConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheConfig>,
    #[serde(default)]
    pub resource_rewrite_disabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,

}

impl ReverseProxyConfig {
    fn prepare(&mut self, working_dir: &str) -> Result<(), M3uFilterError> {
        if let Some(stream) = self.stream.as_mut() {
            stream.prepare()?;
        }
        if let Some(cache) = self.cache.as_mut() {
            if cache.enabled && self.resource_rewrite_disabled {
                warn!("The cache is disabled because resource rewrite is disabled");
                cache.enabled = false;
            }
            cache.prepare(working_dir);
        }

        if let Some(rate_limit) = self.rate_limit.as_mut() {
            if rate_limit.enabled {
                rate_limit.prepare()?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CustomStreamResponseConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_unavailable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_connections_exhausted: Option<String>, // user has no more connections
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_connections_exhausted: Option<String>, // provider limit reached, has no more connections
}


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub threads: u8,
    pub api: ConfigApi,
    pub sources: Vec<ConfigSource>,
    pub working_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_stream_response: Option<CustomStreamResponseConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates: Option<Vec<PatternTemplate>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<VideoConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedules: Option<Vec<ScheduleConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<LogConfig>,
    #[serde(default)]
    pub user_access_control: bool,
    #[serde(default = "default_connect_timeout_secs")]
    pub connect_timeout_secs: u32,
    #[serde(default)]
    pub update_on_boot: bool,
    #[serde(default)]
    pub web_ui: Option<WebUiConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messaging: Option<MessagingConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverse_proxy: Option<ReverseProxyConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hdhomerun: Option<HdHomeRunConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ConfigProxy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ipcheck: Option<ConfigIpCheck>,
    #[serde(skip)]
    pub t_api_proxy: Arc<RwLock<Option<ApiProxyConfig>>>,
    #[serde(skip)]
    pub t_config_path: String,
    #[serde(skip)]
    pub t_config_file_path: String,
    #[serde(skip)]
    pub t_sources_file_path: String,
    #[serde(skip)]
    pub t_api_proxy_file_path: String,
    #[serde(skip)]
    pub file_locks: Arc<FileLockManager>,
    #[serde(skip)]
    pub t_channel_unavailable_video: Option<Arc<Vec<u8>>>,
    #[serde(skip)]
    pub t_user_connections_exhausted_video: Option<Arc<Vec<u8>>>,
    #[serde(skip)]
    pub t_provider_connections_exhausted_video: Option<Arc<Vec<u8>>>,
    #[serde(skip)]
    pub t_access_token_secret: [u8; 32],
    #[serde(skip)]
    pub t_encrypt_secret: [u8; 16],
}

impl Config {
    pub async fn set_api_proxy(&mut self, api_proxy: Option<ApiProxyConfig>) -> Result<(), M3uFilterError> {
        self.t_api_proxy = Arc::new(RwLock::new(api_proxy));
        self.check_target_user().await
    }

    async fn check_username(&self, output_username: Option<&str>, target_name: &str) -> Result<(), M3uFilterError> {
        if let Some(username) = output_username {
            if let Some((_, config_target)) = self.get_target_for_username(username).await {
                if config_target.name != target_name {
                    return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "User:{username} does not belong to target: {}", target_name);
                }
            } else {
                return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "User: {username} does not exist");
            }
            Ok(())
        } else {
            Ok(())
        }
    }
    async fn check_target_user(&mut self) -> Result<(), M3uFilterError> {
        let check_homerun = self.hdhomerun.as_ref().is_some_and(|h| h.enabled);
        for source in &self.sources {
            for target in &source.targets {
                for output in &target.output {
                    match output {
                        TargetOutput::Xtream(_) | TargetOutput::M3u(_) => {}
                        TargetOutput::Strm(strm_output) => {
                            self.check_username(strm_output.username.as_deref(), &target.name).await?;
                        }
                        TargetOutput::HdHomeRun(hdhomerun_output) => {
                            if check_homerun {
                                let hdhr_name = &hdhomerun_output.device;
                                self.check_username(Some(&hdhomerun_output.username), &target.name).await?;
                                if let Some(homerun) = &mut self.hdhomerun {
                                    for device in &mut homerun.devices {
                                        if &device.name == hdhr_name {
                                            device.t_username.clone_from(&hdhomerun_output.username);
                                            device.t_enabled = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(hdhomerun) = &self.hdhomerun {
            for device in &hdhomerun.devices {
                if !device.t_enabled {
                    debug!("HdHomeRun device '{}' has no username and will be disabled", device.name);
                }
            }
        }
        Ok(())
    }

    pub fn is_reverse_proxy_resource_rewrite_enabled(&self) -> bool {
        self.reverse_proxy.as_ref().is_none_or(|r| !r.resource_rewrite_disabled)
    }

    fn intern_get_target_for_user(&self, user_target: Option<(ProxyUserCredentials, String)>) -> Option<(ProxyUserCredentials, &ConfigTarget)> {
        match user_target {
            Some((user, target_name)) => {
                for source in &self.sources {
                    for target in &source.targets {
                        if target_name.eq_ignore_ascii_case(&target.name) {
                            return Some((user, target));
                        }
                    }
                }
                None
            }
            None => None
        }
    }

    pub fn get_inputs_for_target(&self, target_name: &str) -> Option<Vec<&ConfigInput>> {
        for source in &self.sources {
            if let Some(cfg) = source.get_inputs_for_target(target_name) {
                return Some(cfg);
            }
        }
        None
    }

    pub async fn get_target_for_username(&self, username: &str) -> Option<(ProxyUserCredentials, &ConfigTarget)> {
        if let Some(credentials) = self.get_user_credentials(username).await {
            return self.t_api_proxy.read().await.as_ref()
                .and_then(|api_proxy| self.intern_get_target_for_user(api_proxy.get_target_name(&credentials.username, &credentials.password)));
        }
        None
    }

    pub async fn get_target_for_user(&self, username: &str, password: &str) -> Option<(ProxyUserCredentials, &ConfigTarget)> {
        self.t_api_proxy.read().await.as_ref().and_then(|api_proxy| self.intern_get_target_for_user(api_proxy.get_target_name(username, password)))
    }

    pub async fn get_target_for_user_by_token(&self, token: &str) -> Option<(ProxyUserCredentials, &ConfigTarget)> {
        self.t_api_proxy.read().await.as_ref().and_then(|api_proxy| self.intern_get_target_for_user(api_proxy.get_target_name_by_token(token)))
    }

    pub async fn get_user_credentials(&self, username: &str) -> Option<ProxyUserCredentials> {
        self.t_api_proxy.read().await.as_ref().and_then(|api_proxy| api_proxy.get_user_credentials(username))
    }

    pub fn get_input_by_name(&self, input_name: &str) -> Option<&ConfigInput> {
        for source in &self.sources {
            for input in &source.inputs {
                if input.name == input_name {
                    return Some(input);
                }
            }
        }
        None
    }

    pub fn get_input_options_by_name(&self, input_name: &str) -> Option<&ConfigInputOptions> {
        for source in &self.sources {
            for input in &source.inputs {
                if input.name == input_name {
                    return input.options.as_ref();
                }
            }
        }
        None
    }

    pub fn get_input_by_id(&self, input_id: u16) -> Option<&ConfigInput> {
        for source in &self.sources {
            for input in &source.inputs {
                if input.id == input_id {
                    return Some(input);
                }
            }
        }
        None
    }

    pub fn get_target_by_id(&self, target_id: u16) -> Option<&ConfigTarget> {
        for source in &self.sources {
            for target in &source.targets {
                if target.id == target_id {
                    return Some(target);
                }
            }
        }
        None
    }

    pub fn set_mappings(&mut self, mappings_cfg: &Mappings) {
        for source in &mut self.sources {
            for target in &mut source.targets {
                if let Some(mapping_ids) = &target.mapping {
                    let mut target_mappings = Vec::with_capacity(128);
                    for mapping_id in mapping_ids {
                        let mapping = mappings_cfg.get_mapping(mapping_id);
                        if let Some(mappings) = mapping {
                            target_mappings.push(mappings);
                        }
                    }
                    target.t_mapping = if target_mappings.is_empty() { None } else { Some(target_mappings) };
                }
            }
        }
    }

    fn check_unique_input_names(&mut self) -> Result<(), M3uFilterError> {
        let mut seen_names = HashSet::new();
        for source in &mut self.sources {
            for input in &source.inputs {
                let input_name = input.name.trim().to_string();
                if input_name.is_empty() {
                    return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "input name required");
                }
                if seen_names.contains(input_name.as_str()) {
                    return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "input names should be unique: {}", input_name);
                }
                seen_names.insert(input_name);
                if let Some(aliases) = &input.aliases {
                    for alias in aliases {
                        let input_name = alias.name.trim().to_string();
                        if input_name.is_empty() {
                            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "input name required");
                        }
                        if seen_names.contains(input_name.as_str()) {
                            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "input names should be unique: {}", input_name);
                        }
                        seen_names.insert(input_name);
                    }
                }
            }
        }
        Ok(())
    }

    fn check_unique_target_names(&mut self) -> Result<HashSet<String>, M3uFilterError> {
        let mut seen_names = HashSet::new();
        let default_target_name = default_as_default();
        for source in &self.sources {
            for target in &source.targets {
                // check target name is unique
                let target_name = target.name.trim().to_string();
                if target_name.is_empty() {
                    return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "target name required");
                }
                if !default_target_name.eq_ignore_ascii_case(target_name.as_str()) {
                    if seen_names.contains(target_name.as_str()) {
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "target names should be unique: {}", target_name);
                    }
                    seen_names.insert(target_name);
                }
            }
        }
        Ok(seen_names)
    }

    fn check_scheduled_targets(&mut self, target_names: &HashSet<String>) -> Result<(), M3uFilterError> {
        if let Some(schedules) = &self.schedules {
            for schedule in schedules {
                if let Some(targets) = &schedule.targets {
                    for target_name in targets {
                        if !target_names.contains(target_name) {
                            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unknown target name in scheduler: {}", target_name);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /**
    *  if `include_computed` set to true for `app_state`
    */
    pub fn prepare(&mut self, include_computed: bool) -> Result<(), M3uFilterError> {
        let work_dir = &self.working_dir;
        self.working_dir = file_utils::get_working_path(work_dir);
        if include_computed {
            self.t_access_token_secret = generate_secret();
            self.t_encrypt_secret = <&[u8] as TryInto<[u8; 16]>>::try_into(&generate_secret()[0..16]).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()))?;
            self.prepare_custom_stream_response();
        }
        self.prepare_directories();
        if let Some(reverse_proxy) = self.reverse_proxy.as_mut() {
            reverse_proxy.prepare(&self.working_dir)?;
        }
        if let Some(proxy) = &mut self.proxy {
            proxy.prepare()?;
        }
        if let Some(ipcheck) = self.ipcheck.as_mut() {
            ipcheck.prepare()?;
        }
        self.prepare_hdhomerun()?;
        self.api.prepare();
        self.prepare_api_web_root();
        self.prepare_templates()?;
        self.prepare_sources(include_computed)?;
        let target_names = self.check_unique_target_names()?;
        self.check_scheduled_targets(&target_names)?;
        self.check_unique_input_names()?;
        self.prepare_video_config()?;
        self.prepare_web()?;

        Ok(())
    }

    fn prepare_directories(&mut self) {
        fn set_directory(path: &mut Option<String>, default_subdir: &str, working_dir: &str) {
            *path = Some(match path.as_ref() {
                Some(existing) => existing.to_owned(),
                None => PathBuf::from(working_dir).join(default_subdir).clean().to_string_lossy().to_string(),
            });
        }

        set_directory(&mut self.backup_dir, "backup", &self.working_dir);
        set_directory(&mut self.user_config_dir, "user_config", &self.working_dir);
    }

    fn prepare_hdhomerun(&mut self) -> Result<(), M3uFilterError> {
        if let Some(hdhomerun) = self.hdhomerun.as_mut() {
            if hdhomerun.enabled {
                hdhomerun.prepare(self.api.port)?;
            }
        }
        Ok(())
    }

    fn prepare_sources(&mut self, include_computed: bool) -> Result<(), M3uFilterError> {
        // prepare sources and set id's
        let mut source_index: u16 = 1;
        let mut target_index: u16 = 1;
        for source in &mut self.sources {
            source_index = source.prepare(source_index, include_computed)?;
            for target in &mut source.targets {
                // prepare target templates
                let prepare_result = match &self.templates {
                    Some(templ) => target.prepare(target_index, Some(templ)),
                    _ => target.prepare(target_index, None)
                };
                prepare_result?;
                target_index += 1;
            }
        }
        Ok(())
    }

    fn prepare_templates(&mut self) -> Result<(), M3uFilterError> {
        if let Some(templates) = &mut self.templates {
            match prepare_templates(templates) {
                Ok(tmplts) => {
                    self.templates = Some(tmplts);
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    fn prepare_web(&mut self) -> Result<(), M3uFilterError> {
        if let Some(web_ui_config) = self.web_ui.as_mut() {
            web_ui_config.prepare(&self.t_config_path)?;
        }
        Ok(())
    }

    fn prepare_video_config(&mut self) -> Result<(), M3uFilterError> {
        match &mut self.video {
            None => {
                self.video = Some(VideoConfig {
                    extensions: vec!["mkv".to_string(), "avi".to_string(), "mp4".to_string()],
                    download: None,
                    web_search: None,
                });
            }
            Some(video) => {
                match video.prepare() {
                    Ok(()) => {}
                    Err(err) => return Err(err)
                }
            }
        }
        Ok(())
    }

    fn prepare_custom_stream_response(&mut self) {
        if let Some(custom_stream_response) = self.custom_stream_response.as_ref() {
            fn load_and_set_file(path: Option<&String>, working_dir: &str) -> Option<Arc<Vec<u8>>> {
                path.as_ref()
                    .map(|file| file_utils::make_absolute_path(file, working_dir))
                    .and_then(|absolute_path| match file_utils::read_file_as_bytes(&PathBuf::from(&absolute_path)) {
                        Ok(data) => Some(Arc::new(data)),
                        Err(err) => {
                            error!("Failed to load file: {absolute_path} {err}");
                            None
                        }
                    })
            }

            self.t_channel_unavailable_video = load_and_set_file(custom_stream_response.channel_unavailable.as_ref(), &self.working_dir);
            self.t_user_connections_exhausted_video = load_and_set_file(custom_stream_response.user_connections_exhausted.as_ref(), &self.working_dir);
            self.t_provider_connections_exhausted_video = load_and_set_file(custom_stream_response.provider_connections_exhausted.as_ref(), &self.working_dir);
        }
    }

    fn prepare_api_web_root(&mut self) {
        if !self.api.web_root.is_empty() {
            self.api.web_root = file_utils::make_absolute_path(&self.api.web_root, &self.working_dir);
        }
    }

    /// # Panics
    ///
    /// Will panic if default server invalid
    pub async fn get_server_info(&self, server_info_name: &str) -> ApiProxyServerInfo {
        let server_info_list = self.t_api_proxy.read().await.as_ref().unwrap().server.clone();
        server_info_list.iter().find(|c| c.name.eq(server_info_name)).map_or_else(|| server_info_list.first().unwrap().clone(), Clone::clone)
    }

    pub async fn get_user_server_info(&self, user: &ProxyUserCredentials) -> ApiProxyServerInfo {
        let server_info_name = user.server.as_ref().map_or("default", |server_name| server_name.as_str());
        self.get_server_info(server_info_name).await
    }
}

/// Returns the targets that were specified as parameters.
/// If invalid targets are found, the program will be terminated.
/// The return value has `enabled` set to true, if selective targets should be processed, otherwise false.
///
/// * `target_args` the program parameters given with `-target` parameter.
/// * `sources` configured sources in config file
///
pub fn validate_targets(target_args: Option<&Vec<String>>, sources: &Vec<ConfigSource>) -> Result<ProcessTargets, M3uFilterError> {
    let mut enabled = true;
    let mut inputs: Vec<u16> = vec![];
    let mut targets: Vec<u16> = vec![];
    if let Some(user_targets) = target_args {
        let mut check_targets: HashMap<String, u16> = user_targets.iter().map(|t| (t.to_lowercase(), 0)).collect();
        for source in sources {
            let mut target_added = false;
            for target in &source.targets {
                for user_target in user_targets {
                    let key = user_target.to_lowercase();
                    if target.name.eq_ignore_ascii_case(key.as_str()) {
                        targets.push(target.id);
                        target_added = true;
                        if let Some(value) = check_targets.get(key.as_str()) {
                            check_targets.insert(key, value + 1);
                        }
                    }
                }
            }
            if target_added {
                source.inputs.iter().map(|i| i.id).for_each(|id| inputs.push(id));
            }
        }

        let missing_targets: Vec<String> = check_targets.iter().filter(|&(_, v)| *v == 0).map(|(k, _)| k.to_string()).collect();
        if !missing_targets.is_empty() {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "No target found for {}", missing_targets.join(", "));
        }
        // let processing_targets: Vec<String> = check_targets.iter().filter(|&(_, v)| *v != 0).map(|(k, _)| k.to_string()).collect();
        // info!("Processing targets {}", processing_targets.join(", "));
    } else {
        enabled = false;
    }

    Ok(ProcessTargets {
        enabled,
        inputs,
        targets,
    })
}
