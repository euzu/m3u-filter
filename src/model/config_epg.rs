use log::warn;
use regex::Regex;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind, create_m3u_filter_error_result};
use crate::utils::CONSTANTS;

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