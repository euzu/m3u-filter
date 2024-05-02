use std::ptr;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct UserCredential {
    pub username: String,
    pub password: String,
}


impl UserCredential {
    pub(crate) fn zeroize(&mut self) {
        unsafe {
            let password_ptr = self.password.as_mut_ptr();
            let password_len = self.password.len();
            ptr::write_bytes(password_ptr, 0, password_len);
        }
    }
}
