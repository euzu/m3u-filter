use std::ptr;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserCredential {
    pub username: String,
    pub password: String,
}


impl UserCredential {
    pub fn zeroize(&mut self) {
        let password_ptr = self.password.as_mut_ptr();
        let password_len = self.password.len();
        unsafe {
            ptr::write_bytes(password_ptr, 0, password_len);
        }
    }
}
