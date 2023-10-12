use std::collections::{HashMap, HashSet};
use path_absolutize::*;
use enum_iterator::Sequence;
use log::{debug, error};

use crate::filter::{Filter, get_filter, MockValueProcessor, PatternTemplate, prepare_templates, ValueProvider};
use crate::model::mapping::Mappings;
use crate::model::mapping::Mapping;
use crate::model::model_config::{ItemField, ProcessingOrder, SortOrder, TargetType, default_as_zero, default_as_false, default_as_true};
use crate::{exit, utils};
use crate::model::api_proxy::ApiProxyConfig;
use crate::utils::get_working_path;

fn default_as_frm() -> ProcessingOrder { ProcessingOrder::Frm }

fn default_as_default() -> String { String::from("default") }

fn default_as_empty_map() -> HashMap<String, String> { HashMap::new() }

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
    pub(crate) fn prepare(&mut self) {
        let re = regex::Regex::new(&self.group_pattern);
        if re.is_err() {
            exit!("cant parse regex: {}", &self.group_pattern);
        }
        self.re = Some(re.unwrap());
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
    pub(crate) fn prepare(&mut self) {
        if let Some(channels) = self.channels.as_mut() {
            channels.iter_mut().for_each(|r| r.prepare());
        }
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
    pub fn prepare(&mut self) {
        let re = regex::Regex::new(&self.pattern);
        if re.is_err() {
            exit!("cant parse regex: {}", &self.pattern);
        }
        self.re = Some(re.unwrap());
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigOptions {
    #[serde(default = "default_as_false")]
    pub ignore_logo: bool,
    #[serde(default = "default_as_false")]
    pub underscore_whitespace: bool,
    #[serde(default = "default_as_false")]
    pub cleanup: bool,
    #[serde(default = "default_as_false")]
    pub kodi_style: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigTarget {
    #[serde(skip)]
    pub id: u16,
    #[serde(default = "default_as_true")]
    pub enabled: bool,
    #[serde(default = "default_as_default")]
    pub name: String,
    #[serde(default = "default_as_false")]
    pub publish: bool,
    pub filename: Option<String>,
    pub options: Option<ConfigOptions>,
    pub sort: Option<ConfigSort>,
    pub filter: String,
    #[serde(alias = "type")]
    pub output: Option<TargetType>,
    pub rename: Option<Vec<ConfigRename>>,
    pub mapping: Option<Vec<String>>,
    #[serde(default = "default_as_frm")]
    pub processing_order: ProcessingOrder,
    #[serde(skip_serializing, skip_deserializing)]
    pub _filter: Option<Filter>,
    #[serde(skip_serializing, skip_deserializing)]
    pub _mapping: Option<Vec<Mapping>>,
}

impl ConfigTarget {
    pub fn prepare(&mut self, id: u16, templates: Option<&Vec<PatternTemplate>>) {
        self.id = id;
        let fltr = get_filter(&self.filter, templates);
        debug!("Filter: {}", fltr);
        self._filter = Some(fltr);
        if let Some(renames) = self.rename.as_mut() {
            renames.iter_mut().for_each(|r| r.prepare());
        }
        if let Some(sort) = self.sort.as_mut() {
            sort.prepare();
        }
    }
    pub fn filter(&self, provider: &ValueProvider) -> bool {
        let mut processor = MockValueProcessor {};
        return self._filter.as_ref().unwrap().filter(provider, &mut processor);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConfigSource {
    pub inputs: Vec<ConfigInput>,
    pub targets: Vec<ConfigTarget>,
}

impl ConfigSource {
    pub fn prepare(&mut self, id: u16) {
        self.inputs.iter_mut().for_each(|i| i.prepare(id));
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct InputAffix {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence)]
pub(crate) enum InputType {
    #[serde(rename = "m3u")]
    M3u,
    #[serde(rename = "xtream")]
    Xtream,
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
    pub username: Option<String>,
    pub password: Option<String>,
    pub persist: Option<String>,
    pub prefix: Option<InputAffix>,
    pub suffix: Option<InputAffix>,
    #[serde(default = "default_as_true")]
    pub enabled: bool,
}

impl ConfigInput {
    pub fn prepare(&mut self, id: u16) {
        self.id = id;
        if self.url.trim().is_empty() {
            exit!("url for input is mandatory");
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
                    debug!("for input type m3u: username and password are ignored")
                }
            }
            InputType::Xtream => {
                if self.username.is_none() || self.password.is_none() {
                    exit!("for input type xtream: username and password are mandatory");
                }
            }
        }
        if let Some(persist_path) = &self.persist {
            if persist_path.trim().is_empty() {
                self.persist = None;
            }
        }
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
pub(crate) struct MessagingConfig {
    pub telegram: Option<TelegramMessagingConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Config {
    #[serde(default = "default_as_zero")]
    pub threads: u8,
    pub api: ConfigApi,
    pub sources: Vec<ConfigSource>,
    pub working_dir: String,
    pub templates: Option<Vec<PatternTemplate>>,
    pub video_suffix: Option<Vec<String>>,
    pub schedule: Option<String>,
    pub messaging: Option<MessagingConfig>,
    #[serde(skip_serializing, skip_deserializing)]
    pub _api_proxy: Option<ApiProxyConfig>,
}

impl Config {

    pub(crate) fn has_published_targets(&self) -> bool {
        for source in &self.sources {
            for target in &source.targets {
                if target.publish {
                    return true;
                }
            }
        }
        false
    }

    pub fn set_api_proxy(&mut self, api_proxy: Option<ApiProxyConfig>) {
        self._api_proxy = api_proxy;
    }

    pub fn get_target_for_user(&self, username: &str, password: &str) -> Option<String> {
        match &self._api_proxy {
            Some(api_proxy) => api_proxy.get_target_name(username, password),
            _ => None
        }
    }

    pub fn set_mappings(&mut self, mappings: Option<Mappings>) {
        if let Some(mapping_list) = mappings {
            for source in &mut self.sources {
                for input in &source.inputs {
                    let is_m3u = matches!(input.input_type, InputType::M3u);
                    for target in &mut source.targets {
                        if is_m3u && target.filename.is_none() {
                            exit!("filename is required for m3u type: {}", target.name);
                        }
                        if !is_m3u && target.filename.is_none() && !target.publish {
                            exit!("filename or publish is required for xtream type: {}", target.name);
                        }

                        if let Some(mapping_ids) = &target.mapping {
                            let mut target_mappings = Vec::new();
                            for mapping_id in mapping_ids {
                                let mapping = mapping_list.get_mapping(mapping_id);
                                if let Some(mappings) = mapping {
                                    target_mappings.push(mappings);
                                }
                            }
                            target._mapping = if !target_mappings.is_empty() { Some(target_mappings) } else { None };
                        }
                    }
                }
            }
        }
    }

    pub fn prepare(&mut self) {
        self.working_dir = get_working_path(&self.working_dir);
        self.api.prepare();
        self.prepare_api_web_root();
        if let Some(templates) = &mut self.templates { self.templates = Some(prepare_templates(templates)) };
        // prepare sources and set id's
        let mut target_names_check = HashSet::<String>::new();
        let default_target_name = default_as_default();
        let mut source_index: u16 = 1;
        let mut target_index: u16 = 1;
        for source in &mut self.sources {
            source.prepare(source_index);
            source_index += 1;
            for target in &mut source.targets {
                // check target name is unique
                let target_name = target.name.clone();
                if !default_target_name.eq_ignore_ascii_case(target_name.as_str()) {
                    if target_names_check.contains(target_name.as_str()) {
                        exit!("target names should be unique: {}", target_name);
                    } else {
                        target_names_check.insert(target_name);
                    }
                }
                // prepare templaes
                match &self.templates {
                    Some(templ) => target.prepare(target_index, Some(templ)),
                    _ => target.prepare(target_index, None)
                }
                target_index += 1;
            }
        }
    }

    fn prepare_api_web_root(&mut self) {
        if !self.api.web_root.is_empty() {
            let wrpb = std::path::PathBuf::from(&self.api.web_root);
            if wrpb.is_relative() {
                let mut wrpb2 = std::path::PathBuf::from(&self.working_dir).join(&self.api.web_root);
                if !wrpb2.exists() {
                    wrpb2 = utils::get_exe_path().join(&self.api.web_root);
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
pub(crate) fn validate_targets(target_args: &Option<Vec<String>>, sources: &Vec<ConfigSource>) -> ProcessTargets {
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
            exit!("No target found for {}", missing_targets.join(", "));
        }
        let processing_targets: Vec<String> = check_targets.iter().filter(|&(_, v)| *v != 0).map(|(k, _)| k.to_string()).collect();
        debug!("Processing targets {}", processing_targets.join(", "));
    } else {
        enabled = false;
    }

    ProcessTargets {
        enabled,
        inputs,
        targets,
    }
}
