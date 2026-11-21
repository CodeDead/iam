use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::{common::validation::validate_password_strength, web::dto::users::UserResponse};

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RegisterRequest {
    #[validate(length(min = 1, max = 80, message = "First name must be 1 to 80 characters"))]
    pub first_name: String,
    #[validate(length(min = 1, max = 80, message = "Last name must be 1 to 80 characters"))]
    pub last_name: String,
    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,
    #[validate(custom(function = "validate_password_strength"))]
    pub password: String,
    #[validate(must_match(other = "password", message = "Password confirmation does not match"))]
    pub password_confirmation: String,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,
    #[validate(length(min = 1, max = 128, message = "Password is required"))]
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub expires_at: DateTime<Utc>,
    pub csrf_token: String,
    pub user: UserResponse,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MeResponse {
    pub user: UserResponse,
    pub permissions: Vec<String>,
}
