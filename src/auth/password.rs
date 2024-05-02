// use argon2::{
//     password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
//     Argon2,
// };
// use log::error;
//
// pub async fn hash(password: &[u8]) -> Option<String> {
//     let salt = SaltString::generate(&mut OsRng);
//     match Argon2::default().hash_password(password, &salt) {
//         Ok(pwd) => Some(pwd.to_string()),
//         Err(err) => {
//             error!("Failed to hash password {}", err);
//             None
//         }
//     }
// }
//
// pub fn verify_password(hash: &str, password: &[u8]) -> bool {
//     let parsed_hash = PasswordHash::new(hash)?;
//     match Argon2::default().verify_password(password, &parsed_hash) {
//         Ok(_) => true,
//         Err(_) => false
//     }
// }