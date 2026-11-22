use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

use crate::{
    common::validation::validate_password_strength, web::dto::users::UserProfileUpdateDto,
};

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateProfileRequest {
    #[validate(length(min = 1, max = 80, message = "First name must be 1 to 80 characters"))]
    pub first_name: Option<String>,
    #[validate(length(min = 1, max = 80, message = "Last name must be 1 to 80 characters"))]
    pub last_name: Option<String>,
    #[validate(email(message = "Must be a valid email address"))]
    pub email: Option<String>,
    pub profile: Option<UserProfileUpdateDto>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 1, max = 128, message = "Current password is required"))]
    pub current_password: String,
    #[validate(custom(function = "validate_password_strength"))]
    pub new_password: String,
    #[validate(must_match(
        other = "new_password",
        message = "New password confirmation does not match"
    ))]
    pub new_password_confirmation: String,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteAccountRequest {
    #[validate(length(min = 1, max = 128, message = "Current password is required"))]
    pub current_password: String,
}
