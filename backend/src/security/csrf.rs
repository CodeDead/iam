use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub fn generate_opaque_token(num_bytes: usize) -> String {
    let mut bytes = vec![0u8; num_bytes];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn sha256_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    bool::from(a_bytes.ct_eq(b_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_distinct_opaque_tokens() {
        let t1 = generate_opaque_token(32);
        let t2 = generate_opaque_token(32);
        assert_ne!(t1, t2);
        assert!(t1.len() >= 40);
    }

    #[test]
    fn constant_time_comparison_works() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("abc", "abcd"));
    }
}
