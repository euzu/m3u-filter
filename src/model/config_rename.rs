use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind, create_m3u_filter_error_result};
use crate::model::ItemField;

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