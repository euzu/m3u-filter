use crate::utils::default_as_true;
use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;
use enum_iterator::Sequence;
use log::{debug, warn};
use url::Url;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind, create_m3u_filter_error_result, handle_m3u_filter_error_result_list, info_err};
use crate::model::{EpgConfig};
use crate::utils::config_reader::csv_read_inputs;
use crate::utils::request::{get_base_url_from_str, get_credentials_from_url, get_credentials_from_url_str};
use crate::utils::get_trimmed_string;

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

#[allow(clippy::struct_excessive_bools)]
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
