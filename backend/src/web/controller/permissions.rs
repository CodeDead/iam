use actix_web::{HttpResponse, web};

use crate::{
    app::AppState,
    common::{errors::AppError, pagination::PaginatedResponse, responses::ApiResponse},
    middleware::AuthenticatedUser,
    web::dto::{
        permissions::{
            CreatePermissionRequest, ListPermissionsQuery, PermissionResponse,
            UpdatePermissionRequest,
        },
        roles::RoleSummary,
    },
};

#[utoipa::path(
    get,
    path = "/api/v1/permissions",
    tag = "Permissions",
    security(("bearer_auth" = [])),
    params(ListPermissionsQuery),
    responses(
        (status = 200, description = "Paginated list of permissions", body = ApiResponse<PaginatedResponse<PermissionResponse>>)
    )
)]
pub async fn list_permissions(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    query: web::Query<ListPermissionsQuery>,
) -> Result<HttpResponse, AppError> {
    let page = state
        .permission_service
        .list(&actor, query.into_inner())
        .await?;
    Ok(ApiResponse::ok(page, "Permissions retrieved successfully"))
}

#[utoipa::path(
    post,
    path = "/api/v1/permissions",
    tag = "Permissions",
    security(("bearer_auth" = [])),
    request_body = CreatePermissionRequest,
    responses(
        (status = 201, description = "Permission created", body = ApiResponse<PermissionResponse>)
    )
)]
pub async fn create_permission(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    body: web::Json<CreatePermissionRequest>,
) -> Result<HttpResponse, AppError> {
    let created = state
        .permission_service
        .create(&actor, body.into_inner())
        .await?;
    Ok(ApiResponse::created(
        created,
        "Permission created successfully",
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/permissions/{id}",
    tag = "Permissions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Permission ObjectId")),
    responses(
        (status = 200, description = "Permission details", body = ApiResponse<PermissionResponse>)
    )
)]
pub async fn get_permission(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let perm = state.permission_service.get_by_id(&actor, &path).await?;
    Ok(ApiResponse::ok(perm, "Permission retrieved successfully"))
}

#[utoipa::path(
    put,
    path = "/api/v1/permissions/{id}",
    tag = "Permissions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Permission ObjectId")),
    request_body = UpdatePermissionRequest,
    responses(
        (status = 200, description = "Permission updated", body = ApiResponse<PermissionResponse>)
    )
)]
pub async fn update_permission(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdatePermissionRequest>,
) -> Result<HttpResponse, AppError> {
    let updated = state
        .permission_service
        .update(&actor, &path, body.into_inner())
        .await?;
    Ok(ApiResponse::ok(updated, "Permission updated successfully"))
}

#[utoipa::path(
    delete,
    path = "/api/v1/permissions/{id}",
    tag = "Permissions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Permission ObjectId")),
    responses(
        (status = 204, description = "Permission deleted"),
        (status = 403, description = "Protected system permission cannot be deleted")
    )
)]
pub async fn delete_permission(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    state.permission_service.delete(&actor, &path).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[utoipa::path(
    get,
    path = "/api/v1/permissions/{id}/roles",
    tag = "Permissions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Permission ObjectId")),
    responses(
        (status = 200, description = "Roles associated with permission", body = ApiResponse<Vec<RoleSummary>>)
    )
)]
pub async fn get_permission_roles(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let roles = state
        .permission_service
        .get_associated_roles(&actor, &path)
        .await?;
    Ok(ApiResponse::ok(
        roles,
        "Associated roles retrieved successfully",
    ))
}
