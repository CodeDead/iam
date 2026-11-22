use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::{
    common::{pagination::SortOrder, validation::validate_organisation_name_trimmed},
    repository::organisation::dao::organisation::{
        OrganisationMemberRole, OrganisationMemberStatus,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrganisationMemberResponse {
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    pub role: OrganisationMemberRole,
    pub status: OrganisationMemberStatus,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrganisationResponse {
    pub id: String,
    pub name: String,
    pub members: Vec<OrganisationMemberResponse>,
    pub member_count: u64,
    pub active_member_count: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserOrganisationSummary {
    pub id: String,
    pub name: String,
    pub role: OrganisationMemberRole,
    pub status: OrganisationMemberStatus,
    pub member_count: u64,
    pub active_member_count: u64,
    pub active_owner_count: u64,
    pub joined_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateOrganisationRequest {
    #[validate(custom(function = "validate_organisation_name_trimmed"))]
    pub name: String,
    #[serde(default)]
    #[validate(length(max = 100, message = "Cannot attach more than 100 initial members"))]
    pub initial_member_ids: Option<Vec<String>>,
    #[serde(default)]
    #[validate(email(message = "Must be a valid email address"))]
    pub initial_owner_email: Option<String>,
    #[serde(default)]
    pub include_creator: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateOrganisationRequest {
    #[validate(custom(function = "validate_organisation_name_trimmed"))]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AddOrganisationMemberRequest {
    pub user_id: Option<String>,
    #[validate(email(message = "Must be a valid email address"))]
    pub email: Option<String>,
    #[serde(default)]
    pub role: Option<OrganisationMemberRole>,
    #[serde(default)]
    pub status: Option<OrganisationMemberStatus>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateOrganisationMemberRequest {
    #[serde(default)]
    pub status: Option<OrganisationMemberStatus>,
    #[serde(default)]
    pub role: Option<OrganisationMemberRole>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListOrganisationsQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub search: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<SortOrder>,
}
