#![allow(clippy::struct_excessive_bools)]
use enum_iterator::Sequence;
use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::fs::File;
use std::io::BufRead;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, RwLock};

use crate::auth::user::UserCredential;
use log::{debug, error, info, warn};
use path_clean::PathClean;
use url::Url;

use crate::filter::{get_filter, prepare_templates, Filter, MockValueProcessor, PatternTemplate, ValueProvider};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::messaging::MsgKind;
use crate::model::api_proxy::{ApiProxyConfig, ApiProxyServerInfo, ProxyUserCredentials};
use crate::model::mapping::Mapping;
use crate::model::mapping::Mappings;
use crate::utils::default_utils::{default_as_default, default_as_true, default_as_two_u16};
use crate::utils::file_lock_manager::FileLockManager;
use crate::utils::{config_reader, file_utils};
use crate::{exit, info_err};
use crate::utils::file_utils::file_reader;
use crate::utils::size_utils::parse_size_base_2;

pub const MAPPER_ATTRIBUTE_FIELDS: &[&str] = &[
    "name", "title", "group", "id", "chno", "logo",
    "logo_small", "parent_code", "audio_track",
    "time_shift", "rec", "url", "epg_channel_id", "epg_id"
];


pub const AFFIX_FIELDS: &[&str] = &["name", "title", "group"];
pub const COUNTER_FIELDS: &[&str] = &["name", "title", "chno"];

const STREAM_QUEUE_SIZE: usize = 1024; // mpsc channel holding messages. with 8092byte chunks and 2Mbit/s approx 8MB

#[macro_export]
macro_rules! valid_property {
  ($key:expr, $array:expr) => {{
        $array.contains(&$key)
    }};
}

#[macro_export]
macro_rules! create_m3u_filter_error {
     ($kind: expr, $($arg:tt)*) => {
        M3uFilterError::new($kind, format!($($arg)*))
    }
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
pub enum TargetType {
    #[serde(rename = "m3u")]
    M3u,
    #[serde(rename = "xtream")]
    Xtream,
    #[serde(rename = "strm")]
    Strm,
}

impl TargetType {
    const M3U: &'static str = "M3u";
    const XTREAM: &'static str = "Xtream";
    const STRM: &'static str = "Strm";
}

impl Display for TargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", match *self {
            Self::M3u => Self::M3U,
            Self::Xtream => Self::XTREAM,
            Self::Strm => Self::STRM,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence, PartialEq, Eq, Default)]
pub enum ProcessingOrder {
    #[serde(rename = "frm")]
    #[default]
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

impl ProcessingOrder {
    const FRM: &'static str = "filter, rename, map";
    const FMR: &'static str = "filter, map, rename";
    const RFM: &'static str = "rename, filter, map";
    const RMF: &'static str = "rename, map, filter";
    const MFR: &'static str = "map, filter, rename";
    const MRF: &'static str = "map, rename, filter";
}

impl Display for ProcessingOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", match *self {
            Self::Frm => Self::FRM,
            Self::Fmr => Self::FMR,
            Self::Rfm => Self::RFM,
            Self::Rmf => Self::RMF,
            Self::Mfr => Self::MFR,
            Self::Mrf => Self::MRF,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence)]
pub enum ItemField {
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "name")]
    Name,
    #[serde(rename = "title")]
    Title,
    #[serde(rename = "url")]
    Url,
    #[serde(rename = "type")]
    Type,
}

impl ItemField {
    const GROUP: &'static str = "Group";
    const NAME: &'static str = "Name";
    const TITLE: &'static str = "Title";
    const URL: &'static str = "Url";
    const TYPE: &'static str = "Type";
}

impl Display for ItemField {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", match *self {
            Self::Group => Self::GROUP,
            Self::Name => Self::NAME,
            Self::Title => Self::TITLE,
            Self::Url => Self::URL,
            Self::Type => Self::TYPE,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SortOrder {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}

#[derive(Clone, Debug)]
pub struct ProcessTargets {
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
#[serde(deny_unknown_fields)]
pub struct ConfigSortGroup {
    pub order: SortOrder,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigSortChannel {
    pub field: ItemField,
    // channel field
    pub group_pattern: String,
    // match against group title
    pub order: SortOrder,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<Vec<String>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub re: Option<regex::Regex>,
}

impl ConfigSortChannel {
    pub fn prepare(&mut self) -> Result<(), M3uFilterError> {
        let re = regex::Regex::new(&self.group_pattern);
        if re.is_err() {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse regex: {}", &self.group_pattern);
        }
        self.re = Some(re.unwrap());
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigSort {
    #[serde(default)]
    pub match_as_ascii: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<ConfigSortGroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<Vec<ConfigSortChannel>>,
}

impl ConfigSort {
    pub fn prepare(&mut self) -> Result<(), M3uFilterError> {
        if let Some(channels) = self.channels.as_mut() {
            handle_m3u_filter_error_result_list!(M3uFilterErrorKind::Info, channels.iter_mut().map(ConfigSortChannel::prepare));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigRename {
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigTargetOptions {
    #[serde(default)]
    pub ignore_logo: bool,
    #[serde(default)]
    pub underscore_whitespace: bool,
    #[serde(default)]
    pub cleanup: bool,
    #[serde(default)]
    pub kodi_style: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strm_props: Option<Vec<String>>,
    #[serde(default = "default_as_true")]
    pub xtream_skip_live_direct_source: bool,
    #[serde(default = "default_as_true")]
    pub xtream_skip_video_direct_source: bool,
    #[serde(default = "default_as_true")]
    pub xtream_skip_series_direct_source: bool,
    #[serde(default)]
    pub xtream_resolve_series: bool,
    #[serde(default = "default_as_two_u16")]
    pub xtream_resolve_series_delay: u16,
    #[serde(default)]
    pub xtream_resolve_vod: bool,
    #[serde(default = "default_as_two_u16")]
    pub xtream_resolve_vod_delay: u16,
    #[serde(default)]
    pub m3u_include_type_in_url: bool,
    #[serde(default)]
    pub m3u_mask_redirect_url: bool,
    #[serde(default)]
    pub share_live_streams: bool,
    #[serde(default)]
    pub remove_duplicates: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetOutput {
    #[serde(alias = "type")]
    pub target: TargetType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigTarget {
    #[serde(skip)]
    pub id: u16,
    #[serde(default = "default_as_true")]
    pub enabled: bool,
    #[serde(default = "default_as_default")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<ConfigTargetOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<ConfigSort>,
    pub filter: String,
    #[serde(default)]
    pub output: Vec<TargetOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rename: Option<Vec<ConfigRename>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping: Option<Vec<String>>,
    #[serde(default)]
    pub processing_order: ProcessingOrder,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch: Option<Vec<String>>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub t_watch_re: Option<Vec<regex::Regex>>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub t_filter: Option<Filter>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub t_mapping: Option<Vec<Mapping>>,
}


impl ConfigTarget {
    pub fn prepare(&mut self, id: u16, templates: Option<&Vec<PatternTemplate>>) -> Result<(), M3uFilterError> {
        self.id = id;
        if self.output.is_empty() {
            return Err(info_err!(format!("Missing output format for {}", self.name)));
        }
        let mut m3u_cnt = 0;
        let mut strm_cnt = 0;
        let mut xtream_cnt = 0;
        let mut strm_needs_xtream = false;
        for format in &self.output {
            let has_username = if let Some(username) = &format.username { !username.trim().is_empty() } else { false };
            let has_filename = if let Some(fname) = &format.filename { !fname.trim().is_empty() } else { false };

            match format.target {
                TargetType::M3u => {
                    m3u_cnt += 1;
                    if has_username {
                        warn!("Username for target output m3u is ignored: {}", self.name);
                    }
                }
                TargetType::Strm => {
                    strm_cnt += 1;
                    if !has_filename {
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "filename is required for strm type: {}", self.name);
                    }
                    if has_username {
                        strm_needs_xtream = true;
                    }
                }
                TargetType::Xtream => {
                    xtream_cnt += 1;
                    if default_as_default().eq_ignore_ascii_case(&self.name) {
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "unique target name is required for xtream type: {}", self.name);
                    }
                    if has_username {
                        warn!("Username for target output xtream is ignored: {}", self.name);
                    }
                    if has_filename {
                        warn!("Filename for target output xtream is ignored: {}", self.name);
                    }
                }
            }
        }

        if m3u_cnt > 1 || strm_cnt > 1 || xtream_cnt > 1 {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Multiple output formats with same type : {}", self.name);
        }

        if strm_cnt > 0 && strm_needs_xtream && xtream_cnt == 0 {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "strm output with a username is only permitted when used in combination with xtream output: {}", self.name);
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
                // debug!("Filter: {}", fltr);
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

    pub fn filter(&self, provider: &ValueProvider) -> bool {
        let mut processor = MockValueProcessor {};
        self.t_filter.as_ref().unwrap().filter(provider, &mut processor)
    }

    pub fn get_m3u_filename(&self) -> Option<&String> {
        for format in &self.output {
            if format.target == TargetType::M3u {
                return format.filename.as_ref();
            }
        }
        None
    }

    pub fn has_output(&self, tt: &TargetType) -> bool {
        for format in &self.output {
            if tt.eq(&format.target) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigSource {
    pub inputs: Vec<ConfigInput>,
    pub targets: Vec<ConfigTarget>,
}

impl ConfigSource {
    #[allow(clippy::cast_possible_truncation)]
    pub fn prepare(&mut self, index: u16) -> Result<u16, M3uFilterError> {
        handle_m3u_filter_error_result_list!(M3uFilterErrorKind::Info, self.inputs.iter_mut().enumerate().map(|(idx, i)| i.prepare(index+(idx as u16))));
        Ok(index + (self.inputs.len() as u16))
    }

    pub fn get_inputs_for_target(&self, target_name: &str) -> Option<Vec<&ConfigInput>> {
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
#[serde(deny_unknown_fields)]
pub struct InputAffix {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence, PartialEq, Eq, Default)]
pub enum InputType {
    #[serde(rename = "m3u")]
    #[default]
    M3u,
    #[serde(rename = "xtream")]
    Xtream,
}

impl InputType {
    const M3U: &'static str = "m3u";
    const XTREAM: &'static str = "xtream";
}

impl Display for InputType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::M3u => Self::M3U,
            Self::Xtream => Self::XTREAM,
        })
    }
}

impl FromStr for InputType {
    type Err = M3uFilterError;

    fn from_str(s: &str) -> Result<Self, M3uFilterError> {
        if s.eq("m3u") {
            Ok(Self::M3u)
        } else if s.eq("xtream") {
            Ok(Self::Xtream)
        } else {
            create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unknown InputType: {}", s)
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigInputOptions {
    #[serde(default)]
    pub xtream_skip_live: bool,
    #[serde(default)]
    pub xtream_skip_vod: bool,
    #[serde(default)]
    pub xtream_skip_series: bool,
}

pub struct InputUserInfo {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigInput {
    #[serde(skip)]
    pub id: u16,
    #[serde(default, rename = "type")]
    pub input_type: InputType,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epg_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<InputAffix>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suffix: Option<InputAffix>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "default_as_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<ConfigInputOptions>,

}

impl ConfigInput {
    pub fn prepare(&mut self, id: u16) -> Result<(), M3uFilterError> {
        self.id = id;
        if self.url.trim().is_empty() {
            return Err(info_err!("url for input is mandatory".to_string()));
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
                if self.username.is_some() || self.password.is_some() {
                    debug!("for input type m3u: username and password are ignored");
                }
            }
            InputType::Xtream => {
                if self.username.is_none() || self.password.is_none() {
                    return Err(info_err!("for input type xtream: username and password are mandatory".to_string()));
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

    pub fn get_user_info(&self) -> Option<InputUserInfo> {
        if self.input_type == InputType::Xtream {
            if self.username.is_some() || self.password.is_some() {
                return Some(InputUserInfo {
                    base_url: self.url.clone(),
                    username: self.username.as_ref().unwrap().to_owned(),
                    password: self.password.as_ref().unwrap().to_owned(),
                });
            }
        } else if let Ok(url) = Url::parse(&self.url) {
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelegramMessagingConfig {
    pub bot_token: String,
    pub chat_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestMessagingConfig {
    pub url: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct MessagingConfig {
    #[serde(default)]
    pub notify_on: Vec<MsgKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telegram: Option<TelegramMessagingConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest: Option<RestMessagingConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct VideoDownloadConfig {
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    #[serde(default)]
    pub organize_into_directories: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_pattern: Option<String>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub t_re_episode_pattern: Option<regex::Regex>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub t_re_filename: Option<regex::Regex>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub t_re_remove_filename_ending: Option<regex::Regex>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct VideoConfig {
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<VideoDownloadConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
                    downl.headers.borrow_mut().insert("User-Agent".to_string(), "Mozilla/5.0 (AppleTV; U; CPU OS 14_2 like Mac OS X; en-us) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.0.1 Safari/605.1.15
".to_string());
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
    pub video: Option<VideoConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedules: Option<Vec<ScheduleConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messaging: Option<MessagingConfig>,
    #[serde(default = "default_as_true")]
    pub log_sanitize_sensitive_info: bool,
    #[serde(default)]
    pub update_on_boot: bool,
    #[serde(default = "default_as_true")]
    pub web_ui_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_auth: Option<WebAuthConfig>,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebAuthConfig {
    #[serde(default = "default_as_true")]
    pub enabled: bool,
    pub issuer: String,
    pub secret: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
        let userfile_name = self.userfile.as_ref().map_or_else(|| file_utils::get_default_user_file_path(config_path), std::borrow::ToOwned::to_owned);
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
            let reader = file_reader(file);
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
    fn prepare(&mut self,  working_dir: &str, resolve_var: bool) {
        if self.enabled {
            let work_path = PathBuf::from(working_dir);
            if self.dir.is_none() {
                self.dir = Some(work_path.join("cache").to_string_lossy().to_string());
            } else {
                let mut cache_dir = if resolve_var { config_reader::resolve_env_var(self.dir.as_ref().unwrap()) } else { self.dir.as_ref().unwrap().to_string() };
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
}

impl StreamConfig {
    fn prepare(&mut self) {
        if let Some(buffer) = self.buffer.as_mut() {
            buffer.prepare();
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ReverseProxyConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<StreamConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheConfig>,
}

impl ReverseProxyConfig {
    fn prepare(&mut self, working_dir: &str, resolve_var: bool) {
        if let Some(stream) = self.stream.as_mut() {
            stream.prepare();
        }
        if let Some(cache) = self.cache.as_mut() {
            cache.prepare(working_dir, resolve_var);
        }
    }
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
    pub templates: Option<Vec<PatternTemplate>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<VideoConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedules: Option<Vec<ScheduleConfig>>,
    #[serde(default = "default_as_true")]
    pub log_sanitize_sensitive_info: bool,
    #[serde(default)]
    pub update_on_boot: bool,
    #[serde(default = "default_as_true")]
    pub web_ui_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_auth: Option<WebAuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messaging: Option<MessagingConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverse_proxy: Option<ReverseProxyConfig>,
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
}

impl Config {
    pub fn set_api_proxy(&mut self, api_proxy: Option<ApiProxyConfig>) {
        self.t_api_proxy = Arc::new(RwLock::new(api_proxy));
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

    pub fn get_target_for_user(&self, username: &str, password: &str) -> Option<(ProxyUserCredentials, &ConfigTarget)> {
        self.t_api_proxy.read().unwrap().as_ref().and_then(|api_proxy| self.intern_get_target_for_user(api_proxy.get_target_name(username, password)))
    }

    pub fn get_target_for_user_by_token(&self, token: &str) -> Option<(ProxyUserCredentials, &ConfigTarget)> {
        self.t_api_proxy.read().unwrap().as_ref().and_then(|api_proxy| self.intern_get_target_for_user(api_proxy.get_target_name_by_token(token)))
    }

    pub fn get_user_credentials(&self, username: &str) -> Option<ProxyUserCredentials> {
        self.t_api_proxy.read().unwrap().as_ref().and_then(|api_proxy| api_proxy.get_user_credentials(username))
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

    pub fn prepare(&mut self, resolve_var: bool) -> Result<(), M3uFilterError> {
        let work_dir = if resolve_var { &config_reader::resolve_env_var(&self.working_dir) } else { &self.working_dir };
        self.working_dir = file_utils::get_working_path(work_dir);
        if self.backup_dir.is_none() {
            self.backup_dir = Some(PathBuf::from(&self.working_dir).join("backup").clean().to_string_lossy().to_string());
        } else {
            let backup_dir = if resolve_var { &config_reader::resolve_env_var(self.backup_dir.as_ref().unwrap()) } else { self.backup_dir.as_ref().unwrap() };
            self.backup_dir = Some(backup_dir.to_string());
        }
        if let Some(reverse_proxy) = self.reverse_proxy.as_mut() {
            reverse_proxy.prepare(&self.working_dir, resolve_var);
        }
        self.api.prepare();
        self.prepare_api_web_root(resolve_var);
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
                // prepare templates
                let prepare_result = match &self.templates {
                    Some(templ) => target.prepare(target_index, Some(templ)),
                    _ => target.prepare(target_index, None)
                };
                prepare_result?;
                target_index += 1;
            }

            if let Some(schedules) = &self.schedules {
                for schedule in schedules {
                    if let Some(targets) = &schedule.targets {
                        for target_name in targets {
                            if !target_names_check.contains(target_name) {
                                return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unknown target name in scheduler: {}", target_name);
                            }
                        }
                    }
                }
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

    fn prepare_api_web_root(&mut self, resolve_var: bool) {
        if !self.api.web_root.is_empty() {
            let web_root = if resolve_var { config_reader::resolve_env_var(&self.api.web_root) } else { self.api.web_root.clone() };
            self.api.web_root = web_root.to_string();
            let wrpb = std::path::PathBuf::from(&self.api.web_root);
            if wrpb.is_relative() {
                let mut wrpb2 = std::path::PathBuf::from(&self.working_dir).join(&web_root);
                if !wrpb2.exists() {
                    wrpb2 = file_utils::get_exe_path().join(&web_root);
                }
                if !wrpb2.exists() {
                    let cwd = std::env::current_dir();
                    if let Ok(cwd_path) = cwd {
                        wrpb2 = cwd_path.join(&web_root);
                    }
                }
                if wrpb2.exists() {
                    self.api.web_root = String::from(wrpb2.clean().to_str().unwrap_or_default());
                }
            }
        }
    }

    pub fn get_user_server_info(&self, user: &ProxyUserCredentials) -> ApiProxyServerInfo {
        let server_info_list = self.t_api_proxy.read().unwrap().as_ref().unwrap().server.clone();
        let server_info_name = user.server.as_ref().map_or("default", |server_name| server_name.as_str());
        server_info_list.iter().find(|c| c.name.eq(server_info_name)).map_or_else(|| server_info_list.first().unwrap().clone(), std::clone::Clone::clone)
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
        let processing_targets: Vec<String> = check_targets.iter().filter(|&(_, v)| *v != 0).map(|(k, _)| k.to_string()).collect();
        info!("Processing targets {}", processing_targets.join(", "));
    } else {
        enabled = false;
    }

    Ok(ProcessTargets {
        enabled,
        inputs,
        targets,
    })
}


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthcheckConfig {
    pub api: ConfigApi,
}