use std::fs;

use ed25519_dalek::{
    SigningKey,
    pkcs8::{EncodePrivateKey, EncodePublicKey, spki::der::pem::LineEnding},
};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{common::errors::AppError, config::Settings};

#[derive(Serialize, Deserialize)]
struct KeyPairProbeClaims {
    sub: String,
    exp: usize,
}

#[derive(Clone)]
pub struct JwtKeyPair {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl JwtKeyPair {
    pub fn from_settings(settings: &Settings) -> Result<Self, AppError> {
        let priv_path = settings.jwt.private_key_path.trim();
        let pub_path = settings.jwt.public_key_path.trim();

        match (!priv_path.is_empty(), !pub_path.is_empty()) {
            (true, true) => {
                let private_pem = fs::read(priv_path).map_err(|e| {
                    AppError::internal(format!(
                        "Failed to read JWT private key at {priv_path}: {e}"
                    ))
                })?;
                let public_pem = fs::read(pub_path).map_err(|e| {
                    AppError::internal(format!("Failed to read JWT public key at {pub_path}: {e}"))
                })?;

                return Self::from_ed_pem(&private_pem, &public_pem);
            }
            (true, false) | (false, true) => {
                return Err(AppError::internal(
                    "Both JWT_PRIVATE_KEY_PATH and JWT_PUBLIC_KEY_PATH must be configured together",
                ));
            }
            (false, false) => {}
        }

        if settings.is_production() {
            return Err(AppError::internal(
                "Refusing to generate ephemeral JWT keys in production environment",
            ));
        }

        warn!(
            "JWT_PRIVATE_KEY_PATH / JWT_PUBLIC_KEY_PATH not configured; generating ephemeral Ed25519 keypair for non-production use"
        );
        Self::generate_ephemeral()
    }

    pub fn from_ed_pem(private_pem: &[u8], public_pem: &[u8]) -> Result<Self, AppError> {
        let encoding = EncodingKey::from_ed_pem(private_pem)
            .map_err(|e| AppError::internal(format!("Invalid Ed25519 private key PEM: {e}")))?;
        let decoding = DecodingKey::from_ed_pem(public_pem)
            .map_err(|e| AppError::internal(format!("Invalid Ed25519 public key PEM: {e}")))?;

        // Startup self-check: ensure private and public keys belong to the same Ed25519 keypair.
        let probe = KeyPairProbeClaims {
            sub: "keypair-self-check".to_string(),
            exp: usize::MAX,
        };
        let token = encode(&Header::new(Algorithm::EdDSA), &probe, &encoding).map_err(|e| {
            AppError::internal(format!("JWT private key failed signing self-check: {e}"))
        })?;
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.validate_exp = false;
        decode::<KeyPairProbeClaims>(&token, &decoding, &validation).map_err(|e| {
            AppError::internal(format!(
                "JWT public key does not match the configured private key: {e}"
            ))
        })?;

        Ok(Self { encoding, decoding })
    }

    pub fn generate_ephemeral() -> Result<Self, AppError> {
        let mut rng = rand::rng();
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key = signing_key.verifying_key();

        let private_pem = signing_key.to_pkcs8_pem(LineEnding::LF).map_err(|e| {
            AppError::internal(format!("Failed to encode Ed25519 private key: {e}"))
        })?;
        let public_pem = verifying_key
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| AppError::internal(format!("Failed to encode Ed25519 public key: {e}")))?;

        Self::from_ed_pem(private_pem.as_bytes(), public_pem.as_bytes())
    }
}
