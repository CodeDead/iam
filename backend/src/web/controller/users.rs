use actix_web::{HttpResponse, web};

use crate::{
    app::AppState,
    common::{errors::AppError, pagination::PaginatedResponse, responses::ApiResponse},
    middleware::AuthenticatedUser,
    web::dto::{
        organisations::UserOrganisationSummary,
        permissions::PermissionResponse,
        roles::RoleSummary,
        users::{
            CreateUserRequest, ListUsersQuery, UpdateUserRequest, UpdateUserRolesRequest,
            UpdateUserStatusRequest, UserResponse,
        },
    },
};

#[utoipa::path(
    get,
    path = "/api/v1/users",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(ListUsersQuery),
    responses(
        (status = 200, description = "Paginated list of users", body = ApiResponse<PaginatedResponse<UserResponse>>),
        (status = 401, description = "Unauthenticated"),
        (status = 403, description = "Insufficient permissions")
    )
)]
pub async fn list_users(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    query: web::Query<ListUsersQuery>,
) -> Result<HttpResponse, AppError> {
    let page = state.user_service.list(&actor, query.into_inner()).await?;
    Ok(ApiResponse::ok(page, "Users retrieved successfully"))
}

#[utoipa::path(
    post,
    path = "/api/v1/users",
    tag = "Users",
    security(("bearer_auth" = [])),
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created successfully", body = ApiResponse<UserResponse>),
        (status = 403, description = "Insufficient permissions"),
        (status = 409, description = "Email already exists")
    )
)]
pub async fn create_user(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    body: web::Json<CreateUserRequest>,
) -> Result<HttpResponse, AppError> {
    let created = state.user_service.create(&actor, body.into_inner()).await?;
    Ok(ApiResponse::created(created, "User created successfully"))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/{id}",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "User ObjectId")),
    responses(
        (status = 200, description = "User details retrieved", body = ApiResponse<UserResponse>),
        (status = 404, description = "User not found")
    )
)]
pub async fn get_user(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let user = state.user_service.get_by_id(&actor, &path).await?;
    Ok(ApiResponse::ok(user, "User retrieved successfully"))
}

#[utoipa::path(
    put,
    path = "/api/v1/users/{id}",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "User ObjectId")),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User updated", body = ApiResponse<UserResponse>),
        (status = 404, description = "User not found")
    )
)]
pub async fn update_user(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdateUserRequest>,
) -> Result<HttpResponse, AppError> {
    let updated = state
        .user_service
        .update(&actor, &path, body.into_inner())
        .await?;
    Ok(ApiResponse::ok(updated, "User updated successfully"))
}

#[utoipa::path(
    patch,
    path = "/api/v1/users/{id}/status",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "User ObjectId")),
    request_body = UpdateUserStatusRequest,
    responses(
        (status = 200, description = "User status updated", body = ApiResponse<UserResponse>),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Cannot disable last active Super Administrator")
    )
)]
pub async fn update_user_status(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdateUserStatusRequest>,
) -> Result<HttpResponse, AppError> {
    let updated = state
        .user_service
        .update_status(&actor, &path, body.into_inner())
        .await?;
    Ok(ApiResponse::ok(updated, "User status updated successfully"))
}

#[utoipa::path(
    delete,
    path = "/api/v1/users/{id}",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "User ObjectId")),
    responses(
        (status = 204, description = "User deleted"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Cannot delete last active Super Administrator or last active organisation owner")
    )
)]
pub async fn delete_user(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    state.user_service.delete(&actor, &path).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[utoipa::path(
    get,
    path = "/api/v1/users/{id}/roles",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "User ObjectId")),
    responses(
        (status = 200, description = "Assigned roles for user", body = ApiResponse<Vec<RoleSummary>>)
    )
)]
pub async fn get_user_roles(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let roles = state.user_service.get_roles(&actor, &path).await?;
    Ok(ApiResponse::ok(roles, "User roles retrieved successfully"))
}

#[utoipa::path(
    put,
    path = "/api/v1/users/{id}/roles",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "User ObjectId")),
    request_body = UpdateUserRolesRequest,
    responses(
        (status = 200, description = "User roles updated", body = ApiResponse<UserResponse>),
        (status = 403, description = "Privilege escalation or self-modification denied")
    )
)]
pub async fn update_user_roles(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdateUserRolesRequest>,
) -> Result<HttpResponse, AppError> {
    let updated = state
        .user_service
        .update_roles(&actor, &path, body.into_inner())
        .await?;
    Ok(ApiResponse::ok(updated, "User roles updated successfully"))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/{id}/permissions",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "User ObjectId")),
    responses(
        (status = 200, description = "Effective permissions for user", body = ApiResponse<Vec<PermissionResponse>>)
    )
)]
pub async fn get_user_permissions(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let perms = state
        .user_service
        .get_effective_permissions(&actor, &path)
        .await?;
    Ok(ApiResponse::ok(
        perms,
        "User effective permissions retrieved successfully",
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/{id}/organisations",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "User ObjectId")),
    responses(
        (status = 200, description = "Organisation memberships for user", body = ApiResponse<Vec<UserOrganisationSummary>>),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "User not found")
    )
)]
pub async fn get_user_organisations(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let orgs = state.user_service.get_organisations(&actor, &path).await?;
    Ok(ApiResponse::ok(
        orgs,
        "User organisations retrieved successfully",
    ))
}
