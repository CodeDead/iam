use actix_web::{HttpResponse, web};
use chrono::Utc;

use crate::{
    app::AppState,
    common::{errors::AppError, responses::ApiResponse},
    web::dto::health::HealthStatus,
};

#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    responses(
        (status = 200, description = "Service is alive", body = ApiResponse<HealthStatus>)
    )
)]
pub async fn liveness() -> HttpResponse {
    ApiResponse::ok(
        HealthStatus {
            status: "ok".to_string(),
            database: "not_checked".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        },
        "Service is healthy",
    )
}

#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "Health",
    responses(
        (status = 200, description = "Service and database are ready", body = ApiResponse<HealthStatus>),
        (status = 500, description = "Database unavailable")
    )
)]
pub async fn readiness(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    if let Some(ref db) = state.mongodb {
        db.ping().await?;
    }

    Ok(ApiResponse::ok(
        HealthStatus {
            status: "ready".to_string(),
            database: if state.mongodb.is_some() {
                "connected".to_string()
            } else {
                "in_memory".to_string()
            },
            timestamp: Utc::now().to_rfc3339(),
        },
        "Service is ready",
    ))
}
