use actix_web::{
    HttpRequest, HttpResponse,
    cookie::{Cookie, SameSite, time::Duration as CookieDuration},
    http::header,
    web,
};

use crate::{
    app::AppState,
    common::{
        errors::AppError,
        responses::{ApiResponse, EmptyData},
    },
    config::settings::CookieSettings,
    middleware::{AuthenticatedUser, extract_client_ip, validate_csrf_double_submit},
    web::dto::auth::{AuthTokenResponse, LoginRequest, MeResponse, RegisterRequest},
};

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "Authentication",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered successfully", body = ApiResponse<AuthTokenResponse>),
        (status = 409, description = "Email already exists"),
        (status = 422, description = "Validation error")
    )
)]
pub async fn register(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RegisterRequest>,
) -> Result<HttpResponse, AppError> {
    let (user_agent, ip) = extract_client_meta(&req, state.settings.rate_limit.trust_proxy_headers);
    let session = state
        .auth_service
        .register(body.into_inner(), user_agent, ip)
        .await?;

    let mut response = HttpResponse::Created();
    attach_auth_cookies(
        &mut response,
        &state.settings.cookie,
        state.settings.jwt.refresh_token_ttl,
        &session.raw_refresh_token,
        &session.csrf_token,
    );

    Ok(response.json(ApiResponse {
        success: true,
        data: session.response,
        message: "Account registered successfully".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "Authentication",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Authenticated successfully", body = ApiResponse<AuthTokenResponse>),
        (status = 401, description = "Invalid credentials"),
        (status = 403, description = "Account disabled or locked")
    )
)]
pub async fn login(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, AppError> {
    let (user_agent, ip) = extract_client_meta(&req, state.settings.rate_limit.trust_proxy_headers);
    let session = state
        .auth_service
        .login(body.into_inner(), user_agent, ip)
        .await?;

    let mut response = HttpResponse::Ok();
    attach_auth_cookies(
        &mut response,
        &state.settings.cookie,
        state.settings.jwt.refresh_token_ttl,
        &session.raw_refresh_token,
        &session.csrf_token,
    );

    Ok(response.json(ApiResponse {
        success: true,
        data: session.response,
        message: "Signed in successfully".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "Authentication",
    responses(
        (status = 200, description = "Access token refreshed", body = ApiResponse<AuthTokenResponse>),
        (status = 401, description = "Refresh token invalid, expired, or reused"),
        (status = 403, description = "CSRF validation failed or account inactive")
    )
)]
pub async fn refresh(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let raw_refresh = req
        .cookie(&state.settings.cookie.refresh_cookie_name)
        .map(|c| c.value().to_string())
        .ok_or_else(|| {
            AppError::unauthorized("MISSING_REFRESH_TOKEN", "Refresh token cookie is missing")
        })?;

    validate_csrf_double_submit(&req, &state.settings.cookie)?;

    let (user_agent, ip) = extract_client_meta(&req, state.settings.rate_limit.trust_proxy_headers);
    let session = state
        .auth_service
        .refresh(&raw_refresh, user_agent, ip)
        .await?;

    let mut response = HttpResponse::Ok();
    attach_auth_cookies(
        &mut response,
        &state.settings.cookie,
        state.settings.jwt.refresh_token_ttl,
        &session.raw_refresh_token,
        &session.csrf_token,
    );

    Ok(response.json(ApiResponse {
        success: true,
        data: session.response,
        message: "Session refreshed successfully".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    tag = "Authentication",
    responses(
        (status = 200, description = "Signed out successfully", body = ApiResponse<EmptyData>),
        (status = 403, description = "CSRF validation failed")
    )
)]
pub async fn logout(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if req
        .cookie(&state.settings.cookie.refresh_cookie_name)
        .is_some()
    {
        validate_csrf_double_submit(&req, &state.settings.cookie)?;
    }

    let raw_refresh = req
        .cookie(&state.settings.cookie.refresh_cookie_name)
        .map(|c| c.value().to_string());

    state.auth_service.logout(raw_refresh.as_deref()).await?;

    let mut response = HttpResponse::Ok();
    clear_auth_cookies(&mut response, &state.settings.cookie);

    Ok(response.json(ApiResponse {
        success: true,
        data: EmptyData {},
        message: "Signed out successfully".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    tag = "Authentication",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Current authenticated user and effective permissions", body = ApiResponse<MeResponse>),
        (status = 401, description = "Unauthenticated")
    )
)]
pub async fn me(
    actor: AuthenticatedUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let data = state.auth_service.me(&actor).await?;
    Ok(ApiResponse::ok(
        data,
        "Authenticated user retrieved successfully",
    ))
}

fn extract_client_meta(
    req: &HttpRequest,
    trust_proxy_headers: bool,
) -> (Option<String>, Option<String>) {
    let user_agent = req
        .headers()
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(256).collect());
    let ip = extract_client_ip(req, trust_proxy_headers);
    (user_agent, ip)
}

fn parse_same_site(value: &str) -> SameSite {
    match value.to_ascii_lowercase().as_str() {
        "strict" => SameSite::Strict,
        "none" => SameSite::None,
        _ => SameSite::Lax,
    }
}

fn attach_auth_cookies(
    response: &mut actix_web::HttpResponseBuilder,
    settings: &CookieSettings,
    refresh_ttl_seconds: i64,
    raw_refresh: &str,
    csrf_token: &str,
) {
    let same_site = parse_same_site(&settings.same_site);
    let max_age = CookieDuration::seconds(refresh_ttl_seconds);

    let mut refresh_builder = Cookie::build(
        settings.refresh_cookie_name.clone(),
        raw_refresh.to_string(),
    )
    .http_only(true)
    .secure(settings.secure)
    .same_site(same_site)
    .path("/api/v1/auth")
    .max_age(max_age);

    let mut csrf_builder = Cookie::build(settings.csrf_cookie_name.clone(), csrf_token.to_string())
        .http_only(false)
        .secure(settings.secure)
        .same_site(same_site)
        .path("/")
        .max_age(max_age);

    if !settings.domain.trim().is_empty() {
        refresh_builder = refresh_builder.domain(settings.domain.clone());
        csrf_builder = csrf_builder.domain(settings.domain.clone());
    }

    response.cookie(refresh_builder.finish());
    response.cookie(csrf_builder.finish());
}

pub(crate) fn clear_auth_cookies(
    response: &mut actix_web::HttpResponseBuilder,
    settings: &CookieSettings,
) {
    let same_site = parse_same_site(&settings.same_site);
    let mut refresh_cookie = Cookie::build(settings.refresh_cookie_name.clone(), "")
        .http_only(true)
        .secure(settings.secure)
        .same_site(same_site)
        .path("/api/v1/auth")
        .max_age(CookieDuration::ZERO);

    let mut csrf_cookie = Cookie::build(settings.csrf_cookie_name.clone(), "")
        .http_only(false)
        .secure(settings.secure)
        .same_site(same_site)
        .path("/")
        .max_age(CookieDuration::ZERO);

    if !settings.domain.trim().is_empty() {
        refresh_cookie = refresh_cookie.domain(settings.domain.clone());
        csrf_cookie = csrf_cookie.domain(settings.domain.clone());
    }

    response.cookie(refresh_cookie.finish());
    response.cookie(csrf_cookie.finish());
}
