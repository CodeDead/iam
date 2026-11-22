use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    common::pagination::SortOrder, database::mongodb::bson_dt_to_chrono,
    repository::audit::dao::audit_log::AuditLog,
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuditLogResponse {
    pub id: String,
    pub action: String,
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_email: Option<String>,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

impl From<AuditLog> for AuditLogResponse {
    fn from(entry: AuditLog) -> Self {
        Self {
            id: entry.id.to_hex(),
            action: entry.action,
            resource_type: entry.resource_type,
            resource_id: entry.resource_id,
            resource_name: entry.resource_name,
            actor_id: entry.actor_id.map(|id| id.to_hex()),
            actor_email: entry.actor_email,
            summary: entry.summary,
            details: entry.details,
            created_at: bson_dt_to_chrono(entry.created_at),
        }
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListAuditLogsQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub search: Option<String>,
    pub resource_type: Option<String>,
    pub action: Option<String>,
    pub actor_email: Option<String>,
    pub sort_order: Option<SortOrder>,
}
