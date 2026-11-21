use actix_web::HttpRequest;

use crate::{
    common::errors::AppError, config::settings::CookieSettings, security::csrf::constant_time_eq,
};

pub const CSRF_HEADER_NAME: &str = "X-CSRF-Token";

pub fn validate_csrf_double_submit(
    req: &HttpRequest,
    cookie_settings: &CookieSettings,
) -> Result<(), AppError> {
    let cookie_val = req
        .cookie(&cookie_settings.csrf_cookie_name)
        .map(|c| c.value().to_string())
        .ok_or_else(|| {
            AppError::forbidden(
                "CSRF_TOKEN_MISSING",
                "Missing CSRF cookie for cookie-authenticated request",
            )
        })?;

    let header_val = req
        .headers()
        .get(CSRF_HEADER_NAME)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            AppError::forbidden(
                "CSRF_HEADER_MISSING",
                "Missing X-CSRF-Token header for cookie-authenticated request",
            )
        })?;

    if cookie_val.is_empty() || !constant_time_eq(&cookie_val, header_val) {
        return Err(AppError::forbidden(
            "CSRF_TOKEN_MISMATCH",
            "CSRF validation failed",
        ));
    }

    Ok(())
}
