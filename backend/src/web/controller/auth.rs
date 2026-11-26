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
        responses::{ApiResponse, EmptyData, ErrorResponse},
    },
    config::{Settings, settings::CookieSettings},
    middleware::{AuthenticatedUser, extract_client_ip, validate_csrf_double_submit},
    repository::auth::dao::refresh_token::RefreshTokenTransport,
    web::dto::auth::{
        AuthTokenResponse, LoginRequest, MeResponse, RefreshTokenRequest, RegisterRequest,
    },
};

pub const AUTH_TRANSPORT_HEADER: &str = "X-Auth-Transport";
const MAX_REFRESH_BODY_BYTES: usize = 4_096;
const MAX_REFRESH_TOKEN_LEN: usize = 512;

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "Authentication",
    params(
        ("X-Auth-Transport" = Option<String>, Header, description = "Optional refresh token transport: 'cookie' (default, browser HttpOnly cookie + CSRF) or 'body' (JSON response refresh_token for non-browser/desktop/WebView clients)")
    ),
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered successfully", body = ApiResponse<AuthTokenResponse>),
        (status = 400, description = "Invalid request payload or X-Auth-Transport header", body = ErrorResponse),
        (status = 403, description = "JSON-body refresh tokens disabled or browser origin forbidden", body = ErrorResponse),
        (status = 409, description = "Email or organisation already exists", body = ErrorResponse),
        (status = 422, description = "Validation error", body = ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = ErrorResponse)
    )
)]
pub async fn register(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RegisterRequest>,
) -> Result<HttpResponse, AppError> {
    let transport = parse_requested_transport(&req)?.unwrap_or(RefreshTokenTransport::Cookie);
    if transport == RefreshTokenTransport::Body {
        enforce_body_transport_policy(&req, &state.settings, true)?;
    }

    let (user_agent, ip) = extract_client_meta(&req, state.settings.rate_limit.trust_proxy_headers);
    let session = state
        .auth_service
        .register(body.into_inner(), transport, user_agent, ip)
        .await?;

    let mut response = HttpResponse::Created();
    if session.transport == RefreshTokenTransport::Cookie
        && let (Some(raw_refresh), Some(csrf)) = (&session.raw_refresh_token, &session.csrf_token)
    {
        attach_auth_cookies(
            &mut response,
            &state.settings.cookie,
            state.settings.jwt.refresh_token_ttl,
            raw_refresh,
            csrf,
        );
    }

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
    params(
        ("X-Auth-Transport" = Option<String>, Header, description = "Optional refresh token transport: 'cookie' (default, browser HttpOnly cookie + CSRF) or 'body' (JSON response refresh_token for non-browser/desktop/WebView clients)")
    ),
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Authenticated successfully", body = ApiResponse<AuthTokenResponse>),
        (status = 400, description = "Invalid request payload or X-Auth-Transport header", body = ErrorResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse),
        (status = 403, description = "Account disabled, locked, unverified, or body transport forbidden", body = ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = ErrorResponse)
    )
)]
pub async fn login(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, AppError> {
    let transport = parse_requested_transport(&req)?.unwrap_or(RefreshTokenTransport::Cookie);
    if transport == RefreshTokenTransport::Body {
        enforce_body_transport_policy(&req, &state.settings, true)?;
    }

    let (user_agent, ip) = extract_client_meta(&req, state.settings.rate_limit.trust_proxy_headers);
    let session = state
        .auth_service
        .login(body.into_inner(), transport, user_agent, ip)
        .await?;

    let mut response = HttpResponse::Ok();
    if session.transport == RefreshTokenTransport::Cookie
        && let (Some(raw_refresh), Some(csrf)) = (&session.raw_refresh_token, &session.csrf_token)
    {
        attach_auth_cookies(
            &mut response,
            &state.settings.cookie,
            state.settings.jwt.refresh_token_ttl,
            raw_refresh,
            csrf,
        );
    }

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
    params(
        ("X-CSRF-Token" = Option<String>, Header, description = "Double-submit CSRF token matching the csrf_token cookie (required when refreshing via HttpOnly refresh_token cookie)")
    ),
    request_body(
        content = Option<RefreshTokenRequest>,
        description = "Optional JSON body containing refresh_token for non-browser/desktop/WebView clients; omit or send {} when using the HttpOnly refresh_token cookie"
    ),
    responses(
        (status = 200, description = "Access token refreshed", body = ApiResponse<AuthTokenResponse>),
        (status = 400, description = "Ambiguous refresh token, mismatched transport, or invalid JSON payload", body = ErrorResponse),
        (status = 401, description = "Refresh token missing, invalid, expired, or reused", body = ErrorResponse),
        (status = 403, description = "CSRF validation failed, body transport disabled/forbidden, or account inactive", body = ErrorResponse)
    )
)]
pub async fn refresh(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Bytes,
) -> Result<HttpResponse, AppError> {
    let cookie_token = req
        .cookie(&state.settings.cookie.refresh_cookie_name)
        .map(|c| c.value().to_string());
    let body_token = parse_optional_refresh_body(&req, &body)?;
    let header_transport = parse_requested_transport(&req)?;

    if cookie_token.is_some() && body_token.is_some() {
        return Err(AppError::bad_request(
            "AMBIGUOUS_REFRESH_TOKEN",
            "Cannot supply refresh token in both cookie and request body",
        ));
    }

    let (user_agent, ip) = extract_client_meta(&req, state.settings.rate_limit.trust_proxy_headers);

    let session = if let Some(raw_refresh) = body_token {
        if header_transport == Some(RefreshTokenTransport::Cookie) {
            return Err(AppError::bad_request(
                "REFRESH_TOKEN_TRANSPORT_MISMATCH",
                "Cannot request cookie transport when authenticating with a body refresh token",
            ));
        }
        enforce_body_transport_policy(&req, &state.settings, true)?;
        state
            .auth_service
            .refresh(&raw_refresh, RefreshTokenTransport::Body, user_agent, ip)
            .await?
    } else if let Some(raw_refresh) = cookie_token {
        if header_transport == Some(RefreshTokenTransport::Body) {
            return Err(AppError::bad_request(
                "REFRESH_TOKEN_TRANSPORT_MISMATCH",
                "Cookie-authenticated sessions cannot switch to JSON-body refresh tokens",
            ));
        }
        validate_csrf_double_submit(&req, &state.settings.cookie)?;
        state
            .auth_service
            .refresh(&raw_refresh, RefreshTokenTransport::Cookie, user_agent, ip)
            .await?
    } else {
        return Err(AppError::unauthorized(
            "MISSING_REFRESH_TOKEN",
            "Refresh token cookie or request body is missing",
        ));
    };

    let mut response = HttpResponse::Ok();
    if session.transport == RefreshTokenTransport::Cookie
        && let (Some(raw_refresh), Some(csrf)) = (&session.raw_refresh_token, &session.csrf_token)
    {
        attach_auth_cookies(
            &mut response,
            &state.settings.cookie,
            state.settings.jwt.refresh_token_ttl,
            raw_refresh,
            csrf,
        );
    }

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
    params(
        ("X-CSRF-Token" = Option<String>, Header, description = "Double-submit CSRF token required when the HttpOnly refresh_token cookie is present")
    ),
    request_body(
        content = Option<RefreshTokenRequest>,
        description = "Optional JSON body containing refresh_token for non-browser/desktop/WebView clients"
    ),
    responses(
        (status = 200, description = "Signed out successfully", body = ApiResponse<EmptyData>),
        (status = 400, description = "Ambiguous refresh token, mismatched transport, or invalid JSON payload", body = ErrorResponse),
        (status = 403, description = "CSRF validation failed or browser origin forbidden", body = ErrorResponse)
    )
)]
pub async fn logout(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Bytes,
) -> Result<HttpResponse, AppError> {
    let cookie_token = req
        .cookie(&state.settings.cookie.refresh_cookie_name)
        .map(|c| c.value().to_string());
    let body_token = parse_optional_refresh_body(&req, &body)?;
    let header_transport = parse_requested_transport(&req)?;

    if cookie_token.is_some() && body_token.is_some() {
        return Err(AppError::bad_request(
            "AMBIGUOUS_REFRESH_TOKEN",
            "Cannot supply refresh token in both cookie and request body",
        ));
    }

    let mut response = HttpResponse::Ok();

    if let Some(raw_refresh) = body_token {
        if header_transport == Some(RefreshTokenTransport::Cookie) {
            return Err(AppError::bad_request(
                "REFRESH_TOKEN_TRANSPORT_MISMATCH",
                "Cannot request cookie transport when signing out with a body refresh token",
            ));
        }
        enforce_body_transport_policy(&req, &state.settings, false)?;
        state
            .auth_service
            .logout(Some(&raw_refresh), Some(RefreshTokenTransport::Body))
            .await?;
    } else if let Some(raw_refresh) = cookie_token {
        if header_transport == Some(RefreshTokenTransport::Body) {
            return Err(AppError::bad_request(
                "REFRESH_TOKEN_TRANSPORT_MISMATCH",
                "Cookie-authenticated sessions cannot use body transport for logout",
            ));
        }
        validate_csrf_double_submit(&req, &state.settings.cookie)?;
        state
            .auth_service
            .logout(Some(&raw_refresh), Some(RefreshTokenTransport::Cookie))
            .await?;
        clear_auth_cookies(&mut response, &state.settings.cookie);
    } else {
        state.auth_service.logout(None, None).await?;
        if header_transport != Some(RefreshTokenTransport::Body) {
            clear_auth_cookies(&mut response, &state.settings.cookie);
        }
    }

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

fn parse_requested_transport(req: &HttpRequest) -> Result<Option<RefreshTokenTransport>, AppError> {
    let Some(header_value) = req.headers().get(AUTH_TRANSPORT_HEADER) else {
        return Ok(None);
    };

    let raw = header_value.to_str().map_err(|_| {
        AppError::bad_request(
            "INVALID_AUTH_TRANSPORT",
            "X-Auth-Transport header must be 'cookie' or 'body'",
        )
    })?;

    match raw.trim().to_ascii_lowercase().as_str() {
        "cookie" => Ok(Some(RefreshTokenTransport::Cookie)),
        "body" => Ok(Some(RefreshTokenTransport::Body)),
        _ => Err(AppError::bad_request(
            "INVALID_AUTH_TRANSPORT",
            "X-Auth-Transport header must be 'cookie' or 'body'",
        )),
    }
}

fn normalize_origin(origin: &str) -> String {
    origin.trim().trim_end_matches('/').to_ascii_lowercase()
}

fn enforce_body_transport_policy(
    req: &HttpRequest,
    settings: &Settings,
    require_enabled: bool,
) -> Result<(), AppError> {
    if require_enabled && !settings.app.allow_body_refresh_tokens {
        return Err(AppError::forbidden(
            "BODY_REFRESH_TOKENS_DISABLED",
            "JSON-body refresh tokens are disabled on this server",
        ));
    }

    if let Some(origin_header) = req.headers().get(header::ORIGIN) {
        let raw_origin = origin_header.to_str().map_err(|_| {
            AppError::forbidden(
                "BROWSER_ORIGIN_FORBIDDEN",
                "Invalid Origin header for JSON-body refresh token request",
            )
        })?;
        let origin = normalize_origin(raw_origin);

        let is_allowed_body_origin = settings
            .server
            .allowed_body_token_origins
            .iter()
            .any(|allowed| normalize_origin(allowed) == origin);
        if is_allowed_body_origin {
            return Ok(());
        }

        let is_web_origin = normalize_origin(&settings.server.frontend_url) == origin
            || settings
                .server
                .allowed_origins
                .iter()
                .any(|allowed| normalize_origin(allowed) == origin);
        if is_web_origin {
            return Err(AppError::forbidden(
                "BROWSER_ORIGIN_FORBIDDEN",
                "Web browser origins must use cookie-based refresh tokens",
            ));
        }

        if settings.app.reject_browser_origin_for_body_tokens {
            return Err(AppError::forbidden(
                "BROWSER_ORIGIN_FORBIDDEN",
                "Origin is not allowed to use JSON-body refresh tokens; configure ALLOWED_BODY_TOKEN_ORIGINS for WebView clients",
            ));
        }
    } else if settings.app.reject_browser_origin_for_body_tokens
        && req.headers().contains_key("sec-fetch-site")
    {
        return Err(AppError::forbidden(
            "BROWSER_ORIGIN_FORBIDDEN",
            "Browser fetch requests without an allowlisted body-token origin must use cookie-based refresh tokens",
        ));
    }

    Ok(())
}

fn parse_optional_refresh_body(req: &HttpRequest, body: &[u8]) -> Result<Option<String>, AppError> {
    if body.len() > MAX_REFRESH_BODY_BYTES {
        return Err(AppError::bad_request(
            "INVALID_JSON_PAYLOAD",
            "Refresh token request body is too large",
        ));
    }

    if body.trim_ascii().is_empty() {
        return Ok(None);
    }

    let is_json_content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"));

    if !is_json_content_type {
        return Err(AppError::bad_request(
            "INVALID_CONTENT_TYPE",
            "Content-Type must be application/json when supplying a request body",
        ));
    }

    let parsed: RefreshTokenRequest = serde_json::from_slice(body).map_err(|err| {
        AppError::bad_request(
            "INVALID_JSON_PAYLOAD",
            format!("Invalid JSON payload: {err}"),
        )
    })?;

    if let Some(ref token) = parsed.refresh_token
        && token.len() > MAX_REFRESH_TOKEN_LEN
    {
        return Err(AppError::bad_request(
            "INVALID_JSON_PAYLOAD",
            "Refresh token exceeds maximum allowed length",
        ));
    }

    Ok(parsed.refresh_token)
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
