use std::collections::HashMap;

use actix_web::HttpResponse;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
    pub message: String,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T, message: impl Into<String>) -> HttpResponse {
        HttpResponse::Ok().json(Self {
            success: true,
            data,
            message: message.into(),
        })
    }

    pub fn created(data: T, message: impl Into<String>) -> HttpResponse {
        HttpResponse::Created().json(Self {
            success: true,
            data,
            message: message.into(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: ErrorBody,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EmptyData {}
