use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};

use crate::common::errors::AppError;

fn build_argon2() -> Result<Argon2<'static>, AppError> {
    // OWASP-aligned Argon2id parameters: 19 MiB memory, 2 iterations, 1 lane.
    let params = Params::new(19_456, 2, 1, None)
        .map_err(|e| AppError::internal(format!("Failed to construct Argon2id params: {e}")))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

pub fn hash_password_sync(password: &str) -> Result<String, AppError> {
    let argon2 = build_argon2()?;
    let hash = argon2
        .hash_password(password.as_bytes())
        .map_err(|e| AppError::internal(format!("Password hashing failed: {e}")))?;
    Ok(hash.to_string())
}

pub fn verify_password_sync(password: &str, password_hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(password_hash)
        .map_err(|e| AppError::internal(format!("Invalid stored password hash format: {e}")))?;
    let argon2 = build_argon2()?;
    Ok(argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub async fn hash_password(password: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || hash_password_sync(&password))
        .await
        .map_err(|e| AppError::internal(format!("Password hashing task failed: {e}")))?
}

pub async fn verify_password(password: String, password_hash: String) -> Result<bool, AppError> {
    tokio::task::spawn_blocking(move || verify_password_sync(&password, &password_hash))
        .await
        .map_err(|e| AppError::internal(format!("Password verification task failed: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_and_verifies_password_with_argon2id() -> Result<(), AppError> {
        let hash = hash_password_sync("Sup3rS3cret!Pass")?;
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password_sync("Sup3rS3cret!Pass", &hash)?);
        assert!(!verify_password_sync("WrongPassword!1", &hash)?);
        Ok(())
    }

    #[test]
    fn generates_unique_salts_per_hash() -> Result<(), AppError> {
        let h1 = hash_password_sync("SamePass123!")?;
        let h2 = hash_password_sync("SamePass123!")?;
        assert_ne!(h1, h2);
        Ok(())
    }
}
