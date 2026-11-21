use actix_web::{HttpResponse, web};

use crate::{
    app::AppState,
    common::{errors::AppError, responses::ApiResponse},
    middleware::AuthenticatedUser,
    web::dto::dashboard::DashboardStatsResponse,
};

#[utoipa::path(
    get,
    path = "/api/v1/dashboard/stats",
    tag = "Dashboard",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Authorized dashboard statistics", body = ApiResponse<DashboardStatsResponse>)
    )
)]
pub async fn get_dashboard_stats(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let stats = state.dashboard_service.get_stats(&actor).await?;
    Ok(ApiResponse::ok(
        stats,
        "Dashboard statistics retrieved successfully",
    ))
}
