use regex::Regex;
use crate::tuliprox_error::{TuliProxError, TuliProxErrorKind, create_tuliprox_error, handle_tuliprox_error_result_list};
use crate::model::{ItemField};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SortOrder {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}

fn compile_regex_vec(patterns: Option<&Vec<String>>) -> Result<Option<Vec<Regex>>, TuliProxError> {
    patterns.as_ref()
        .map(|seq| {
            seq.iter()
                .map(|s| Regex::new(s).map_err(|err| {
                    create_tuliprox_error!(TuliProxErrorKind::Info, "cant parse regex: {s} {err}")
                }))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose() // convert Option<Result<...>> to Result<Option<...>>
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigSortGroup {
    pub order: SortOrder,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<Vec<String>>,
    #[serde(default, skip)]
    pub t_sequence: Option<Vec<Regex>>,
}


impl ConfigSortGroup {
    pub fn prepare(&mut self) -> Result<(), TuliProxError> {
        // Compile sequence patterns, if any
        self.t_sequence = compile_regex_vec(self.sequence.as_ref())?;
        Ok(())
    }
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
    pub fn prepare(&mut self) -> Result<(), TuliProxError> {
        // Compile group_pattern
        self.t_re_group_pattern = Some(
            Regex::new(&self.group_pattern).map_err(|err| {
                create_tuliprox_error!(TuliProxErrorKind::Info, "cant parse regex: {} {err}", &self.group_pattern)
            })?
        );

        // Compile sequence patterns, if any
        self.t_sequence = compile_regex_vec(self.sequence.as_ref())?;
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
    pub fn prepare(&mut self) -> Result<(), TuliProxError> {
        if let Some(group) = self.groups.as_mut() {
            group.prepare()?;
        }
        if let Some(channels) = self.channels.as_mut() {
            handle_tuliprox_error_result_list!(TuliProxErrorKind::Info, channels.iter_mut().map(ConfigSortChannel::prepare));
        }
        Ok(())
    }
}