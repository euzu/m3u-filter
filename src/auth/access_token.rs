use chrono::{Utc};
use serde::{Serialize, Deserialize};
use crate::repository::storage::hex_encode;

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[derive(Serialize, Deserialize, Debug)]
struct AccessToken {
    timestamp: i64,
    ttl_secs: i64,
    signature: String,
}

pub fn create_access_token(secret: &[u8; 32], ttl_secs: i64) -> String {
    let timestamp = Utc::now().timestamp();
    let data = format!("{}", timestamp);

    let hash = blake3::keyed_hash(secret, data.as_bytes());
    let signature = hex_encode(hash.as_bytes());

    // token as json
    let token = AccessToken {
        timestamp,
        ttl_secs,
        signature,
    };

    // serialize as json
    serde_json::to_string(&token).unwrap()
}

pub fn verify_access_token(token_str: &str, secret: &[u8; 32]) -> bool {
    // deserialize token
    let token: AccessToken = serde_json::from_str(token_str).unwrap();

    // Validate time
    let current_timestamp = Utc::now().timestamp();
    if current_timestamp - token.timestamp > token.ttl_secs {
        return false;
    }

    // Create HMAC-Hash for the timestamp with blake3
    let data = token.timestamp.to_string();
    let expected = blake3::keyed_hash(secret, data.as_bytes());
    let expected_hash = hex_encode(expected.as_bytes());
    constant_time_eq(expected_hash.as_bytes(), token.signature.as_bytes())
}

#[cfg(test)]
mod tests {
    use std::thread;
    use crate::auth::access_token::{create_access_token, verify_access_token};

    #[test]
    fn test_valid_token() {
        let secret = b"37c30f739e83ba27b4c17b174c31f3a9";
        let token = create_access_token(secret, 1);
        assert_eq!(verify_access_token(token.as_str(), secret), true);
        thread::sleep(std::time::Duration::from_secs(2));
        assert_eq!(verify_access_token(token.as_str(), secret), false);
    }
}