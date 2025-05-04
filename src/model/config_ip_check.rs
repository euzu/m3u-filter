use regex::Regex;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigIpCheck {
    /// URL that may return both IPv4 and IPv6 in one response
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Dedicated URL to fetch only IPv4
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_ipv4: Option<String>,

    /// Dedicated URL to fetch only IPv6
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_ipv6: Option<String>,

    /// Optional regex pattern to extract IPv4
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_ipv4: Option<String>,

    /// Optional regex pattern to extract IPv6
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_ipv6: Option<String>,

    #[serde(skip)]
    pub t_pattern_ipv4: Option<Regex>,
    #[serde(skip)]
    pub t_pattern_ipv6: Option<Regex>,
}

impl ConfigIpCheck {
    pub fn prepare(&mut self) -> Result<(), M3uFilterError> {
        if let Some(url) = &self.url {
            let url = url.trim();
            if url.is_empty() {
                self.url = None;
            } else {
                self.url = Some(url.to_owned());
            }
        }
        if let Some(url) = &self.url_ipv4 {
            let url = url.trim();
            if url.is_empty() {
                self.url_ipv4 = None;
            } else {
                self.url_ipv4 = Some(url.to_owned());
            }
        }
        if let Some(url) = &self.url_ipv6 {
            let url = url.trim();
            if url.is_empty() {
                self.url_ipv6 = None;
            } else {
                self.url_ipv6 = Some(url.to_owned());
            }
        }

        if self.url.is_none()  && self.url_ipv4.is_none() && self.url_ipv6.is_none() {
            return Err(M3uFilterError::new(M3uFilterErrorKind::Info, "No url provided!".to_owned()));
        }

        if self.url.is_some() && (self.url_ipv4.is_some() || self.url_ipv6.is_some()) {
            return Err(M3uFilterError::new(M3uFilterErrorKind::Info, "url in combination with ipv4 and/or ipv6 url not allowed!".to_owned()));
        }

        if let Some(p4) = &self.pattern_ipv4 {
            self.t_pattern_ipv4 = Some(
                Regex::new(p4).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, format!("Invalid IPv4 regex: {p4} {err}")))?,
            );
        }
        if let Some(p6) = &self.pattern_ipv6 {
            self.t_pattern_ipv6 = Some(
                Regex::new(p6).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, format!("Invalid IPv6 regex: {p6} {err}")))?,
            );
        }
        Ok(())
    }
}
