use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use base64::{engine::general_purpose, Engine as _};
use openssl::symm::{Cipher, Crypter, Mode};
use rand::Rng;


fn encode_base64_string(input: &[u8]) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(input)
}

fn decode_base64_string(input: &str) -> Vec<u8> {
    general_purpose::URL_SAFE_NO_PAD.decode(input).unwrap_or_else(|_| input.as_bytes().to_vec())
}

pub fn xor_bytes(secret: &[u8], data: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, &b)| b ^ secret[i % secret.len()])
        .collect()
}

pub fn obfuscate_text(secret: &[u8], text: &str) -> Result<String, String> {
    Ok(encode_base64_string(&xor_bytes(secret, text.as_bytes())))
}

pub fn deobfuscate_text(secret: &[u8], text: &str) -> Result<String, String> {
    let data = xor_bytes(secret, &decode_base64_string(text));
    if let Ok(result) = String::from_utf8(data) {
        Ok(result)
    } else {
        Err(text.to_string())
    }
}

pub fn encrypt_text(secret: &[u8; 16], text: &str) -> Result<String, M3uFilterError> {
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

pub fn decrypt_text(secret: &[u8; 16], encrypted_text: &str) -> Result<String, M3uFilterError> {
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
    use crate::utils::crypto_utils::{decrypt_text, deobfuscate_text, encrypt_text, obfuscate_text};
    use rand::Rng;

    #[test]
    fn test_encrypt() {
        let secret: [u8; 16] = rand::rng().random(); // Random IV (AES-CBC 16 Bytes)
        let plain = "hello world";
        let encrypted = encrypt_text(&secret, &plain);
        let decrypted = decrypt_text(&secret, &encrypted.unwrap()).unwrap();

        assert_eq!(decrypted, plain);
    }
    #[test]
    fn test_obfuscate() {
        let secret: [u8; 16] = rand::rng().random(); // Random IV (AES-CBC 16 Bytes)
        let plain = "hello world";
        let encrypted = obfuscate_text(&secret, &plain);
        let decrypted = deobfuscate_text(&secret, &encrypted.unwrap()).unwrap();

        assert_eq!(decrypted, plain);
    }
}