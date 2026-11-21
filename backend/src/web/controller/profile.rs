use actix_web::{HttpResponse, web};

use crate::{
    app::AppState,
    common::{
        errors::AppError,
        responses::{ApiResponse, EmptyData},
    },
    middleware::AuthenticatedUser,
    web::dto::{
        profile::{ChangePasswordRequest, UpdateProfileRequest},
        users::UserResponse,
    },
};

#[utoipa::path(
    get,
    path = "/api/v1/profile",
    tag = "Profile",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Current user profile", body = ApiResponse<UserResponse>)
    )
)]
pub async fn get_profile(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let profile = state.profile_service.get_profile(&actor).await?;
    Ok(ApiResponse::ok(profile, "Profile retrieved successfully"))
}

#[utoipa::path(
    put,
    path = "/api/v1/profile",
    tag = "Profile",
    security(("bearer_auth" = [])),
    request_body = UpdateProfileRequest,
    responses(
        (status = 200, description = "Profile updated", body = ApiResponse<UserResponse>)
    )
)]
pub async fn update_profile(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    body: web::Json<UpdateProfileRequest>,
) -> Result<HttpResponse, AppError> {
    let updated = state
        .profile_service
        .update_profile(&actor, body.into_inner())
        .await?;
    Ok(ApiResponse::ok(updated, "Profile updated successfully"))
}

#[utoipa::path(
    patch,
    path = "/api/v1/profile/password",
    tag = "Profile",
    security(("bearer_auth" = [])),
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed successfully", body = ApiResponse<EmptyData>)
    )
)]
pub async fn change_password(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    body: web::Json<ChangePasswordRequest>,
) -> Result<HttpResponse, AppError> {
    state
        .profile_service
        .change_password(&actor, body.into_inner())
        .await?;
    Ok(ApiResponse::ok(
        EmptyData {},
        "Password changed successfully; all existing sessions have been revoked",
    ))
}
