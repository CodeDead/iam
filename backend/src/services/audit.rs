use std::sync::Arc;

use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use tracing::{error, info, warn};

use crate::{
    common::{
        errors::AppError,
        pagination::{PaginatedResponse, PaginationParams, SortOrder},
        validation::parse_object_id,
    },
    middleware::AuthenticatedUser,
    repository::audit::{
        audit_log_repository::{AuditLogFilter, AuditLogRepository},
        dao::audit_log::AuditLog,
    },
    web::dto::audit::{AuditLogResponse, ListAuditLogsQuery},
};

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub action: &'static str,
    pub resource_type: &'static str,
    pub resource_id: Option<String>,
    pub resource_name: Option<String>,
    pub actor_id: Option<ObjectId>,
    pub actor_email: Option<String>,
    pub summary: String,
    pub details: Option<serde_json::Value>,
    pub is_warning: bool,
}

impl AuditEvent {
    pub fn by_actor(
        actor: &AuthenticatedUser,
        action: &'static str,
        resource_type: &'static str,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            action,
            resource_type,
            resource_id: None,
            resource_name: None,
            actor_id: Some(actor.user_id),
            actor_email: Some(actor.email.clone()),
            summary: summary.into(),
            details: None,
            is_warning: false,
        }
    }

    pub fn system(
        action: &'static str,
        resource_type: &'static str,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            action,
            resource_type,
            resource_id: None,
            resource_name: None,
            actor_id: None,
            actor_email: None,
            summary: summary.into(),
            details: None,
            is_warning: false,
        }
    }

    pub fn with_actor(mut self, actor_id: Option<ObjectId>, actor_email: Option<String>) -> Self {
        self.actor_id = actor_id;
        self.actor_email = actor_email;
        self
    }

    pub fn with_resource(
        mut self,
        resource_id: impl Into<String>,
        resource_name: impl Into<String>,
    ) -> Self {
        self.resource_id = Some(resource_id.into());
        self.resource_name = Some(resource_name.into());
        self
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn as_warning(mut self) -> Self {
        self.is_warning = true;
        self
    }
}

#[derive(Clone)]
pub struct AuditService {
    audit_repo: Arc<dyn AuditLogRepository>,
}

impl AuditService {
    pub fn new(audit_repo: Arc<dyn AuditLogRepository>) -> Self {
        Self { audit_repo }
    }

    pub async fn record(&self, event: AuditEvent) {
        let actor_id_str = event
            .actor_id
            .map(|id| id.to_hex())
            .unwrap_or_else(|| "system".to_string());
        let actor_email_str = event.actor_email.as_deref().unwrap_or("system");
        let resource_id_str = event.resource_id.as_deref().unwrap_or("-");
        let resource_name_str = event.resource_name.as_deref().unwrap_or("-");

        if event.is_warning {
            warn!(
                target: "auth_backend::audit",
                action = event.action,
                resource_type = event.resource_type,
                resource_id = resource_id_str,
                resource_name = resource_name_str,
                actor_id = %actor_id_str,
                actor_email = actor_email_str,
                "{}",
                event.summary
            );
        } else {
            info!(
                target: "auth_backend::audit",
                action = event.action,
                resource_type = event.resource_type,
                resource_id = resource_id_str,
                resource_name = resource_name_str,
                actor_id = %actor_id_str,
                actor_email = actor_email_str,
                "{}",
                event.summary
            );
        }

        let entry = AuditLog {
            id: ObjectId::new(),
            action: event.action.to_string(),
            resource_type: event.resource_type.to_string(),
            resource_id: event.resource_id,
            resource_name: event.resource_name,
            actor_id: event.actor_id,
            actor_email: event.actor_email,
            summary: event.summary,
            details: event.details,
            created_at: BsonDateTime::now(),
        };

        if let Err(err) = self.audit_repo.create(&entry).await {
            error!(
                target: "auth_backend::audit",
                action = %entry.action,
                error = %err,
                "Failed to persist audit log entry"
            );
        }
    }

    pub async fn list(
        &self,
        actor: &AuthenticatedUser,
        query: ListAuditLogsQuery,
    ) -> Result<PaginatedResponse<AuditLogResponse>, AppError> {
        actor.require_permission("audit_logs.read")?;

        let pagination = PaginationParams {
            page: query.page,
            page_size: query.page_size,
            sort_by: None,
            sort_order: query.sort_order,
        };

        let filter = AuditLogFilter {
            search: query.search,
            resource_type: query.resource_type,
            action: query.action,
            actor_email: query.actor_email,
            skip: pagination.skip(),
            limit: pagination.normalized_page_size(),
            sort_order: Some(query.sort_order.unwrap_or(SortOrder::Desc)),
        };

        let (entries, total) = self.audit_repo.find_all(&filter).await?;
        let items: Vec<AuditLogResponse> =
            entries.into_iter().map(AuditLogResponse::from).collect();

        Ok(PaginatedResponse::new(
            items,
            total,
            pagination.normalized_page(),
            pagination.normalized_page_size(),
        ))
    }

    pub async fn get_by_id(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
    ) -> Result<AuditLogResponse, AppError> {
        actor.require_permission("audit_logs.read")?;
        let log_id = parse_object_id(id)?;
        let entry = self.audit_repo.find_by_id(log_id).await?.ok_or_else(|| {
            AppError::not_found("AUDIT_LOG_NOT_FOUND", "Audit log entry not found")
        })?;
        Ok(AuditLogResponse::from(entry))
    }
}
