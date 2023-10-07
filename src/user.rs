#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct UserCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TargetUser {
    pub target_name: String,
    pub credentials: Vec<UserCredentials>,
}

impl TargetUser {
    pub fn get_target_name(&self, username: &str, password: &str) -> Option<String> {
        if self.credentials.iter().find(|c| c.username.eq_ignore_ascii_case(username) && c.password.eq(password)).is_some() {
            return Some(self.target_name.clone());
        }
        None
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct User {
    pub user: TargetUser,
}

impl User {
    pub fn prepare(&self, _verbose: bool) {
        // TODO check if username is unique, a user can only access one target
    }

    pub fn get_target_name(&self, username: &str, password: &str) -> Option<String> {
        self.user.get_target_name(username, password)
    }
}
