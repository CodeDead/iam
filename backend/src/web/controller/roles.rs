use actix_web::{HttpResponse, web};

use crate::{
    app::AppState,
    common::{errors::AppError, pagination::PaginatedResponse, responses::ApiResponse},
    middleware::AuthenticatedUser,
    web::dto::{
        permissions::PermissionResponse,
        roles::{
            CreateRoleRequest, ListRolesQuery, RoleResponse, UpdateRolePermissionsRequest,
            UpdateRoleRequest,
        },
        users::UserResponse,
    },
};

#[utoipa::path(
    get,
    path = "/api/v1/roles",
    tag = "Roles",
    security(("bearer_auth" = [])),
    params(ListRolesQuery),
    responses(
        (status = 200, description = "Paginated list of roles", body = ApiResponse<PaginatedResponse<RoleResponse>>)
    )
)]
pub async fn list_roles(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    query: web::Query<ListRolesQuery>,
) -> Result<HttpResponse, AppError> {
    let page = state.role_service.list(&actor, query.into_inner()).await?;
    Ok(ApiResponse::ok(page, "Roles retrieved successfully"))
}

#[utoipa::path(
    post,
    path = "/api/v1/roles",
    tag = "Roles",
    security(("bearer_auth" = [])),
    request_body = CreateRoleRequest,
    responses(
        (status = 201, description = "Role created", body = ApiResponse<RoleResponse>)
    )
)]
pub async fn create_role(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    body: web::Json<CreateRoleRequest>,
) -> Result<HttpResponse, AppError> {
    let created = state.role_service.create(&actor, body.into_inner()).await?;
    Ok(ApiResponse::created(created, "Role created successfully"))
}

#[utoipa::path(
    get,
    path = "/api/v1/roles/{id}",
    tag = "Roles",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Role ObjectId")),
    responses(
        (status = 200, description = "Role details", body = ApiResponse<RoleResponse>)
    )
)]
pub async fn get_role(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let role = state.role_service.get_by_id(&actor, &path).await?;
    Ok(ApiResponse::ok(role, "Role retrieved successfully"))
}

#[utoipa::path(
    put,
    path = "/api/v1/roles/{id}",
    tag = "Roles",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Role ObjectId")),
    request_body = UpdateRoleRequest,
    responses(
        (status = 200, description = "Role updated", body = ApiResponse<RoleResponse>)
    )
)]
pub async fn update_role(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdateRoleRequest>,
) -> Result<HttpResponse, AppError> {
    let updated = state
        .role_service
        .update(&actor, &path, body.into_inner())
        .await?;
    Ok(ApiResponse::ok(updated, "Role updated successfully"))
}

#[utoipa::path(
    delete,
    path = "/api/v1/roles/{id}",
    tag = "Roles",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Role ObjectId")),
    responses(
        (status = 204, description = "Role deleted"),
        (status = 403, description = "Protected system role cannot be deleted")
    )
)]
pub async fn delete_role(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    state.role_service.delete(&actor, &path).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[utoipa::path(
    get,
    path = "/api/v1/roles/{id}/permissions",
    tag = "Roles",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Role ObjectId")),
    responses(
        (status = 200, description = "Permissions assigned to role", body = ApiResponse<Vec<PermissionResponse>>)
    )
)]
pub async fn get_role_permissions(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let perms = state.role_service.get_permissions(&actor, &path).await?;
    Ok(ApiResponse::ok(
        perms,
        "Role permissions retrieved successfully",
    ))
}

#[utoipa::path(
    put,
    path = "/api/v1/roles/{id}/permissions",
    tag = "Roles",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Role ObjectId")),
    request_body = UpdateRolePermissionsRequest,
    responses(
        (status = 200, description = "Role permissions updated", body = ApiResponse<RoleResponse>)
    )
)]
pub async fn update_role_permissions(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdateRolePermissionsRequest>,
) -> Result<HttpResponse, AppError> {
    let updated = state
        .role_service
        .update_permissions(&actor, &path, body.into_inner())
        .await?;
    Ok(ApiResponse::ok(
        updated,
        "Role permissions updated successfully",
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/roles/{id}/users",
    tag = "Roles",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Role ObjectId")),
    responses(
        (status = 200, description = "Users assigned to role", body = ApiResponse<Vec<UserResponse>>)
    )
)]
pub async fn get_role_users(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let users = state.role_service.get_assigned_users(&actor, &path).await?;
    Ok(ApiResponse::ok(
        users,
        "Assigned users retrieved successfully",
    ))
}
