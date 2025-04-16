use openssl::symm::{Cipher, Crypter, Mode};
use base64::{engine::general_purpose, Engine as _};
use rand::Rng;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};

pub fn encrypt_text(secret: &[u8;16], text: &str) -> Result<String, M3uFilterError> {
    let iv: [u8; 16] = rand::rng().random(); // Random IV (AES-CBC 16 Bytes)
    let cipher = Cipher::aes_128_cbc();
    let mut crypter = Crypter::new(cipher, Mode::Encrypt, secret, Some(&iv)).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()))?;
    let mut ciphertext = vec![0; text.len() + cipher.block_size()];
    let mut count = crypter.update(text.as_bytes(), &mut ciphertext).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()))?;
    count += crypter.finalize(&mut ciphertext[count..]).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()))?;
    ciphertext.truncate(count);

    // IV + Ciphertext
    let mut out = iv.to_vec();
    out.extend(ciphertext);
    Ok(general_purpose::URL_SAFE_NO_PAD.encode(out))
}

pub fn decrypt_text(secret: &[u8;16], encrypted_text: &str) -> Result<String, M3uFilterError> {
    let data = general_purpose::URL_SAFE_NO_PAD.decode(encrypted_text).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()))?;
    let (iv, ciphertext) = data.split_at(16); // first 16 bytes IV
    let cipher = Cipher::aes_128_cbc();
    let mut crypter = Crypter::new(cipher, Mode::Decrypt, secret, Some(iv)).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()))?;
    let mut decrypted = vec![0; ciphertext.len() + cipher.block_size()];
    let mut count = crypter.update(ciphertext, &mut decrypted).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()))?;
    count += crypter.finalize(&mut decrypted[count..]).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()))?;
    decrypted.truncate(count);

    String::from_utf8(decrypted).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()))
}

#[cfg(test)]
mod tests {
    use rand::Rng;
    use crate::utils::crypto_utils::{decrypt_text, encrypt_text};

    #[test]
    fn test_encrypt() {
        let secret: [u8; 16] = rand::rng().random(); // Random IV (AES-CBC 16 Bytes)
        let plain = "hello world";
        let encrypted = encrypt_text(&secret, &plain);
        let decrypted = decrypt_text(&secret, &encrypted.unwrap()).unwrap();

        assert_eq!(decrypted, plain);
    }
}