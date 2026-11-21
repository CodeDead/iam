use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::{
    common::{pagination::SortOrder, validation::validate_permission_segment},
    database::mongodb::bson_dt_to_chrono,
    repository::permission::dao::permission::Permission,
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PermissionResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub resource: String,
    pub action: String,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Permission> for PermissionResponse {
    fn from(p: Permission) -> Self {
        Self {
            id: p.id.to_hex(),
            name: p.name,
            description: p.description,
            resource: p.resource,
            action: p.action,
            is_system: p.is_system,
            created_at: bson_dt_to_chrono(p.created_at),
            updated_at: bson_dt_to_chrono(p.updated_at),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatePermissionRequest {
    #[validate(custom(function = "validate_permission_segment"))]
    pub resource: String,
    #[validate(custom(function = "validate_permission_segment"))]
    pub action: String,
    #[validate(length(
        min = 1,
        max = 255,
        message = "Description must be 1 to 255 characters"
    ))]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdatePermissionRequest {
    #[validate(custom(function = "validate_permission_segment"))]
    pub resource: Option<String>,
    #[validate(custom(function = "validate_permission_segment"))]
    pub action: Option<String>,
    #[validate(length(
        min = 1,
        max = 255,
        message = "Description must be 1 to 255 characters"
    ))]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListPermissionsQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub search: Option<String>,
    pub resource: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<SortOrder>,
}
