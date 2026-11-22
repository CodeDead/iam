use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::{
    common::validation::{
        validate_name_trimmed, validate_organisation_name_trimmed, validate_password_strength,
    },
    web::dto::{organisations::UserOrganisationSummary, users::UserResponse},
};

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RegisterRequest {
    #[validate(custom(function = "validate_name_trimmed"))]
    pub first_name: String,
    #[validate(custom(function = "validate_name_trimmed"))]
    pub last_name: String,
    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,
    #[validate(custom(function = "validate_password_strength"))]
    pub password: String,
    #[validate(must_match(other = "password", message = "Password confirmation does not match"))]
    pub password_confirmation: String,
    #[serde(default)]
    #[validate(custom(function = "validate_organisation_name_trimmed"))]
    pub organisation_name: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organisation: Option<UserOrganisationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MeResponse {
    pub user: UserResponse,
    pub permissions: Vec<String>,
}
