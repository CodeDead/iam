use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::web::dto::organisations::UserOrganisationSummary;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserStats {
    pub total: u64,
    pub active: u64,
    pub disabled: u64,
    pub locked: u64,
    pub pending_verification: u64,
    pub new_last_7_days: u64,
    pub active_sessions: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RoleDistributionItem {
    pub id: String,
    pub name: String,
    pub is_system: bool,
    pub permission_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RoleStats {
    pub total: u64,
    pub system: u64,
    pub custom: u64,
    pub distribution: Vec<RoleDistributionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResourceCountItem {
    pub resource: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PermissionStats {
    pub total: u64,
    pub system: u64,
    pub custom: u64,
    pub by_resource: Vec<ResourceCountItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DailyActivityPoint {
    pub date: String,
    pub total: u64,
    pub auth_success: u64,
    pub auth_failed: u64,
    pub admin_changes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RecentAuditEvent {
    pub id: String,
    pub action: String,
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_email: Option<String>,
    pub summary: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuditStats {
    pub total: u64,
    pub events_last_24h: u64,
    pub events_last_7d: u64,
    pub failed_logins_last_24h: u64,
    pub token_reuse_alerts_last_24h: u64,
    pub token_reuse_alerts_last_7d: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_token_reuse_at: Option<DateTime<Utc>>,
    pub daily_activity: Vec<DailyActivityPoint>,
    pub by_resource_type: Vec<ResourceCountItem>,
    pub recent_events: Vec<RecentAuditEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrganisationSizeItem {
    pub id: String,
    pub name: String,
    pub member_count: u64,
    pub active_member_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrganisationStats {
    pub total: u64,
    pub new_last_7_days: u64,
    pub total_memberships: u64,
    pub active_memberships: u64,
    pub invited_memberships: u64,
    pub suspended_memberships: u64,
    pub largest_organisations: Vec<OrganisationSizeItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DashboardStatsResponse {
    pub my_active_sessions: u64,
    #[serde(default)]
    pub my_organisations: Vec<UserOrganisationSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<UserStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roles: Option<RoleStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<PermissionStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organisations: Option<OrganisationStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit: Option<AuditStats>,
}
