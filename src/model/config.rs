#![allow(clippy::struct_excessive_bools)]
use bitflags::bitflags;
use enum_iterator::Sequence;
use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::fs::File;
use std::io::BufRead;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::fmt;
use tokio::sync::RwLock;

use crate::auth::user::UserCredential;
use log::{debug, error, warn};
use path_clean::PathClean;
use rand::Rng;
use regex::Regex;
use serde::de::{self, Error, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use url::Url;

use crate::foundation::filter::{get_filter, prepare_templates, Filter, MockValueProcessor, PatternTemplate, ValueProvider};
use crate::m3u_filter_error::info_err;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::messaging::MsgKind;
use crate::model::api_proxy::{ApiProxyConfig, ApiProxyServerInfo, ProxyUserCredentials};
use crate::model::mapping::Mapping;
use crate::model::mapping::Mappings;
use crate::utils::default_utils::{default_as_default, default_as_true, default_connect_timeout_secs, default_grace_period_millis, default_grace_period_timeout_secs, default_resolve_delay_secs};
use crate::utils::file::file_lock_manager::FileLockManager;
use crate::utils::file::file_utils;
use crate::utils::file::file_utils::file_reader;
use crate::utils::size_utils::{parse_size_base_2, parse_to_kbps};
use crate::utils::sys_utils::exit;

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (AppleTV; U; CPU OS 14_2 like Mac OS X; en-us) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.0.1 Safari/605.1.15";

pub const MAPPER_ATTRIBUTE_FIELDS: &[&str] = &[
    "name", "title", "caption", "group", "id", "chno", "logo",
    "logo_small", "parent_code", "audio_track",
    "time_shift", "rec", "url", "epg_channel_id", "epg_id"
];

pub const AFFIX_FIELDS: &[&str] = &["name", "title", "caption", "group"];
pub const COUNTER_FIELDS: &[&str] = &["name", "title", "caption", "chno"];

const STREAM_QUEUE_SIZE: usize = 1024; // mpsc channel holding messages. with 8192byte chunks and 2Mbit/s approx 8MB

const RESERVED_PATHS: &[&str] = &[
    "live", "movie", "series", "m3u-stream", "healthcheck", "status",
    "player_api.php", "panel_api.php", "xtream", "timeshift", "timeshift.php", "streaming",
    "get.php", "apiget", "m3u", "resource"
];

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
use crate::m3u_filter_error::{create_m3u_filter_error, create_m3u_filter_error_result, handle_m3u_filter_error_result, handle_m3u_filter_error_result_list};
use crate::model::hdhomerun_config::HdHomeRunConfig;
use crate::model::playlist::{PlaylistItemType, XtreamCluster};
use crate::utils::constants::CONSTANTS;
use crate::utils::file::config_reader::csv_read_inputs;
use crate::utils::network::request::{get_base_url_from_str, get_credentials_from_url, get_credentials_from_url_str};
use crate::utils::string_utils::get_trimmed_string;

#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize, Sequence, PartialEq, Eq, Hash)]
pub enum TargetType {
    #[serde(rename = "m3u")]
    M3u,
    #[serde(rename = "xtream")]
    Xtream,
    #[serde(rename = "strm")]
    Strm,
    #[serde(rename = "hdhomerun")]
    HdHomeRun,
}

impl TargetType {
    const M3U: &'static str = "M3u";
    const XTREAM: &'static str = "Xtream";
    const STRM: &'static str = "Strm";
    const HDHOMERUN: &'static str = "HdHomeRun";
}

impl Display for TargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", match *self {
            Self::M3u => Self::M3U,
            Self::Xtream => Self::XTREAM,
            Self::Strm => Self::STRM,
            Self::HdHomeRun => Self::HDHOMERUN,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence, PartialEq, Eq, Hash)]
enum HdHomeRunUseTargetType {
    #[serde(rename = "m3u")]
    M3u,
    #[serde(rename = "xtream")]
    Xtream,
}

impl TryFrom<TargetType> for HdHomeRunUseTargetType {
    type Error = &'static str;

    fn try_from(value: TargetType) -> Result<Self, Self::Error> {
        match value {
            TargetType::Xtream => Ok(Self::Xtream),
            TargetType::M3u => Ok(Self::M3u),
            _ => Err("Not allowed!"),
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<Vec<String>>,
    #[serde(default, skip)]
    pub t_sequence: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigSortChannel {
    // channel field
    pub field: ItemField,
    // match against group title
    pub group_pattern: String,
    pub order: SortOrder,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<Vec<String>>,
    #[serde(default, skip)]
    pub t_sequence: Option<Vec<Regex>>,
    #[serde(skip)]
    pub t_re_group_pattern: Option<Regex>,
}

impl ConfigSortChannel {
    pub fn prepare(&mut self) -> Result<(), M3uFilterError> {
        // Compile group_pattern
        self.t_re_group_pattern = Some(
            Regex::new(&self.group_pattern).map_err(|err| {
                create_m3u_filter_error!(M3uFilterErrorKind::Info, "cant parse regex: {} {err}", &self.group_pattern)
            })?
        );

        // Compile sequence patterns, if any
        self.t_sequence = self.sequence.as_ref()
            .map(|seq| {
                seq.iter()
                    .map(|s| Regex::new(s).map_err(|err| {
                        create_m3u_filter_error!(M3uFilterErrorKind::Info, "cant parse regex: {} {err}", s)
                    }))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?; // convert Option<Result<...>> to Result<Option<...>>

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
    pub share_live_streams: bool,
    #[serde(default)]
    pub remove_duplicates: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_redirect: Option<ClusterFlags>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XtreamTargetOutput {
    #[serde(default = "default_as_true")]
    pub skip_live_direct_source: bool,
    #[serde(default = "default_as_true")]
    pub skip_video_direct_source: bool,
    #[serde(default = "default_as_true")]
    pub skip_series_direct_source: bool,
    #[serde(default)]
    pub resolve_series: bool,
    #[serde(default = "default_resolve_delay_secs")]
    pub resolve_series_delay: u16,
    #[serde(default)]
    pub resolve_vod: bool,
    #[serde(default = "default_resolve_delay_secs")]
    pub resolve_vod_delay: u16,
}


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3uTargetOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default)]
    pub include_type_in_url: bool,
    #[serde(default)]
    pub mask_redirect_url: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrmTargetOutput {
    pub directory: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default)]
    pub underscore_whitespace: bool,
    #[serde(default)]
    pub cleanup: bool,
    #[serde(default)]
    pub kodi_style: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strm_props: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HdHomeRunTargetOutput {
    pub device: String,
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_output: Option<TargetType>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all = "lowercase")]
pub enum TargetOutput {
    Xtream(XtreamTargetOutput),
    M3u(M3uTargetOutput),
    Strm(StrmTargetOutput),
    HdHomeRun(HdHomeRunTargetOutput),
}

bitflags! {
    #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct ClusterFlags: u16 {
        const Live   = 1;      // 0b0000_0001
        const Vod    = 1 << 1; // 0b0000_0010
        const Series = 1 << 2; // 0b0000_0100
    }
}

impl ClusterFlags {
    pub fn has_cluster(&self, item_type: PlaylistItemType) -> bool {
        XtreamCluster::try_from(item_type).ok().is_some_and(|cluster| match cluster {
            XtreamCluster::Live => self.contains(ClusterFlags::Live),
            XtreamCluster::Video => self.contains(ClusterFlags::Vod),
            XtreamCluster::Series => self.contains(ClusterFlags::Series),
        })
    }

    pub fn has_full_flags(&self) -> bool {
        self.is_all()
    }

    fn from_items<I, S>(items: I) -> Result<Self, &'static str>
    where
        I: IntoIterator<Item=S>,
        S: AsRef<str>,
    {
        let mut result = ClusterFlags::empty();

        for item in items {
            match item.as_ref().trim() {
                "live" => result.set(ClusterFlags::Live, true),
                "vod" => result.set(ClusterFlags::Vod, true),
                "series" => result.set(ClusterFlags::Series, true),
                _ => return Err("Invalid flag {item}, allowed are live, vod, series"),
            }
        }

        Ok(result)
    }
}

impl fmt::Display for ClusterFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut flag_strings = Vec::new();
        if self.contains(ClusterFlags::Live) {
            flag_strings.push("live");
        }
        if self.contains(ClusterFlags::Vod) {
            flag_strings.push("vod");
        }
        if self.contains(ClusterFlags::Series) {
            flag_strings.push("series");
        }

        write!(f, "[{}]", flag_strings.join(","))
    }
}

impl TryFrom<&str> for ClusterFlags {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let input = value.trim().trim_matches(['[', ']'].as_ref());
        let items = input.split(',').map(str::trim);
        ClusterFlags::from_items(items)
    }
}

impl TryFrom<Vec<String>> for ClusterFlags {
    type Error = &'static str;

    fn try_from(value: Vec<String>) -> Result<Self, Self::Error> {
        ClusterFlags::from_items(value)
    }
}

impl Serialize for ClusterFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_some(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ClusterFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ClusterFlagsVisitor;

        impl<'de> Visitor<'de> for ClusterFlagsVisitor {
            type Value = ClusterFlags;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or a map entry like : [vod, live, series]")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                ClusterFlags::try_from(v).map_err(E::custom)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(val) = seq.next_element::<String>()? {
                    let entry = val.trim().to_lowercase();
                    values.push(entry);
                }
                ClusterFlags::try_from(values).map_err(A::Error::custom)
            }
        }
        deserializer.deserialize_any(ClusterFlagsVisitor)
    }
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
        let mut hdhr_cnt = 0;
        let mut hdhomerun_needs_m3u = false;
        let mut hdhomerun_needs_xtream = false;

        for target_output in &mut self.output {
            match target_output {
                TargetOutput::Xtream(_) => {
                    xtream_cnt += 1;
                    if default_as_default().eq_ignore_ascii_case(&self.name) {
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "unique target name is required for xtream type output: {}", self.name);
                    }
                }
                TargetOutput::M3u(m3u_output) => {
                    m3u_cnt += 1;
                    m3u_output.filename = m3u_output.filename.as_ref().map(|s| s.trim().to_string());
                }
                TargetOutput::Strm(strm_output) => {
                    strm_cnt += 1;
                    strm_output.directory = strm_output.directory.trim().to_string();
                    if strm_output.directory.trim().is_empty() {
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "directory is required for strm type: {}", self.name);
                    }
                    let has_username = if let Some(username) = &strm_output.username { !username.trim().is_empty() } else { false };
                    if has_username {
                        strm_needs_xtream = true;
                    }
                }
                TargetOutput::HdHomeRun(hdhomerun_output) => {
                    hdhr_cnt += 1;
                    hdhomerun_output.username = hdhomerun_output.username.trim().to_string();
                    if hdhomerun_output.username.is_empty() {
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Username is required for HdHomeRun type: {}", self.name);
                    }

                    hdhomerun_output.device = hdhomerun_output.device.trim().to_string();
                    if hdhomerun_output.device.is_empty() {
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Device is required for HdHomeRun type: {}", self.name);
                    }

                    if let Some(use_output) = hdhomerun_output.use_output.as_ref() {
                        match &use_output {
                            TargetType::M3u => { hdhomerun_needs_m3u = true; }
                            TargetType::Xtream => { hdhomerun_needs_xtream = true; }
                            _ => return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "HdHomeRun output option `use_output` only accepts `m3u` or `xtream` for target: {}", self.name),
                        }
                    }
                }
            }
        }

        if m3u_cnt > 1 || strm_cnt > 1 || xtream_cnt > 1 || hdhr_cnt > 1 {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Multiple output formats with same type : {}", self.name);
        }

        if strm_cnt > 0 && strm_needs_xtream && xtream_cnt == 0 {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "strm output with a username is only permitted when used in combination with xtream output: {}", self.name);
        }

        if hdhr_cnt > 0 {
            if xtream_cnt == 0 && m3u_cnt == 0 {
                return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "HdHomeRun output is only permitted when used in combination with xtream or m3u output: {}", self.name);
            }
            if hdhomerun_needs_m3u && m3u_cnt == 0 {
                return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "HdHomeRun output has `use_output=m3u` but no `m3u` output defined: {}", self.name);
            }
            if hdhomerun_needs_xtream && xtream_cnt == 0 {
                return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "HdHomeRun output has `use_output=xtream` but no `xtream` output defined: {}", self.name);
            }
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

    pub(crate) fn get_xtream_output(&self) -> Option<&XtreamTargetOutput> {
        if let Some(TargetOutput::Xtream(output)) = self.output.iter().find(|o| matches!(o, TargetOutput::Xtream(_))) {
            Some(output)
        } else {
            None
        }
    }

    pub(crate) fn get_m3u_output(&self) -> Option<&M3uTargetOutput> {
        if let Some(TargetOutput::M3u(output)) = self.output.iter().find(|o| matches!(o, TargetOutput::M3u(_))) {
            Some(output)
        } else {
            None
        }
    }

    // pub(crate) fn get_strm_output(&self) -> Option<&StrmTargetOutput> {
    //     if let Some(TargetOutput::Strm(output)) = self.output.iter().find(|o| matches!(o, TargetOutput::Strm(_))) {
    //         Some(output)
    //     } else {
    //         None
    //     }
    // }

    pub(crate) fn get_hdhomerun_output(&self) -> Option<&HdHomeRunTargetOutput> {
        if let Some(TargetOutput::HdHomeRun(output)) = self.output.iter().find(|o| matches!(o, TargetOutput::HdHomeRun(_))) {
            Some(output)
        } else {
            None
        }
    }

    pub fn has_output(&self, tt: &TargetType) -> bool {
        for target_output in &self.output {
            match target_output {
                TargetOutput::Xtream(_) => { if tt == &TargetType::Xtream { return true; } }
                TargetOutput::M3u(_) => { if tt == &TargetType::M3u { return true; } }
                TargetOutput::Strm(_) => { if tt == &TargetType::Strm { return true; } }
                TargetOutput::HdHomeRun(_) => { if tt == &TargetType::HdHomeRun { return true; } }
            }
        }
        false
    }

    pub fn is_force_redirect(&self, item_type: PlaylistItemType) -> bool {
        self.options
            .as_ref()
            .and_then(|options| options.force_redirect.as_ref())
            .is_some_and(|flags| flags.has_cluster(item_type))
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
    pub fn prepare(&mut self, index: u16, include_computed: bool) -> Result<u16, M3uFilterError> {
        handle_m3u_filter_error_result_list!(M3uFilterErrorKind::Info, self.inputs.iter_mut().enumerate().map(|(idx, i)| i.prepare(index+(idx as u16), include_computed)));
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

#[derive(
    Debug,
    Copy,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    Sequence,
    PartialEq,
    Eq,
    Default
)]
pub enum InputType {
    #[serde(rename = "m3u")]
    #[default]
    M3u,
    #[serde(rename = "xtream")]
    Xtream,
    #[serde(rename = "m3u_batch")]
    M3uBatch,
    #[serde(rename = "xtream_batch")]
    XtreamBatch,
}

impl InputType {
    const M3U: &'static str = "m3u";
    const XTREAM: &'static str = "xtream";
    const M3U_BATCH: &'static str = "m3u_batch";
    const XTREAM_BATCH: &'static str = "xtream_batch";
}

impl Display for InputType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::M3u => Self::M3U,
            Self::Xtream => Self::XTREAM,
            Self::M3uBatch => Self::M3U_BATCH,
            Self::XtreamBatch => Self::XTREAM_BATCH,
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

#[derive(
    Debug,
    Copy,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    Sequence,
    PartialEq,
    Eq,
    Default
)]
pub enum InputFetchMethod {
    #[default]
    GET,
    POST,
}

impl InputFetchMethod {
    const GET_METHOD: &'static str = "GET";
    const POST_METHOD: &'static str = "POST";
}

impl Display for InputFetchMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::GET => Self::GET_METHOD,
            Self::POST => Self::POST_METHOD,
        })
    }
}

impl FromStr for InputFetchMethod {
    type Err = M3uFilterError;

    fn from_str(s: &str) -> Result<Self, M3uFilterError> {
        if s.eq(Self::GET_METHOD) {
            Ok(Self::GET)
        } else if s.eq(Self::POST_METHOD) {
            Ok(Self::POST)
        } else {
            create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unknown Fetch Method: {}", s)
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

impl InputUserInfo {
    pub fn new(input_type: InputType, username: Option<&str>, password: Option<&str>, input_url: &str) -> Option<Self> {
        if input_type == InputType::Xtream {
            if let (Some(username), Some(password)) = (username, password) {
                return Some(Self {
                    base_url: input_url.to_string(),
                    username: username.to_owned(),
                    password: password.to_owned(),
                });
            }
        } else if let Ok(url) = Url::parse(input_url) {
            let base_url = url.origin().ascii_serialization();
            let (username, password) = get_credentials_from_url(&url);
            if username.is_some() || password.is_some() {
                if let (Some(username), Some(password)) = (username.as_ref(), password.as_ref()) {
                    return Some(Self {
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

macro_rules! check_input_credentials {
    ($this:ident, $input_type:expr) => {
     match $input_type {
            InputType::M3u | InputType::M3uBatch => {
                if $this.username.is_some() || $this.password.is_some() {
                    debug!("for input type m3u: username and password are ignored");
                }
                if $this.username.is_none() && $this.password.is_none() {
                    let (username, password) = get_credentials_from_url_str(&$this.url);
                    $this.username = username;
                    $this.password = password;
                }
            }
            InputType::Xtream | InputType::XtreamBatch => {
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
    pub name: String,
    pub url: String,
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default)]
    pub priority: i16,
    #[serde(default)]
    pub max_connections: u16,
    #[serde(skip)]
    pub t_base_url: String,
}


impl ConfigInputAlias {
    pub fn prepare(&mut self, index: u16, input_type: &InputType) -> Result<(), M3uFilterError> {
        self.id = index;
        self.name = self.name.trim().to_string();
        if self.name.is_empty() {
            return Err(info_err!("name for input is mandatory".to_string()));
        }
        self.url = self.url.trim().to_string();
        if self.url.is_empty() {
            return Err(info_err!("url for input is mandatory".to_string()));
        }
        if let Some(base_url) = get_base_url_from_str(&self.url) {
            self.t_base_url = base_url;
        }
        self.username = get_trimmed_string(&self.username);
        self.password = get_trimmed_string(&self.password);
        check_input_credentials!(self, input_type);

        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum EpgUrl {
    Single(String),
    Multi(Vec<String>),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EpgNamePrefix {
    #[default]
    Ignore,
    Suffix(String),
    Prefix(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpgSmartMatchConfig {
    #[serde(default)]
    pub enabled: bool,
    pub normalize_regex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strip: Option<Vec<String>>,
    #[serde(default)]
    pub name_prefix: EpgNamePrefix,
    #[serde(skip)]
    pub name_prefix_separator: Option<Vec<char>>,
    #[serde(default)]
    pub fuzzy_matching: bool,
    #[serde(default)]
    pub match_threshold: u16,
    #[serde(default)]
    pub best_match_threshold: u16,
    #[serde(skip)]
    pub t_strip: Vec<String>,
    #[serde(skip)]
    pub t_normalize_regex: Option<Regex>,
    #[serde(skip)]
    pub t_name_prefix_separator: Vec<char>,

}

impl EpgSmartMatchConfig {
    /// Creates a new enabled `EpgSmartMatchConfig` with default settings and prepares it.
    ///
    /// Returns an error if preparation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// let config = EpgSmartMatchConfig::new().unwrap();
    /// assert!(config.enabled);
    /// ```
    pub fn new() -> Result<Self, M3uFilterError> {
        let mut this = Self { enabled: true, ..Self::default() };
        this.prepare()?;
        Ok(this)
    }

    /// # Panics
    ///
    /// Prepares the EPG smart match configuration by validating thresholds, compiling normalization regex, and setting default values as needed.
    ///
    /// Adjusts match thresholds to valid ranges, compiles the normalization regex, and sets default strip values and name prefix separators if not provided. Returns an error if the normalization regex is invalid.
    ///
    /// # Returns
    ///
    /// `Ok(())` if preparation succeeds, or an `M3uFilterError` if regex compilation fails.
    pub fn prepare(&mut self) -> Result<(), M3uFilterError> {
        if !self.enabled {
            return Ok(());
        }

        self.t_name_prefix_separator = match &self.name_prefix_separator {
            None => vec![':', '|', '-'],
            Some(list) => list.clone(),
        };

        if self.match_threshold < 10 {
            warn!("match_threshold is less than 10%, setting to 10%");
            self.match_threshold = 10;
        } else if self.match_threshold > 100 {
            warn!("match_threshold is more than 100%, setting to 80%");
            self.match_threshold = 100;
        }

        if self.best_match_threshold == 0 || self.best_match_threshold > 100 || self.best_match_threshold < self.match_threshold {
            self.best_match_threshold = 99;
        }

        self.t_normalize_regex = match self.normalize_regex.as_ref() {
            None => Some(CONSTANTS.re_epg_normalize.clone()),
            Some(regstr) => {
                let re = regex::Regex::new(regstr.as_str());
                if re.is_err() {
                    return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse regex: {}", regstr);
                }
                Some(re.unwrap())
            }
        };

        if self.strip.is_none() {
            self.t_strip = ["3840p", "uhd", "fhd", "hd", "sd", "4k", "plus", "raw"].iter().map(std::string::ToString::to_string).collect();
        }
        Ok(())
    }
}

impl Default for EpgSmartMatchConfig {
    fn default() -> Self {
        let mut instance = EpgSmartMatchConfig {
            enabled: false,
            normalize_regex: None,
            strip: None,
            name_prefix: EpgNamePrefix::default(),
            name_prefix_separator: None,
            fuzzy_matching: false,
            match_threshold: 0,
            best_match_threshold: 0,
            t_strip: Vec::default(),
            t_normalize_regex: None,
            t_name_prefix_separator: Vec::default(),
        };
        let _ = instance.prepare();
        instance
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpgConfig {
    #[serde(default)]
    pub auto_epg: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<EpgUrl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smart_match: Option<EpgSmartMatchConfig>,
    #[serde(skip)]
    pub t_urls: Vec<String>,
    #[serde(skip)]
    pub t_smart_match: EpgSmartMatchConfig,
}

impl EpgConfig {
    pub fn prepare(&mut self, include_computed: bool) -> Result<(), M3uFilterError> {
        if include_computed {
            self.t_urls = self.url.take().map_or_else(Vec::new, |epg_url| {
                match epg_url {
                    EpgUrl::Single(url) => if url.trim().is_empty() {
                        vec![]
                    } else {
                        vec![url.trim().to_string()]
                    },
                    EpgUrl::Multi(urls) =>
                        urls.into_iter()
                            .map(|url| url.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                }
            });

            self.t_smart_match = match self.smart_match.as_mut() {
                None => {
                    let mut normalize: EpgSmartMatchConfig = EpgSmartMatchConfig::default();
                    normalize.prepare()?;
                    normalize
                }
                Some(normalize_cfg) => {
                    let mut normalize: EpgSmartMatchConfig = normalize_cfg.clone();
                    normalize.prepare()?;
                    normalize
                }
            };
        }
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
    pub epg: Option<EpgConfig>,
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
    #[serde(default)]
    pub method: InputFetchMethod,
    #[serde(skip)]
    pub t_base_url: String,
}

impl ConfigInput {
    #[allow(clippy::cast_possible_truncation)]
    pub fn prepare(&mut self, index: u16, include_computed: bool) -> Result<u16, M3uFilterError> {
        self.id = index;
        self.check_url()?;
        self.prepare_batch()?;

        self.name = self.name.trim().to_string();
        if self.name.is_empty() {
            return Err(info_err!("name for input is mandatory".to_string()));
        }

        self.username = get_trimmed_string(&self.username);
        self.password = get_trimmed_string(&self.password);
        check_input_credentials!(self, self.input_type);
        self.persist = get_trimmed_string(&self.persist);
        if let Some(base_url) = get_base_url_from_str(&self.url) {
            self.t_base_url = base_url;
        }

        if let Some(epg) = self.epg.as_mut() {
            let _ = epg.prepare(include_computed);
            if include_computed && epg.auto_epg {
                let (username, password) = if self.username.is_none() || self.password.is_none() {
                    get_credentials_from_url_str(&self.url)
                } else {
                    (self.username.clone(), self.password.clone())
                };

                if username.is_none() || password.is_none() {
                    warn!("auto_epg is enabled for input {}, but no credentials could be extracted", self.name);
                } else if !self.t_base_url.is_empty() {
                    let provider_epg_url = format!("{}/xmltv.php?username={}&password={}", self.t_base_url, username.unwrap_or_default(), password.unwrap_or_default());
                    if !epg.t_urls.contains(&provider_epg_url) {
                        debug!("Added provider epg url {provider_epg_url} for input {}", self.name);
                        epg.t_urls.push(provider_epg_url);
                    }
                } else {
                    warn!("auto_epg is enabled for input {}, but url could not be parsed {}", self.name, self.url);
                }
            }
        }

        if let Some(aliases) = self.aliases.as_mut() {
            let input_type = &self.input_type;
            handle_m3u_filter_error_result_list!(M3uFilterErrorKind::Info, aliases.iter_mut().enumerate().map(|(idx, i)| i.prepare(index+1+(idx as u16), input_type)));
        }
        Ok(index + self.aliases.as_ref().map_or(0, std::vec::Vec::len) as u16)
    }

    fn check_url(&mut self) -> Result<(), M3uFilterError> {
        self.url = self.url.trim().to_string();
        if self.url.is_empty() {
            return Err(info_err!("url for input is mandatory".to_string()));
        }
        Ok(())
    }

    fn prepare_batch(&mut self) -> Result<(), M3uFilterError> {
        if self.input_type == InputType::M3uBatch || self.input_type == InputType::XtreamBatch {
            let input_type = if self.input_type == InputType::M3uBatch {
                InputType::M3u
            } else {
                InputType::Xtream
            };

            match csv_read_inputs(self) {
                Ok(mut batch_aliases) => {
                    if !batch_aliases.is_empty() {
                        batch_aliases.reverse();
                        if let Some(mut first) = batch_aliases.pop() {
                            self.username = first.username.take();
                            self.password = first.password.take();
                            self.url = first.url.trim().to_string();
                            self.max_connections = first.max_connections;
                            self.priority = first.priority;
                            if self.name.is_empty() {
                                self.name = first.name.to_string();
                            }
                        }
                        if !batch_aliases.is_empty() {
                            batch_aliases.reverse();
                            if let Some(aliases) = self.aliases.as_mut() {
                                aliases.extend(batch_aliases);
                            } else {
                                self.aliases = Some(batch_aliases);
                            }
                        }
                    }
                }
                Err(err) => {
                    return Err(M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()));
                }
            }
            self.input_type = input_type;
        }
        Ok(())
    }

    pub fn get_user_info(&self) -> Option<InputUserInfo> {
        InputUserInfo::new(self.input_type, self.username.as_deref(), self.password.as_deref(), &self.url)
    }

    pub fn get_matched_config_by_url<'a>(&'a self, url: &str) -> Option<(&'a str, Option<&'a String>, Option<&'a String>)> {
        if url.contains(&self.t_base_url) {
            return Some((&self.t_base_url, self.username.as_ref(), self.password.as_ref()));
        }

        if let Some(aliases) = &self.aliases {
            for alias in aliases {
                if url.contains(&alias.t_base_url) {
                    return Some((&alias.t_base_url, alias.username.as_ref(), alias.password.as_ref()));
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
    pub log_active_user: bool,
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
    pub t_re_episode_pattern: Option<Regex>,
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
    pub fn prepare(&mut self, config_path: &str) -> Result<(), M3uFilterError> {
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
                    // debug!("Read ui user {}", username);
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
pub struct WebUiConfig {
    #[serde(default = "default_as_true")]
    pub enabled: bool,
    #[serde(default = "default_as_true")]
    pub user_ui_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<WebAuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_server: Option<String>,
}

impl WebUiConfig {
    pub fn prepare(&mut self, config_path: &str) -> Result<(), M3uFilterError> {
        if !self.enabled {
            self.auth = None;
        }

        if let Some(web_ui_path) = self.path.as_ref() {
            let web_path = web_ui_path.trim();
            if web_path.is_empty() {
                self.path = None;
            } else {
                let web_path = web_path.trim().trim_start_matches('/').trim_end_matches('/').to_string();
                if RESERVED_PATHS.contains(&web_path.to_lowercase().as_str()) {
                    return Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("web ui path is a reserved path. Do not use {RESERVED_PATHS:?}")));
                }
                self.path = Some(web_path);
            }
        }

        if let Some(web_auth) = &mut self.auth {
            if web_auth.enabled {
                web_auth.prepare(config_path)?;
            } else {
                self.auth = None;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigProxy {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ConfigProxy {
    fn prepare(&mut self) -> Result<(), M3uFilterError> {
        if self.username.is_some() || self.password.is_some() {
            if let (Some(username), Some(password)) = (self.username.as_ref(), self.password.as_ref()) {
                let uname = username.trim();
                let pwd = password.trim();
                if uname.is_empty() || pwd.is_empty() {
                    return Err(M3uFilterError::new(M3uFilterErrorKind::Info,"Proxy credentials missing".to_string()));
                }
                self.username = Some(uname.to_string());
                self.password = Some(pwd.to_string());
            } else {
                return Err(M3uFilterError::new(M3uFilterErrorKind::Info,"Proxy credentials missing".to_string()));
            }
        }

        self.url = self.url.trim().to_string();
        if self.url.is_empty() {
            return Err(M3uFilterError::new(M3uFilterErrorKind::Info,"Proxy url missing".to_string()));
        }
        Ok(())
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


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthcheckConfig {
    pub api: ConfigApi,
}
