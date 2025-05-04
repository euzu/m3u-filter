use crate::model::ConfigApi;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthcheckConfig {
    pub api: ConfigApi,
}
