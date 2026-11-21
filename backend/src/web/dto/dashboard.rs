use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserStats {
    pub total: u64,
    pub active: u64,
    pub disabled: u64,
    pub locked: u64,
    pub pending_verification: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DashboardStatsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<UserStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_roles: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_permissions: Option<u64>,
}
