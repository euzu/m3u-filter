#![allow(clippy::struct_excessive_bools)]
use enum_iterator::Sequence;
use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::fs::File;
use std::io::BufRead;
use std::path::PathBuf;
use std::str::FromStr;
use parking_lot::RwLock;
use std::sync::Arc;

use crate::auth::user::UserCredential;
use log::{debug, error, warn};
use path_clean::PathClean;
use url::Url;

use crate::foundation::filter::{get_filter, prepare_templates, Filter, MockValueProcessor, PatternTemplate, ValueProvider};
use crate::m3u_filter_error::info_err;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::messaging::MsgKind;
use crate::model::api_proxy::{ApiProxyConfig, ApiProxyServerInfo, ProxyUserCredentials};
use crate::model::mapping::Mapping;
use crate::model::mapping::Mappings;
use crate::utils::file::config_reader;
use crate::utils::default_utils::{default_as_default, default_as_true, default_as_two_u16};
use crate::utils::file::file_lock_manager::FileLockManager;
use crate::utils::file::file_utils;
use crate::utils::file::file_utils::file_reader;
use crate::utils::size_utils::parse_size_base_2;
use crate::utils::sys_utils::exit;

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (AppleTV; U; CPU OS 14_2 like Mac OS X; en-us) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.0.1 Safari/605.1.15";

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
pub use valid_property;
use crate::m3u_filter_error::{create_m3u_filter_error_result, handle_m3u_filter_error_result, handle_m3u_filter_error_result_list};
use crate::utils::string_utils::get_trimmed_string;

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
    #[serde(rename = "input")]
    Input,
    #[serde(rename = "type")]
    Type,
}

impl ItemField {
    const GROUP: &'static str = "Group";
    const NAME: &'static str = "Name";
    const TITLE: &'static str = "Title";
    const URL: &'static str = "Url";
    const INPUT: &'static str = "Input";
    const TYPE: &'static str = "Type";
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
        match regex::Regex::new(&self.group_pattern) {
            Ok(pattern) => {
                self.re = Some(pattern);
                Ok(())
            }
            Err(err) => create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse regex: {} {err}", &self.group_pattern),
        }
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
        match regex::Regex::new(&self.pattern) {
            Ok(pattern) => {
                self.re = Some(pattern);
                Ok(())
            }
            Err(err) => create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse regex: {} {err}", &self.pattern),
        }
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
        if let Some(filter) = self.t_filter.as_ref() {
            return filter.filter(provider, &mut processor);
        }
        true
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
        if s.eq(Self::M3U) {
            Ok(Self::M3u)
        } else if s.eq(Self::XTREAM) {
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
    #[serde(default = "default_as_true")]
    pub xtream_live_stream_use_prefix: bool,
    #[serde(default)]
    pub xtream_live_stream_without_extension: bool,
}

pub struct InputUserInfo {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

macro_rules! check_input_credentials {
    ($this:ident, $input_type:expr) => {
     match $input_type {
            InputType::M3u => {
                if $this.username.is_some() || $this.password.is_some() {
                    debug!("for input type m3u: username and password are ignored");
                }
            }
            InputType::Xtream => {
                if $this.username.is_none() || $this.password.is_none() {
                    return Err(info_err!("for input type xtream: username and password are mandatory".to_string()));
                }
            }
        }
    };
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigInputAlias {
    #[serde(skip)]
    pub id: u16,
    pub url: String,
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default)]
    pub priority: i16,
    #[serde(default)]
    pub max_connections: u16,
}


impl ConfigInputAlias {
    pub fn prepare(&mut self, index: u16, input_type: &InputType) -> Result<(), M3uFilterError> {
        self.id = index;
        self.url = self.url.trim().to_string();
        if self.url.is_empty() {
            return Err(info_err!("url for input is mandatory".to_string()));
        }
        self.username = get_trimmed_string(&self.username);
        self.password = get_trimmed_string(&self.password);
        check_input_credentials!(self, input_type);
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigInput {
    #[serde(skip)]
    pub id: u16,
    pub name: String,
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
    #[serde(default = "default_as_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<ConfigInputOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<ConfigInputAlias>>,
    #[serde(default)]
    pub priority: i16,
    #[serde(default)]
    pub max_connections: u16,
}

impl ConfigInput {
    #[allow(clippy::cast_possible_truncation)]
    pub fn prepare(&mut self, index: u16) -> Result<u16, M3uFilterError> {
        self.id = index;
        self.name = self.name.trim().to_string();
        if self.name.is_empty() {
            return Err(info_err!("name for input is mandatory".to_string()));
        }
        self.url = self.url.trim().to_string();
        if self.url.is_empty() {
            return Err(info_err!("url for input is mandatory".to_string()));
        }
        self.username = get_trimmed_string(&self.username);
        self.password = get_trimmed_string(&self.password);
        check_input_credentials!(self, self.input_type);
        self.persist = get_trimmed_string(&self.persist);
        if let Some(aliases) = self.aliases.as_mut() {
            let input_type = &self.input_type;
            handle_m3u_filter_error_result_list!(M3uFilterErrorKind::Info, aliases.iter_mut().enumerate().map(|(idx, i)| i.prepare(index+(idx as u16), input_type)));
        }
        Ok(index + self.aliases.as_ref().map_or(0, std::vec::Vec::len) as u16)
    }

    pub fn get_user_info(&self) -> Option<InputUserInfo> {
        if self.input_type == InputType::Xtream {
            if let (Some(username), Some(password)) = (self.username.as_ref(), self.password.as_ref()) {
                return Some(InputUserInfo {
                    base_url: self.url.clone(),
                    username: username.to_owned(),
                    password: password.to_owned(),
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
                if let (Some(username), Some(password)) = (username.as_ref(), password.as_ref()) {
                    return Some(InputUserInfo {
                        base_url,
                        username: username.to_owned(),
                        password: password.to_owned(),
                    });
                }
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushoverMessagingConfig {
    pub(crate) url: Option<String>,
    pub(crate) token: String,
    pub(crate) user: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pushover: Option<PushoverMessagingConfig>,

}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LogConfig {
    #[serde(default = "default_as_true")]
    pub sanitize_sensitive_info: bool,
    #[serde(default)]
    pub active_clients: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct LogLevelConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<LogConfig>,
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

    /// # Panics
    ///
    /// Will panic if default `RegEx` gets invalid
    pub fn prepare(&mut self) -> Result<(), M3uFilterError> {
        self.extensions = ["mkv", "avi", "mp4", "mpeg", "divx", "mov"].iter().map(|&arg| arg.to_string()).collect();
        match &mut self.download {
            None => {}
            Some(downl) => {
                if downl.headers.is_empty() {
                    downl.headers.borrow_mut().insert("Accept".to_string(), "video/*".to_string());
                    downl.headers.borrow_mut().insert("User-Agent".to_string(), DEFAULT_USER_AGENT.to_string());
                }

                if let Some(episode_pattern) = &downl.episode_pattern {
                    if !episode_pattern.is_empty() {
                        match regex::Regex::new(episode_pattern) {
                            Ok(pattern) => {
                                downl.t_re_episode_pattern = Some(pattern);
                            }
                            Err(err) => {
                                return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse regex: {episode_pattern} {err}");
                            }
                        }
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
    fn prepare(&mut self, working_dir: &str, resolve_var: bool) {
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
    #[serde(default)]
    pub resource_rewrite_disabled: bool,
}

impl ReverseProxyConfig {
    fn prepare(&mut self, working_dir: &str, resolve_var: bool) {
        if let Some(stream) = self.stream.as_mut() {
            stream.prepare();
        }
        if let Some(cache) = self.cache.as_mut() {
            if cache.enabled && self.resource_rewrite_disabled {
                warn!("The cache is disabled because resource rewrite is disabled");
                cache.enabled = false;
            }
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
    pub user_config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_unavailable_file: Option<String>,
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
    #[serde(skip)]
    pub t_channel_unavailable_file: Option<Arc<Vec<u8>>>,
}

impl Config {
    pub fn set_api_proxy(&mut self, api_proxy: Option<ApiProxyConfig>) {
        self.t_api_proxy = Arc::new(RwLock::new(api_proxy));
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

    pub fn get_target_for_username(&self, username: &str) -> Option<(ProxyUserCredentials, &ConfigTarget)> {
        if let Some(credentials) =  self.get_user_credentials(username) {
            return self.t_api_proxy.read().as_ref().and_then(|api_proxy| self.intern_get_target_for_user(api_proxy.get_target_name(&credentials.username, &credentials.password)))
        }
        None
    }

    pub fn get_target_for_user(&self, username: &str, password: &str) -> Option<(ProxyUserCredentials, &ConfigTarget)> {
        self.t_api_proxy.read().as_ref().and_then(|api_proxy| self.intern_get_target_for_user(api_proxy.get_target_name(username, password)))
    }

    pub fn get_target_for_user_by_token(&self, token: &str) -> Option<(ProxyUserCredentials, &ConfigTarget)> {
        self.t_api_proxy.read().as_ref().and_then(|api_proxy| self.intern_get_target_for_user(api_proxy.get_target_name_by_token(token)))
    }

    pub fn get_user_credentials(&self, username: &str) -> Option<ProxyUserCredentials> {
        self.t_api_proxy.read().as_ref().and_then(|api_proxy| api_proxy.get_user_credentials(username))
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

    pub fn prepare(&mut self, resolve_var: bool) -> Result<(), M3uFilterError> {
        let work_dir = if resolve_var { &config_reader::resolve_env_var(&self.working_dir) } else { &self.working_dir };
        self.working_dir = file_utils::get_working_path(work_dir);

        if let Some(channel_unavailable_file) = &self.channel_unavailable_file {
            let channel_unavailable = file_utils::make_absolute_path(channel_unavailable_file, &self.working_dir, resolve_var);
            match file_utils::read_file_as_bytes(&PathBuf::from(&channel_unavailable)) {
                Ok(data) => {
                    self.t_channel_unavailable_file = Some(Arc::new(data));
                },
                Err(err) => {
                    error!("Failed to load channel unavailable file: {channel_unavailable} {err}");
                }
            }
            self.channel_unavailable_file = Some(channel_unavailable);
        }

        if self.backup_dir.is_none() {
            self.backup_dir = Some(PathBuf::from(&self.working_dir).join("backup").clean().to_string_lossy().to_string());
        } else {
            self.backup_dir = self.backup_dir.as_ref().map(|backup_dir| {
                if resolve_var {
                    config_reader::resolve_env_var(backup_dir)
                } else {
                    backup_dir.to_owned()
                }
            }).map(|dir| dir.to_string());
        }
        if self.user_config_dir.is_none() {
            self.user_config_dir = Some(PathBuf::from(&self.working_dir).join("user_config").clean().to_string_lossy().to_string());
        } else {
            self.user_config_dir = self.user_config_dir.as_ref().map(|user_config_dir| {
                if resolve_var {
                    config_reader::resolve_env_var(user_config_dir)
                } else {
                    user_config_dir.to_owned()
                }
            }).map(|dir| dir.to_string());
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
        let mut source_index: u16 = 1;
        let mut target_index: u16 = 1;
        self.check_unique_input_names()?;
        let target_names = self.check_unique_target_names()?;
        self.check_scheduled_targets(&target_names)?;
        for source in &mut self.sources {
            source_index = source.prepare(source_index)?;
            for target in &mut source.targets {
                // prepare templates
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

    fn prepare_api_web_root(&mut self, resolve_var: bool) {
        if !self.api.web_root.is_empty() {
            self.api.web_root = file_utils::make_absolute_path(&self.api.web_root, &self.working_dir, resolve_var);
        }
    }


    /// # Panics
    ///
    /// Will panic if default server invalid
    pub fn get_user_server_info(&self, user: &ProxyUserCredentials) -> ApiProxyServerInfo {
        let server_info_list = self.t_api_proxy.read().as_ref().unwrap().server.clone();
        let server_info_name = user.server.as_ref().map_or("default", |server_name| server_name.as_str());
        server_info_list.iter().find(|c| c.name.eq(server_info_name)).map_or_else(|| server_info_list.first().unwrap().clone(), Clone::clone)
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


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthcheckConfig {
    pub api: ConfigApi,
}