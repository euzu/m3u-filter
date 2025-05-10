use crate::utils::default_as_true;
use crate::tuliprox_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::WebAuthConfig;

const RESERVED_PATHS: &[&str] = &[
    "live", "movie", "series", "m3u-stream", "healthcheck", "status",
    "player_api.php", "panel_api.php", "xtream", "timeshift", "timeshift.php", "streaming",
    "get.php", "apiget", "m3u", "resource"
];

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