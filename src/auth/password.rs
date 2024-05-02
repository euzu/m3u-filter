use std::io::ErrorKind;
use rand::{Rng, distributions::Alphanumeric, rngs::OsRng};

fn generate_salt(length: usize) -> String {
    let rng = OsRng;
    let salt: String = rng
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect();
    salt
}

pub(crate) fn hash(password: &[u8]) -> Option<String> {
    let salt = generate_salt(64);
    if password.len() > 0 {
        let config = argon2::Config::default();
        if let Ok(hash) = argon2::hash_encoded(password, salt.as_bytes(), &config) {
            return Some(hash);
        }
    }
    None
}

pub(crate) fn verify_password(hash: &str, password: &[u8]) -> bool {
    if let Ok(valid) = argon2::verify_encoded(hash, password) {
        return valid;
    }
    false
}


pub(crate) fn generate_password() -> std::io::Result<String> {
    match rpassword::prompt_password("password> ") {
        Ok(pwd) => {
          match hash(pwd.as_bytes()) {
              None => Err(std::io::Error::new(ErrorKind::Other, "Failed to generate hash")),
              Some(hash) => Ok(hash),
          }
        },
        Err(err) => Err(err)
    }
}

