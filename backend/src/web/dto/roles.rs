use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::{
    common::{
        pagination::SortOrder,
        validation::{validate_description_trimmed, validate_role_name_trimmed},
    },
    database::mongodb::bson_dt_to_chrono,
    repository::role::dao::role::Role,
    web::dto::permissions::PermissionResponse,
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RoleSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_system: bool,
}

impl From<&Role> for RoleSummary {
    fn from(r: &Role) -> Self {
        Self {
            id: r.id.to_hex(),
            name: r.name.clone(),
            description: r.description.clone(),
            is_system: r.is_system,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RoleResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub permission_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<PermissionResponse>>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl RoleResponse {
    pub fn from_role(role: Role, permissions: Option<Vec<PermissionResponse>>) -> Self {
        Self {
            id: role.id.to_hex(),
            name: role.name,
            description: role.description,
            permission_ids: role.permissions.into_iter().map(|id| id.to_hex()).collect(),
            permissions,
            is_system: role.is_system,
            created_at: bson_dt_to_chrono(role.created_at),
            updated_at: bson_dt_to_chrono(role.updated_at),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRoleRequest {
    #[validate(custom(function = "validate_role_name_trimmed"))]
    pub name: String,
    #[validate(custom(function = "validate_description_trimmed"))]
    pub description: String,
    #[serde(default)]
    #[validate(length(max = 200, message = "Cannot assign more than 200 permissions"))]
    pub permission_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateRoleRequest {
    #[validate(custom(function = "validate_role_name_trimmed"))]
    pub name: Option<String>,
    #[validate(custom(function = "validate_description_trimmed"))]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateRolePermissionsRequest {
    #[validate(length(max = 200, message = "Cannot assign more than 200 permissions"))]
    pub permission_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListRolesQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub search: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<SortOrder>,
}
