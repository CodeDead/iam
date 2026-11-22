use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{Algorithm, Header, Validation, decode, encode};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{common::errors::AppError, config::settings::JwtSettings, security::keys::JwtKeyPair};

pub const ACCESS_TOKEN_TYPE: &str = "access";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
    pub nbf: usize,
    pub jti: String,
    pub token_type: String,
    pub tv: u64,
}

#[derive(Clone)]
pub struct JwtService {
    keys: JwtKeyPair,
    issuer: String,
    audience: String,
    access_token_ttl: i64,
}

impl JwtService {
    pub fn new(keys: JwtKeyPair, settings: &JwtSettings) -> Self {
        Self {
            keys,
            issuer: settings.issuer.clone(),
            audience: settings.audience.clone(),
            access_token_ttl: settings.access_token_ttl,
        }
    }

    pub fn access_token_ttl_seconds(&self) -> i64 {
        self.access_token_ttl
    }

    pub fn generate_access_token(
        &self,
        user_id: ObjectId,
        token_version: u64,
    ) -> Result<(String, DateTime<Utc>), AppError> {
        let now = Utc::now();
        let expires_at = now + Duration::seconds(self.access_token_ttl);

        let iat = usize::try_from(now.timestamp())
            .map_err(|_| AppError::internal("Invalid current timestamp"))?;
        let exp = usize::try_from(expires_at.timestamp())
            .map_err(|_| AppError::internal("Invalid expiration timestamp"))?;

        let claims = AccessTokenClaims {
            sub: user_id.to_hex(),
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            exp,
            iat,
            nbf: iat,
            jti: Uuid::new_v4().to_string(),
            token_type: ACCESS_TOKEN_TYPE.to_string(),
            tv: token_version,
        };

        let header = Header::new(Algorithm::EdDSA);
        let token = encode(&header, &claims, &self.keys.encoding)
            .map_err(|e| AppError::internal(format!("Failed to sign access JWT: {e}")))?;

        Ok((token, expires_at))
    }

    pub fn verify_access_token(&self, token: &str) -> Result<AccessTokenClaims, AppError> {
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_audience(&[self.audience.as_str()]);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.leeway = 5;

        let token_data = decode::<AccessTokenClaims>(token, &self.keys.decoding, &validation)
            .map_err(|err| {
                let code = match err.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => "TOKEN_EXPIRED",
                    _ => "INVALID_TOKEN",
                };
                AppError::unauthorized(code, "Authentication token is invalid or expired")
            })?;

        if token_data.claims.token_type != ACCESS_TOKEN_TYPE {
            return Err(AppError::unauthorized(
                "INVALID_TOKEN_TYPE",
                "Token type is not valid for API authentication",
            ));
        }

        Ok(token_data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_jwt_service(ttl: i64) -> Result<JwtService, AppError> {
        let keys = JwtKeyPair::generate_ephemeral()?;
        let settings = JwtSettings {
            issuer: "test-issuer".to_string(),
            audience: "test-audience".to_string(),
            access_token_ttl: ttl,
            refresh_token_ttl: 3600,
            revoked_token_retention_seconds: 3600,
            private_key_path: String::new(),
            public_key_path: String::new(),
        };
        Ok(JwtService::new(keys, &settings))
    }

    #[test]
    fn generates_and_verifies_valid_access_token() -> Result<(), AppError> {
        let svc = test_jwt_service(900)?;
        let user_id = ObjectId::new();
        let (token, _) = svc.generate_access_token(user_id, 3)?;
        let claims = svc.verify_access_token(&token)?;

        assert_eq!(claims.sub, user_id.to_hex());
        assert_eq!(claims.iss, "test-issuer");
        assert_eq!(claims.aud, "test-audience");
        assert_eq!(claims.token_type, ACCESS_TOKEN_TYPE);
        assert_eq!(claims.tv, 3);
        Ok(())
    }

    #[test]
    fn rejects_token_signed_with_different_key() -> Result<(), AppError> {
        let svc1 = test_jwt_service(900)?;
        let svc2 = test_jwt_service(900)?;
        let (token, _) = svc1.generate_access_token(ObjectId::new(), 1)?;

        let result = svc2.verify_access_token(&token);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn rejects_expired_token() -> Result<(), AppError> {
        let svc = test_jwt_service(-60)?;
        let (token, _) = svc.generate_access_token(ObjectId::new(), 1)?;
        let result = svc.verify_access_token(&token);
        assert!(matches!(
            result,
            Err(AppError::Unauthorized { ref code, .. }) if code == "TOKEN_EXPIRED"
        ));
        Ok(())
    }
}
