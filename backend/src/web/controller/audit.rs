use actix_web::{HttpResponse, web};

use crate::{
    app::AppState,
    common::{errors::AppError, pagination::PaginatedResponse, responses::ApiResponse},
    middleware::AuthenticatedUser,
    web::dto::audit::{AuditLogResponse, ListAuditLogsQuery},
};

#[utoipa::path(
    get,
    path = "/api/v1/audit-logs",
    tag = "Audit Logs",
    security(("bearer_auth" = [])),
    params(ListAuditLogsQuery),
    responses(
        (status = 200, description = "Paginated list of audit log entries", body = ApiResponse<PaginatedResponse<AuditLogResponse>>)
    )
)]
pub async fn list_audit_logs(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    query: web::Query<ListAuditLogsQuery>,
) -> Result<HttpResponse, AppError> {
    let page = state.audit_service.list(&actor, query.into_inner()).await?;
    Ok(ApiResponse::ok(page, "Audit logs retrieved successfully"))
}

#[utoipa::path(
    get,
    path = "/api/v1/audit-logs/{id}",
    tag = "Audit Logs",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Audit Log ObjectId")),
    responses(
        (status = 200, description = "Audit log entry details", body = ApiResponse<AuditLogResponse>)
    )
)]
pub async fn get_audit_log(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let entry = state.audit_service.get_by_id(&actor, &path).await?;
    Ok(ApiResponse::ok(entry, "Audit log retrieved successfully"))
}
