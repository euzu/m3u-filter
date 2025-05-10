use crate::utils::default_as_true;
use std::fs::File;
use std::io::BufRead;
use std::path::PathBuf;
use crate::auth::user::UserCredential;
use crate::tuliprox_error::{M3uFilterError, M3uFilterErrorKind, create_tuliprox_error_result};
use crate::utils::file_utils;
use crate::utils::file_utils::file_reader;

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
                return create_tuliprox_error_result!(M3uFilterErrorKind::Info, "Could not find userfile {}", &userfile_name);
            }
        }

        if let Ok(file) = File::open(&userfile_path) {
            let mut users = vec![];
            let reader = file_reader(file);
            // TODO maybe print out errors
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
            return create_tuliprox_error_result!(M3uFilterErrorKind::Info, "Could not read userfile {:?}", &userfile_path);
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