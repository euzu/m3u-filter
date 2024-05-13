use enum_iterator::Sequence;
use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::fs::File;
use std::io::BufRead;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, RwLock};

use log::{debug, error, warn};
use path_absolutize::Absolutize;
use crate::auth::user::UserCredential;

use crate::filter::{Filter, get_filter, MockValueProcessor, PatternTemplate, prepare_templates, ValueProvider};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::messaging::MsgKind;
use crate::model::api_proxy::{ApiProxyConfig, ProxyUserCredentials};
use crate::model::mapping::Mapping;
use crate::model::mapping::Mappings;
use crate::utils::{config_reader, file_utils};
use crate::utils::default_utils::{default_as_default, default_as_false, default_as_true,
                                  default_as_empty_list, default_as_frm, default_as_empty_map,
                                  default_as_zero_u8, default_as_two_u16};

pub(crate) const MAPPER_ATTRIBUTE_FIELDS: &[&str] = &[
    "name", "title", "group", "id", "logo",
    "logo_small", "parent_code", "audio_track",
    "time_shift", "rec", "url",
];
pub(crate) const AFFIX_FIELDS: &[&str] = &["name", "title", "group"];

#[macro_export]
macro_rules! valid_property {
  ($key:expr, $array:expr) => {{
        $array.contains(&$key)
    }};
}

#[macro_export]
macro_rules! create_m3u_filter_error_result {
     ($kind: expr, $($arg:tt)*) => {
        Err(M3uFilterError::new($kind, format!($($arg)*)))
    }
}

#[macro_export]
macro_rules! handle_m3u_filter_error_result_list {
    ($kind:expr, $result: expr) => {
        let errors = $result
            .filter_map(|result| {
                if let Err(err) = result {
                    Some(err.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();
        if !&errors.is_empty() {
            return Err(M3uFilterError::new($kind, errors.join("\n")));
        }
    }
}

#[macro_export]
macro_rules! handle_m3u_filter_error_result {
    ($kind:expr, $result: expr) => {
        if let Err(err) = $result {
            return Err(M3uFilterError::new($kind, err.to_string()));
        }
    }
}


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence, PartialEq, Eq, Hash)]
pub(crate) enum TargetType {
    #[serde(rename = "m3u")]
    M3u,
    #[serde(rename = "strm")]
    Strm,
    #[serde(rename = "xtream")]
    Xtream,
}

impl Display for TargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Self::M3u => write!(f, "M3u"),
            Self::Strm => write!(f, "Strm"),
            Self::Xtream => write!(f, "Xtream"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence, PartialEq)]
pub(crate) enum ProcessingOrder {
    #[serde(rename = "frm")]
    Frm,
    #[serde(rename = "fmr")]
    Fmr,
    #[serde(rename = "rfm")]
    Rfm,
    #[serde(rename = "rmf")]
    Rmf,
    #[serde(rename = "mfr")]
    Mfr,
    #[serde(rename = "mrf")]
    Mrf,
}

impl std::fmt::Display for ProcessingOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Self::Frm => write!(f, "filter, rename, map"),
            Self::Fmr => write!(f, "filter, map, rename"),
            Self::Rfm => write!(f, "rename, filter, map"),
            Self::Rmf => write!(f, "rename, map, filter"),
            Self::Mfr => write!(f, "map, filter, rename"),
            Self::Mrf => write!(f, "map, rename, filter"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence)]
pub(crate) enum ItemField {
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "name")]
    Name,
    #[serde(rename = "title")]
    Title,
    #[serde(rename = "url")]
    Url,
}

impl Display for ItemField {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Self::Group => write!(f, "Group"),
            Self::Name => write!(f, "Name"),
            Self::Title => write!(f, "Title"),
            Self::Url => write!(f, "Url"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) enum FilterMode {
    #[serde(rename = "discard")]
    Discard,
    #[serde(rename = "include")]
    Include,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) enum SortOrder {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}


#[derive(Clone)]
pub(crate) struct ProcessTargets {
    pub enabled: bool,
    pub inputs: Vec<u16>,
    pub targets: Vec<u16>,
}

impl ProcessTargets {
    pub fn has_target(&self, tid: u16) -> bool {
        matches!(self.targets.iter().position(|&x| x == tid), Some(_pos))
    }

    pub fn has_input(&self, tid: u16) -> bool {
        matches!(self.inputs.iter().position(|&x| x == tid), Some(_pos))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigSortGroup {
    pub order: SortOrder,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigSortChannel {
    pub field: ItemField,
    // channel field
    pub group_pattern: String,
    // match against group title
    pub order: SortOrder,
    #[serde(skip_serializing, skip_deserializing)]
    pub re: Option<regex::Regex>,
}

impl ConfigSortChannel {
    pub(crate) fn prepare(&mut self) -> Result<(), M3uFilterError> {
        let re = regex::Regex::new(&self.group_pattern);
        if re.is_err() {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse regex: {}", &self.group_pattern);
        }
        self.re = Some(re.unwrap());
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigSort {
    #[serde(default = "default_as_false")]
    pub match_as_ascii: bool,
    pub groups: Option<ConfigSortGroup>,
    pub channels: Option<Vec<ConfigSortChannel>>,
}

impl ConfigSort {
    pub(crate) fn prepare(&mut self) -> Result<(), M3uFilterError> {
        if let Some(channels) = self.channels.as_mut() {
            handle_m3u_filter_error_result_list!(M3uFilterErrorKind::Info, channels.iter_mut().map(ConfigSortChannel::prepare));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigRename {
    pub field: ItemField,
    pub pattern: String,
    pub new_name: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub re: Option<regex::Regex>,
}

impl ConfigRename {
    pub fn prepare(&mut self) -> Result<(), M3uFilterError> {
        let re = regex::Regex::new(&self.pattern);
        if re.is_err() {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse regex: {}", &self.pattern);
        }
        self.re = Some(re.unwrap());
        Ok(())
    }
}


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigTargetOptions {
    #[serde(default = "default_as_false")]
    pub ignore_logo: bool,
    #[serde(default = "default_as_false")]
    pub underscore_whitespace: bool,
    #[serde(default = "default_as_false")]
    pub cleanup: bool,
    #[serde(default = "default_as_false")]
    pub kodi_style: bool,
    #[serde(default = "default_as_true")]
    pub xtream_skip_live_direct_source: bool,
    #[serde(default = "default_as_true")]
    pub xtream_skip_video_direct_source: bool,
    #[serde(default = "default_as_true")]
    pub xtream_skip_series_direct_source: bool,
    #[serde(default = "default_as_false")]
    pub xtream_resolve_series: bool,
    #[serde(default = "default_as_two_u16")]
    pub xtream_resolve_series_delay: u16,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TargetOutput {
    #[serde(alias = "type")]
    pub target: TargetType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigTarget {
    #[serde(skip)]
    pub id: u16,
    #[serde(default = "default_as_true")]
    pub enabled: bool,
    #[serde(default = "default_as_default")]
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<ConfigTargetOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<ConfigSort>,
    pub filter: String,
    #[serde(alias = "type", default = "default_as_empty_list")]
    pub output: Vec<TargetOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rename: Option<Vec<ConfigRename>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping: Option<Vec<String>>,
    #[serde(default = "default_as_frm")]
    pub processing_order: ProcessingOrder,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch: Option<Vec<String>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_watch_re: Option<Vec<regex::Regex>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_filter: Option<Filter>,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_mapping: Option<Vec<Mapping>>,
}


impl ConfigTarget {
    pub(crate) fn prepare(&mut self, id: u16, templates: Option<&Vec<PatternTemplate>>) -> Result<(), M3uFilterError> {
        self.id = id;
        if self.output.is_empty() {
            return Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Missing output format for {}", self.name)));
        }
        let mut m3u_cnt = 0;
        let mut strm_cnt = 0;
        let mut xtream_cnt = 0;
        for format in &self.output {
            match format.target {
                TargetType::M3u => {
                    m3u_cnt += 1;
                }
                TargetType::Strm => {
                    strm_cnt += 1;
                    if format.filename.is_none() {
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "filename is required for strm type: {}", self.name);
                    }
                }
                TargetType::Xtream => {
                    xtream_cnt += 1;
                    if default_as_default().eq_ignore_ascii_case(&self.name) {
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "unique target name is required for xtream type: {}", self.name);
                    }
                    if let Some(fname) = &format.filename {
                        if !fname.trim().is_empty() {
                            warn!("Filename for target output xtream is ignored: {}", self.name);
                        }
                    }
                }
            }
        }

        if m3u_cnt > 1 || strm_cnt > 1 || xtream_cnt > 1 {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Multiple output formats with same type : {}", self.name);
        }

        if let Some(watch) = &self.watch {
            let regexps: Result<Vec<regex::Regex>, _> = watch.iter().map(|s| regex::Regex::new(s)).collect();
            match regexps {
                Ok(watch_re) => self.t_watch_re = Some(watch_re),
                Err(err) => {
                    return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Invalid watch regular expression: {}", err);
                }
            }
        }

        match get_filter(&self.filter, templates) {
            Ok(fltr) => {
                debug!("Filter: {}", fltr);
                self.t_filter = Some(fltr);
                if let Some(renames) = self.rename.as_mut() {
                    handle_m3u_filter_error_result_list!(M3uFilterErrorKind::Info, renames.iter_mut().map(ConfigRename::prepare));
                }
                if let Some(sort) = self.sort.as_mut() {
                    handle_m3u_filter_error_result!(M3uFilterErrorKind::Info, sort.prepare());
                }
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub(crate) fn filter(&self, provider: &ValueProvider) -> bool {
        let mut processor = MockValueProcessor {};
        return self.t_filter.as_ref().unwrap().filter(provider, &mut processor);
    }

    pub(crate) fn get_m3u_filename(&self) -> Option<&String> {
        for format in &self.output {
            if let TargetType::M3u = format.target {
                return format.filename.as_ref();
            }
        }
        None
    }

    pub(crate) fn has_output(&self, tt: &TargetType) -> bool {
        for format in &self.output {
            if tt.eq(&format.target) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigSource {
    pub inputs: Vec<ConfigInput>,
    pub targets: Vec<ConfigTarget>,
}

impl ConfigSource {
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn prepare(&mut self, index: u16) -> Result<u16, M3uFilterError> {
        handle_m3u_filter_error_result_list!(M3uFilterErrorKind::Info, self.inputs.iter_mut().enumerate().map(|(idx, i)| i.prepare(index+(idx as u16))));
        Ok(index + (self.inputs.len() as u16))
    }

    pub(crate) fn get_inputs_for_target(&self, target_name: &str) -> Option<Vec<&ConfigInput>> {
        for target in &self.targets {
            if target.name.eq(target_name) {
                let inputs = self.inputs.iter().filter(|&i| i.enabled).collect::<Vec<&ConfigInput>>();
                if !inputs.is_empty() {
                    return Some(inputs);
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct InputAffix {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence, PartialEq)]
pub(crate) enum InputType {
    #[serde(rename = "m3u")]
    M3u,
    #[serde(rename = "xtream")]
    Xtream,
}

impl Display for InputType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            InputType::M3u => "m3u",
            InputType::Xtream => "xtream"
        })
    }
}

impl FromStr for InputType {
    type Err = M3uFilterError;

    fn from_str(s: &str) -> Result<Self, M3uFilterError> {
        if s.eq("m3u") {
            Ok(InputType::M3u)
        } else if s.eq("xtream") {
            Ok(InputType::Xtream)
        } else {
            create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unkown InputType: {}", s)
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigInputOptions {
    #[serde(default = "default_as_false")]
    pub xtream_info_cache: bool,
    #[serde(default = "default_as_false")]
    pub xtream_skip_live: bool,
    #[serde(default = "default_as_false")]
    pub xtream_skip_vod: bool,
    #[serde(default = "default_as_false")]
    pub xtream_skip_series: bool,
}

pub(crate) struct InputUserInfo {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

fn default_as_type_m3u() -> InputType { InputType::M3u }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigInput {
    #[serde(skip)]
    pub id: u16,
    #[serde(rename = "type", default = "default_as_type_m3u")]
    pub input_type: InputType,
    #[serde(default = "default_as_empty_map")]
    pub headers: HashMap<String, String>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epg_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<InputAffix>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<InputAffix>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "default_as_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<ConfigInputOptions>,

}

impl ConfigInput {
    pub fn prepare(&mut self, id: u16) -> Result<(), M3uFilterError> {
        self.id = id;
        if self.url.trim().is_empty() {
            return Err(M3uFilterError::new(M3uFilterErrorKind::Info, "url for input is mandatory".to_string()));
        }
        if let Some(user_name) = &self.username {
            if user_name.trim().is_empty() {
                self.username = None;
            }
        }
        if let Some(password) = &self.password {
            if password.trim().is_empty() {
                self.password = None;
            }
        }
        match self.input_type {
            InputType::M3u => {
                if self.username.is_none() || self.password.is_none() {
                    debug!("for input type m3u: username and password are ignored");
                }
            }
            InputType::Xtream => {
                if self.username.is_none() || self.password.is_none() {
                    return Err(M3uFilterError::new(M3uFilterErrorKind::Info, "for input type xtream: username and password are mandatory".to_string()));
                }
            }
        }
        if let Some(persist_path) = &self.persist {
            if persist_path.trim().is_empty() {
                self.persist = None;
            }
        }
        Ok(())
    }

    pub(crate) fn get_user_info(&self) -> Option<InputUserInfo> {
        if self.input_type == InputType::Xtream {
            if self.username.is_some() || self.password.is_some() {
                return Some(InputUserInfo {
                    base_url: self.url.clone(),
                    username: self.username.as_ref().unwrap().to_owned(),
                    password: self.password.as_ref().unwrap().to_owned(),
                });
            }
        } else if let Ok(url) = url::Url::parse(&self.url) {
            let base_url = url.origin().ascii_serialization();
            let mut username = None;
            let mut password = None;
            for (key, value) in url.query_pairs() {
                if key.eq("username") {
                    username = Some(value.into_owned());
                } else if key.eq("password") {
                    password = Some(value.into_owned());
                }
            }
            if username.is_some() || password.is_some() {
                return Some(InputUserInfo {
                    base_url,
                    username: username.as_ref().unwrap().to_owned(),
                    password: password.as_ref().unwrap().to_owned(),
                });
            }
        }
        None
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigApi {
    pub host: String,
    pub port: u16,
    pub web_root: String,
}

impl ConfigApi {
    pub fn prepare(&mut self) {
        if self.web_root.is_empty() {
            self.web_root = String::from("./web");
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TelegramMessagingConfig {
    pub bot_token: String,
    pub chat_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct RestMessagingConfig {
    pub url: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct MessagingConfig {
    #[serde(default = "default_as_empty_list")]
    pub notify_on: Vec<MsgKind>,
    pub telegram: Option<TelegramMessagingConfig>,
    pub rest: Option<RestMessagingConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct VideoDownloadConfig {
    #[serde(default = "default_as_empty_map")]
    pub headers: HashMap<String, String>,
    pub directory: Option<String>,
    #[serde(default = "default_as_false")]
    pub organize_into_directories: bool,
    pub episode_pattern: Option<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_re_episode_pattern: Option<regex::Regex>,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_re_filename: Option<regex::Regex>,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_re_remove_filename_ending: Option<regex::Regex>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct VideoConfig {
    #[serde(default = "default_as_empty_list")]
    pub extensions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download: Option<VideoDownloadConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search: Option<String>,
}

impl VideoConfig {
    pub fn prepare(&mut self) -> Result<(), M3uFilterError> {
        self.extensions = vec!["mkv".to_string(), "avi".to_string(), "mp4".to_string()];
        match &mut self.download {
            None => {}
            Some(downl) => {
                if downl.headers.is_empty() {
                    downl.headers.borrow_mut().insert("Accept".to_string(), "video/*".to_string());
                    downl.headers.borrow_mut().insert("User-Agent".to_string(), "AppleTV/tvOS/9.1.1.".to_string());
                }

                if let Some(episode_pattern) = &downl.episode_pattern {
                    if !episode_pattern.is_empty() {
                        let re = regex::Regex::new(episode_pattern);
                        if re.is_err() {
                            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse regex: {}", episode_pattern);
                        }
                        downl.t_re_episode_pattern = Some(re.unwrap());
                    }
                }

                downl.t_re_filename = Some(regex::Regex::new(r"[^A-Za-z0-9_.-]").unwrap());
                downl.t_re_remove_filename_ending = Some(regex::Regex::new(r"[_.\s-]$").unwrap());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigDto {
    #[serde(default = "default_as_zero_u8")]
    pub threads: u8,
    pub api: ConfigApi,
    pub working_dir: String,
    pub backup_dir: Option<String>,
    pub video: Option<VideoConfig>,
    pub schedule: Option<String>,
    pub messaging: Option<MessagingConfig>,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct WebAuthConfig {
    #[serde(default = "default_as_true")]
    pub enabled: bool,
    pub issuer: String,
    pub secret: String,
    pub userfile: Option<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_users: Option<Vec<UserCredential>>,
}

impl WebAuthConfig {
    pub fn prepare(&mut self, config_path: &str, resolve_var: bool) -> Result<(), M3uFilterError> {
        if resolve_var {
            self.issuer = config_reader::resolve_env_var(&self.issuer);
            self.secret = config_reader::resolve_env_var(&self.secret);
            if let Some(file) = &self.userfile {
                self.userfile = Some(config_reader::resolve_env_var(file));
            }
        }
        let userfile_name = match &self.userfile {
            None => file_utils::get_default_user_file_path(config_path),
            Some(file) => file.to_owned()
        };
        self.userfile = Some(userfile_name.clone());

        let mut userfile_path = PathBuf::from(&userfile_name);
        if !file_utils::path_exists(&userfile_path) {
            userfile_path = PathBuf::from(config_path).join(&userfile_name);
            if !file_utils::path_exists(&userfile_path) {
                return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Could not find userfile {}", &userfile_name);
            }
        }

        if let Ok(file) = File::open(&userfile_path) {
            let mut users = vec![];
            let reader = std::io::BufReader::new(file);
            for credentials in reader.lines().map_while(Result::ok) {
                let mut parts = credentials.split(':');
                if let (Some(username), Some(password)) = (parts.next(), parts.next()) {
                    users.push(UserCredential {
                        username: username.trim().to_string(),
                        password: password.trim().to_string(),
                    });
                    debug!("Read ui user {}", username);
                }
            }

            self.t_users = Some(users);
        } else {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Could not read userfile {:?}", &userfile_path);
        }
        Ok(())
    }

    pub fn get_user_password(&self, username: &str) -> Option<&str> {
        if let Some(users) = &self.t_users {
            for credential in users {
                if credential.username.eq_ignore_ascii_case(username) {
                    return Some(credential.password.as_str());
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Config {
    #[serde(default = "default_as_zero_u8")]
    pub threads: u8,
    pub api: ConfigApi,
    pub sources: Vec<ConfigSource>,
    pub working_dir: String,
    pub backup_dir: Option<String>,
    pub templates: Option<Vec<PatternTemplate>>,
    pub video: Option<VideoConfig>,
    pub schedule: Option<String>,
    #[serde(default = "default_as_false")]
    pub update_on_boot: bool,
    #[serde(default = "default_as_true")]
    pub web_ui_enabled: bool,
    pub web_auth: Option<WebAuthConfig>,
    pub messaging: Option<MessagingConfig>,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_api_proxy: Arc<RwLock<Option<ApiProxyConfig>>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_config_path: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_config_file_path: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_sources_file_path: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_api_proxy_file_path: String,

}

impl Config {
    pub fn set_api_proxy(&mut self, api_proxy: Option<ApiProxyConfig>) {
        self.t_api_proxy = Arc::new(RwLock::new(api_proxy));
    }

    fn _get_target_for_user(&self, user_target: Option<(ProxyUserCredentials, String)>) -> Option<(ProxyUserCredentials, &ConfigTarget)> {
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

    pub(crate) fn get_inputs_for_target(&self, target_name: &str) -> Option<Vec<&ConfigInput>> {
        for source in &self.sources {
            if let Some(cfg) = source.get_inputs_for_target(target_name) {
                return Some(cfg);
            }
        }
        None
    }

    pub fn get_target_for_user(&self, username: &str, password: &str) -> Option<(ProxyUserCredentials, &ConfigTarget)> {
        match self.t_api_proxy.read().unwrap().as_ref() {
            Some(api_proxy) => {
                self._get_target_for_user(api_proxy.get_target_name(username, password))
            }
            _ => None
        }
    }

    pub fn get_target_for_user_by_token(&self, token: &str) -> Option<(ProxyUserCredentials, &ConfigTarget)> {
        match self.t_api_proxy.read().unwrap().as_ref() {
            Some(api_proxy) => {
                self._get_target_for_user(api_proxy.get_target_name_by_token(token))
            }
            _ => None
        }
    }

    pub(crate) fn get_input_by_id(&self, input_id: u16) -> Option<&ConfigInput> {
        for source in &self.sources {
            for input in &source.inputs {
                if input.id == input_id {
                    return Some(input);
                }
            }
        }
        None
    }

    pub(crate) fn set_mappings(&mut self, mappings: Option<Mappings>) {
        if let Some(mapping_list) = mappings {
            for source in &mut self.sources {
                for target in &mut source.targets {
                    if let Some(mapping_ids) = &target.mapping {
                        let mut target_mappings = Vec::new();
                        for mapping_id in mapping_ids {
                            let mapping = mapping_list.get_mapping(mapping_id);
                            if let Some(mappings) = mapping {
                                target_mappings.push(mappings);
                            }
                        }
                        target.t_mapping = if target_mappings.is_empty() { None } else { Some(target_mappings) };
                    }
                }
            }
        }
    }

    pub fn prepare(&mut self, resolve_var: bool) -> Result<(), M3uFilterError> {
        self.working_dir = file_utils::get_working_path(&self.working_dir);
        if self.backup_dir.is_none() {
            self.backup_dir = Some(PathBuf::from(&self.working_dir).join(".backup").into_os_string().to_string_lossy().to_string());
        }
        let backupdir = PathBuf::from(self.backup_dir.as_ref().unwrap());
        if !backupdir.exists() {
            match std::fs::create_dir(backupdir) {
                Ok(()) => {}
                Err(err) => { error!("Could not create backup dir {} {}", self.backup_dir.as_ref().unwrap(), err) }
            }
        }
        self.api.prepare();
        self.prepare_api_web_root();
        if let Some(templates) = &mut self.templates {
            match prepare_templates(templates) {
                Ok(tmplts) => {
                    self.templates = Some(tmplts);
                }
                Err(err) => {
                    return Err(err);
                }
            }
        };
        // prepare sources and set id's
        let mut target_names_check = HashSet::<String>::new();
        let default_target_name = default_as_default();
        let mut source_index: u16 = 1;
        let mut target_index: u16 = 1;
        for source in &mut self.sources {
            source_index = source.prepare(source_index)?;
            for target in &mut source.targets {
                // check target name is unique
                let target_name = target.name.trim().to_string();
                if target_name.is_empty() {
                    return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "target name required");
                }
                if !default_target_name.eq_ignore_ascii_case(target_name.as_str()) {
                    if target_names_check.contains(target_name.as_str()) {
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "target names should be unique: {}", target_name);
                    }
                    target_names_check.insert(target_name);
                }
                // prepare templaes
                let prepare_result = match &self.templates {
                    Some(templ) => target.prepare(target_index, Some(templ)),
                    _ => target.prepare(target_index, None)
                };
                prepare_result?;
                target_index += 1;
            }
        }

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
        };

        if !self.web_ui_enabled {
            self.web_auth = None;
        }

        if let Some(web_auth) = &mut self.web_auth {
            if web_auth.enabled {
                web_auth.prepare(&self.t_config_path, resolve_var)?;
            } else {
                self.web_auth = None;
            }
        }

        Ok(())
    }

    fn prepare_api_web_root(&mut self) {
        if !self.api.web_root.is_empty() {
            let wrpb = std::path::PathBuf::from(&self.api.web_root);
            if wrpb.is_relative() {
                let mut wrpb2 = std::path::PathBuf::from(&self.working_dir).join(&self.api.web_root);
                if !wrpb2.exists() {
                    wrpb2 = file_utils::get_exe_path().join(&self.api.web_root);
                }
                if !wrpb2.exists() {
                    let cwd = std::env::current_dir();
                    if let Ok(cwd_path) = cwd {
                        wrpb2 = cwd_path.join(&self.api.web_root);
                    }
                }
                if wrpb2.exists() {
                    match wrpb2.absolutize() {
                        Ok(os) => self.api.web_root = String::from(os.to_str().unwrap()),
                        Err(e) => {
                            error!("failed to absolutize web_root {:?}", e);
                        }
                    }
                    // } else {
                    //     error!("web_root directory does not exists {:?}", wrpb2)
                }
            }
        }
    }
}

/// Returns the targets that were specified as parameters.
/// If invalid targets are found, the program will be terminated.
/// The return value has `enabled` set to true, if selective targets should be processed, otherwise false.
///
/// * `target_args` the program parameters given with `-target` parameter.
/// * `sources` configured sources in config file
///
pub(crate) fn validate_targets(target_args: &Option<Vec<String>>, sources: &Vec<ConfigSource>) -> Result<ProcessTargets, M3uFilterError> {
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
        let processing_targets: Vec<String> = check_targets.iter().filter(|&(_, v)| *v != 0).map(|(k, _)| k.to_string()).collect();
        debug!("Processing targets {}", processing_targets.join(", "));
    } else {
        enabled = false;
    }

    Ok(ProcessTargets {
        enabled,
        inputs,
        targets,
    })
}
