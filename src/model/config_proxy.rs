use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigProxy {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ConfigProxy {
    pub(crate) fn prepare(&mut self) -> Result<(), M3uFilterError> {
        if self.username.is_some() || self.password.is_some() {
            if let (Some(username), Some(password)) = (self.username.as_ref(), self.password.as_ref()) {
                let uname = username.trim();
                let pwd = password.trim();
                if uname.is_empty() || pwd.is_empty() {
                    return Err(M3uFilterError::new(M3uFilterErrorKind::Info, "Proxy credentials missing".to_string()));
                }
                self.username = Some(uname.to_string());
                self.password = Some(pwd.to_string());
            } else {
                return Err(M3uFilterError::new(M3uFilterErrorKind::Info, "Proxy credentials missing".to_string()));
            }
        }

        self.url = self.url.trim().to_string();
        if self.url.is_empty() {
            return Err(M3uFilterError::new(M3uFilterErrorKind::Info, "Proxy url missing".to_string()));
        }
        Ok(())
    }
}