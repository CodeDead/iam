use actix_web::{HttpResponse, web};

use crate::{
    app::AppState,
    common::{
        errors::AppError,
        pagination::PaginatedResponse,
        responses::{ApiResponse, EmptyData},
    },
    middleware::AuthenticatedUser,
    web::dto::organisations::{
        AddOrganisationMemberRequest, CreateOrganisationRequest, ListOrganisationsQuery,
        OrganisationResponse, UpdateOrganisationMemberRequest, UpdateOrganisationRequest,
        UserOrganisationSummary,
    },
};

#[utoipa::path(
    get,
    path = "/api/v1/organisations/me",
    tag = "Organisations",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Organisations the current user belongs to", body = ApiResponse<Vec<UserOrganisationSummary>>)
    )
)]
pub async fn list_my_organisations(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let orgs = state
        .organisation_service
        .list_my_organisations(&actor)
        .await?;
    Ok(ApiResponse::ok(
        orgs,
        "Your organisations retrieved successfully",
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/organisations",
    tag = "Organisations",
    security(("bearer_auth" = [])),
    params(ListOrganisationsQuery),
    responses(
        (status = 200, description = "Paginated list of organisations", body = ApiResponse<PaginatedResponse<OrganisationResponse>>)
    )
)]
pub async fn list_organisations(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    query: web::Query<ListOrganisationsQuery>,
) -> Result<HttpResponse, AppError> {
    let data = state
        .organisation_service
        .list(&actor, query.into_inner())
        .await?;
    Ok(ApiResponse::ok(
        data,
        "Organisations retrieved successfully",
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/organisations",
    tag = "Organisations",
    security(("bearer_auth" = [])),
    request_body = CreateOrganisationRequest,
    responses(
        (status = 201, description = "Organisation created", body = ApiResponse<OrganisationResponse>),
        (status = 403, description = "Privilege escalation denied"),
        (status = 404, description = "Initial owner or member not found"),
        (status = 409, description = "Organisation already exists or self-service limit reached"),
        (status = 422, description = "Validation error or inactive initial owner")
    )
)]
pub async fn create_organisation(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    body: web::Json<CreateOrganisationRequest>,
) -> Result<HttpResponse, AppError> {
    let created = state
        .organisation_service
        .create(&actor, body.into_inner())
        .await?;
    Ok(ApiResponse::created(
        created,
        "Organisation created successfully",
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/organisations/{id}",
    tag = "Organisations",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Organisation ID")),
    responses(
        (status = 200, description = "Organisation details (privacy-filtered member roster)", body = ApiResponse<OrganisationResponse>),
        (status = 403, description = "Not authorized to inspect this organisation"),
        (status = 404, description = "Organisation not found")
    )
)]
pub async fn get_organisation(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let org = state
        .organisation_service
        .get_by_id(&actor, &path.into_inner())
        .await?;
    Ok(ApiResponse::ok(org, "Organisation retrieved successfully"))
}

#[utoipa::path(
    put,
    path = "/api/v1/organisations/{id}",
    tag = "Organisations",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Organisation ID")),
    request_body = UpdateOrganisationRequest,
    responses(
        (status = 200, description = "Organisation updated", body = ApiResponse<OrganisationResponse>),
        (status = 403, description = "Not authorized to update this organisation"),
        (status = 404, description = "Organisation not found"),
        (status = 409, description = "Organisation name conflict or concurrent modification"),
        (status = 422, description = "Validation error")
    )
)]
pub async fn update_organisation(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdateOrganisationRequest>,
) -> Result<HttpResponse, AppError> {
    let updated = state
        .organisation_service
        .update(&actor, &path.into_inner(), body.into_inner())
        .await?;
    Ok(ApiResponse::ok(
        updated,
        "Organisation updated successfully",
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/organisations/{id}",
    tag = "Organisations",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Organisation ID")),
    responses(
        (status = 204, description = "Organisation deleted"),
        (status = 403, description = "Not authorized to delete this organisation"),
        (status = 404, description = "Organisation not found")
    )
)]
pub async fn delete_organisation(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    state
        .organisation_service
        .delete(&actor, &path.into_inner())
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

#[utoipa::path(
    post,
    path = "/api/v1/organisations/{id}/members",
    tag = "Organisations",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Organisation ID")),
    request_body = AddOrganisationMemberRequest,
    responses(
        (status = 200, description = "Member added or invited to organisation", body = ApiResponse<OrganisationResponse>),
        (status = 403, description = "Not authorized or email required for self-service invite"),
        (status = 404, description = "Organisation or active user not found"),
        (status = 409, description = "User is already a member, pending invite limit reached, or concurrent modification"),
        (status = 422, description = "Validation error")
    )
)]
pub async fn add_organisation_member(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<AddOrganisationMemberRequest>,
) -> Result<HttpResponse, AppError> {
    let updated = state
        .organisation_service
        .add_member(&actor, &path.into_inner(), body.into_inner())
        .await?;
    Ok(ApiResponse::ok(
        updated,
        "Organisation member added successfully",
    ))
}

#[utoipa::path(
    patch,
    path = "/api/v1/organisations/{id}/members/{user_id}",
    tag = "Organisations",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Organisation ID"),
        ("user_id" = String, Path, description = "Member user ID")
    ),
    request_body = UpdateOrganisationMemberRequest,
    responses(
        (status = 200, description = "Member role or status updated", body = ApiResponse<OrganisationResponse>),
        (status = 403, description = "Forbidden role or status transition"),
        (status = 404, description = "Organisation or member not found"),
        (status = 409, description = "Cannot demote/deactivate last active owner or concurrent modification"),
        (status = 422, description = "Validation error")
    )
)]
pub async fn update_organisation_member(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    body: web::Json<UpdateOrganisationMemberRequest>,
) -> Result<HttpResponse, AppError> {
    let (id, user_id) = path.into_inner();
    let updated = state
        .organisation_service
        .update_member(&actor, &id, &user_id, body.into_inner())
        .await?;
    Ok(ApiResponse::ok(
        updated,
        "Organisation member updated successfully",
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/organisations/{id}/members/{user_id}",
    tag = "Organisations",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Organisation ID"),
        ("user_id" = String, Path, description = "Member user ID")
    ),
    responses(
        (status = 200, description = "Member removed from organisation", body = ApiResponse<OrganisationResponse>),
        (status = 403, description = "Not authorized to remove this member"),
        (status = 404, description = "Organisation or member not found"),
        (status = 409, description = "Cannot remove last active owner or concurrent modification")
    )
)]
pub async fn remove_organisation_member(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, AppError> {
    let (id, user_id) = path.into_inner();
    let updated = state
        .organisation_service
        .remove_member(&actor, &id, &user_id)
        .await?;
    Ok(ApiResponse::ok(
        updated,
        "Organisation member removed successfully",
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/organisations/{id}/accept",
    tag = "Organisations",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Organisation ID")),
    responses(
        (status = 200, description = "Invitation accepted", body = ApiResponse<UserOrganisationSummary>),
        (status = 403, description = "No invitation to this organisation"),
        (status = 409, description = "Invitation is not pending or concurrent modification")
    )
)]
pub async fn accept_organisation_invitation(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let summary = state
        .organisation_service
        .accept_invitation(&actor, &path.into_inner())
        .await?;
    Ok(ApiResponse::ok(
        summary,
        "Organisation invitation accepted successfully",
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/organisations/{id}/leave",
    tag = "Organisations",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Organisation ID")),
    responses(
        (status = 200, description = "Left organisation or declined invitation", body = ApiResponse<EmptyData>),
        (status = 403, description = "Not a member of this organisation"),
        (status = 409, description = "Cannot leave as last active owner or concurrent modification")
    )
)]
pub async fn leave_organisation(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    state
        .organisation_service
        .leave_organisation(&actor, &path.into_inner())
        .await?;
    Ok(ApiResponse::ok(
        EmptyData {},
        "Left organisation successfully",
    ))
}
