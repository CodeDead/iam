use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::{
    common::{
        pagination::SortOrder,
        validation::{validate_name_trimmed, validate_password_strength},
    },
    database::mongodb::bson_dt_to_chrono,
    repository::user::dao::user::{AccountStatus, User, UserProfileInfo},
    web::dto::roles::RoleSummary,
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    pub id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub status: AccountStatus,
    pub email_verified: bool,
    pub roles: Vec<RoleSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
    pub profile: UserProfileInfo,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl UserResponse {
    pub fn from_user(
        user: User,
        roles: Vec<RoleSummary>,
        permissions: Option<Vec<String>>,
    ) -> Self {
        Self {
            id: user.id.to_hex(),
            first_name: user.first_name,
            last_name: user.last_name,
            email: user.email,
            status: user.status,
            email_verified: user.email_verified,
            roles,
            permissions,
            profile: user.profile,
            created_at: bson_dt_to_chrono(user.created_at),
            updated_at: bson_dt_to_chrono(user.updated_at),
            last_login_at: user.last_login_at.map(bson_dt_to_chrono),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateUserRequest {
    #[validate(custom(function = "validate_name_trimmed"))]
    pub first_name: String,
    #[validate(custom(function = "validate_name_trimmed"))]
    pub last_name: String,
    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,
    #[validate(custom(function = "validate_password_strength"))]
    pub password: String,
    #[serde(default)]
    pub status: Option<AccountStatus>,
    #[serde(default)]
    pub email_verified: Option<bool>,
    #[serde(default)]
    #[validate(length(max = 100, message = "Cannot assign more than 100 roles"))]
    pub role_ids: Vec<String>,
    #[serde(default)]
    pub profile: Option<UserProfileUpdateDto>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateUserRequest {
    #[validate(custom(function = "validate_name_trimmed"))]
    pub first_name: Option<String>,
    #[validate(custom(function = "validate_name_trimmed"))]
    pub last_name: Option<String>,
    #[validate(email(message = "Must be a valid email address"))]
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub profile: Option<UserProfileUpdateDto>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserProfileUpdateDto {
    #[validate(length(max = 32, message = "Phone number must be at most 32 characters"))]
    pub phone: Option<String>,
    #[validate(length(max = 100, message = "Job title must be at most 100 characters"))]
    pub job_title: Option<String>,
    #[validate(length(max = 100, message = "Department must be at most 100 characters"))]
    pub department: Option<String>,
    #[validate(length(max = 500, message = "Bio must be at most 500 characters"))]
    pub bio: Option<String>,
}

impl From<UserProfileUpdateDto> for UserProfileInfo {
    fn from(dto: UserProfileUpdateDto) -> Self {
        let clean =
            |opt: Option<String>| opt.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        Self {
            phone: clean(dto.phone),
            job_title: clean(dto.job_title),
            department: clean(dto.department),
            bio: clean(dto.bio),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateUserStatusRequest {
    pub status: AccountStatus,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateUserRolesRequest {
    #[validate(length(
        min = 1,
        max = 100,
        message = "A user must have 1 to 100 assigned roles"
    ))]
    pub role_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListUsersQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub search: Option<String>,
    pub status: Option<AccountStatus>,
    pub sort_by: Option<String>,
    pub sort_order: Option<SortOrder>,
}
