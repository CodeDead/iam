#![allow(clippy::unwrap_used, clippy::expect_used)]

use actix_web::{App, cookie::Cookie, http::StatusCode, test, web};
use chrono::{Duration, Utc};
use mongodb::bson::oid::ObjectId;
use serde_json::{Value, json};

use auth_backend::{
    app::configure_app_services, common::pagination::PaginationParams,
    database::mongodb::chrono_to_bson_dt, middleware::RequestIdAndSecurityHeaders,
    repository::auth::dao::refresh_token::RefreshToken, security::csrf::sha256_hex,
    test_support::create_test_app_state,
};

fn extract_cookie(resp: &actix_web::dev::ServiceResponse, name: &str) -> Option<String> {
    resp.response()
        .cookies()
        .find(|c| c.name() == name)
        .map(|c| c.value().to_string())
}

#[actix_web::test]
async fn registration_login_me_refresh_rotation_reuse_detection_and_logout() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new()
            .wrap(RequestIdAndSecurityHeaders)
            .configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // 1. Register regular user
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(json!({
            "first_name": "Alice",
            "last_name": "Walker",
            "email": "Alice.Walker@Example.com",
            "password": "StrongPassword123!",
            "password_confirmation": "StrongPassword123!"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(resp.headers().contains_key("x-request-id"));
    assert_eq!(
        resp.headers()
            .get("x-content-type-options")
            .unwrap()
            .to_str()
            .unwrap(),
        "nosniff"
    );

    let refresh_cookie_1 = extract_cookie(&resp, "refresh_token").unwrap();
    let csrf_cookie_1 = extract_cookie(&resp, "csrf_token").unwrap();
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["user"]["email"], "alice.walker@example.com");
    assert!(body["data"]["user"].get("password_hash").is_none());

    let access_token_1 = body["data"]["access_token"].as_str().unwrap().to_string();
    let roles = body["data"]["user"]["roles"].as_array().unwrap();
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0]["name"], "User");

    // 2. Duplicate registration returns 409 Conflict
    let dup_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(json!({
            "first_name": "Alice",
            "last_name": "Walker",
            "email": "alice.walker@example.com",
            "password": "StrongPassword123!",
            "password_confirmation": "StrongPassword123!"
        }))
        .to_request();
    let dup_resp = test::call_service(&app, dup_req).await;
    assert_eq!(dup_resp.status(), StatusCode::CONFLICT);

    // 3. Mass assignment attempt during registration is rejected with 400
    let mass_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(json!({
            "first_name": "Evil",
            "last_name": "User",
            "email": "evil@example.com",
            "password": "StrongPassword123!",
            "password_confirmation": "StrongPassword123!",
            "roles": ["Super Administrator"]
        }))
        .to_request();
    let mass_resp = test::call_service(&app, mass_req).await;
    assert_eq!(mass_resp.status(), StatusCode::BAD_REQUEST);

    // 4. Unauthenticated request to /auth/me returns 401
    let unauth_req = test::TestRequest::get().uri("/api/v1/auth/me").to_request();
    let unauth_resp = test::call_service(&app, unauth_req).await;
    assert_eq!(unauth_resp.status(), StatusCode::UNAUTHORIZED);

    // 5. Authenticated /auth/me returns user and permissions
    let me_req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Authorization", format!("Bearer {access_token_1}")))
        .to_request();
    let me_resp = test::call_service(&app, me_req).await;
    assert_eq!(me_resp.status(), StatusCode::OK);
    let me_body: Value = test::read_body_json(me_resp).await;
    assert_eq!(me_body["data"]["user"]["email"], "alice.walker@example.com");

    // 5b. Refresh with no refresh cookie at all returns 401 MISSING_REFRESH_TOKEN (not 403 CSRF)
    let no_cookie_refresh = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .to_request(),
    )
    .await;
    assert_eq!(no_cookie_refresh.status(), StatusCode::UNAUTHORIZED);
    let no_cookie_body: Value = test::read_body_json(no_cookie_refresh).await;
    assert_eq!(no_cookie_body["error"]["code"], "MISSING_REFRESH_TOKEN");

    // 6. Refresh without CSRF header fails with 403
    let csrf_fail_req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .cookie(Cookie::new("refresh_token", refresh_cookie_1.clone()))
        .cookie(Cookie::new("csrf_token", csrf_cookie_1.clone()))
        .to_request();
    let csrf_fail_resp = test::call_service(&app, csrf_fail_req).await;
    assert_eq!(csrf_fail_resp.status(), StatusCode::FORBIDDEN);

    // 7. Refresh with valid cookie + matching CSRF header rotates token
    let refresh_req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .cookie(Cookie::new("refresh_token", refresh_cookie_1.clone()))
        .cookie(Cookie::new("csrf_token", csrf_cookie_1.clone()))
        .insert_header(("X-CSRF-Token", csrf_cookie_1.clone()))
        .to_request();
    let refresh_resp = test::call_service(&app, refresh_req).await;
    assert_eq!(refresh_resp.status(), StatusCode::OK);
    let refresh_cookie_2 = extract_cookie(&refresh_resp, "refresh_token").unwrap();
    let csrf_cookie_2 = extract_cookie(&refresh_resp, "csrf_token").unwrap();
    assert_ne!(refresh_cookie_1, refresh_cookie_2);

    // 8. Reusing rotated refresh_cookie_1 fails with 401 AND revokes the entire token family!
    let reuse_req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .cookie(Cookie::new("refresh_token", refresh_cookie_1.clone()))
        .cookie(Cookie::new("csrf_token", csrf_cookie_2.clone()))
        .insert_header(("X-CSRF-Token", csrf_cookie_2.clone()))
        .to_request();
    let reuse_resp = test::call_service(&app, reuse_req).await;
    assert_eq!(reuse_resp.status(), StatusCode::UNAUTHORIZED);
    let reuse_body: Value = test::read_body_json(reuse_resp).await;
    assert_eq!(reuse_body["error"]["code"], "REFRESH_TOKEN_REUSED");

    // Because family was revoked on reuse, refresh_cookie_2 must also now be rejected!
    let revoked_family_req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .cookie(Cookie::new("refresh_token", refresh_cookie_2))
        .cookie(Cookie::new("csrf_token", csrf_cookie_2.clone()))
        .insert_header(("X-CSRF-Token", csrf_cookie_2))
        .to_request();
    let revoked_family_resp = test::call_service(&app, revoked_family_req).await;
    assert_eq!(revoked_family_resp.status(), StatusCode::UNAUTHORIZED);

    // 9. Login with valid credentials, then logout revokes refresh token
    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({
            "email": "alice.walker@example.com",
            "password": "StrongPassword123!"
        }))
        .to_request();
    let login_resp = test::call_service(&app, login_req).await;
    assert_eq!(login_resp.status(), StatusCode::OK);
    let login_refresh = extract_cookie(&login_resp, "refresh_token").unwrap();
    let login_csrf = extract_cookie(&login_resp, "csrf_token").unwrap();

    let logout_req = test::TestRequest::post()
        .uri("/api/v1/auth/logout")
        .cookie(Cookie::new("refresh_token", login_refresh.clone()))
        .cookie(Cookie::new("csrf_token", login_csrf.clone()))
        .insert_header(("X-CSRF-Token", login_csrf.clone()))
        .to_request();
    let logout_resp = test::call_service(&app, logout_req).await;
    assert_eq!(logout_resp.status(), StatusCode::OK);

    // Refresh after logout is rejected
    let after_logout_req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .cookie(Cookie::new("refresh_token", login_refresh))
        .cookie(Cookie::new("csrf_token", login_csrf.clone()))
        .insert_header(("X-CSRF-Token", login_csrf))
        .to_request();
    let after_logout_resp = test::call_service(&app, after_logout_req).await;
    assert_eq!(after_logout_resp.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn expired_and_invalid_refresh_tokens_are_rejected() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    let raw_expired = "expired-refresh-token-value-123456";
    let expired_record = RefreshToken {
        id: ObjectId::new(),
        user_id: ObjectId::new(),
        token_hash: sha256_hex(raw_expired),
        family_id: "expired-family".to_string(),
        expires_at: chrono_to_bson_dt(Utc::now() - Duration::minutes(5)),
        revoked_at: None,
        replaced_by_hash: None,
        user_agent: None,
        ip_address: None,
        created_at: chrono_to_bson_dt(Utc::now() - Duration::hours(1)),
    };
    state
        .refresh_token_repo
        .create(&expired_record)
        .await
        .unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .cookie(Cookie::new("refresh_token", raw_expired))
        .cookie(Cookie::new("csrf_token", "csrf-1"))
        .insert_header(("X-CSRF-Token", "csrf-1"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["error"]["code"], "REFRESH_TOKEN_EXPIRED");

    // Unknown/invalid token
    let invalid_req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .cookie(Cookie::new("refresh_token", "totally-fake-token"))
        .cookie(Cookie::new("csrf_token", "csrf-1"))
        .insert_header(("X-CSRF-Token", "csrf-1"))
        .to_request();
    let invalid_resp = test::call_service(&app, invalid_req).await;
    assert_eq!(invalid_resp.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn health_and_openapi_endpoints_work() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state))),
    )
    .await;

    let health_resp =
        test::call_service(&app, test::TestRequest::get().uri("/health").to_request()).await;
    assert_eq!(health_resp.status(), StatusCode::OK);

    let ready_resp = test::call_service(
        &app,
        test::TestRequest::get().uri("/health/ready").to_request(),
    )
    .await;
    assert_eq!(ready_resp.status(), StatusCode::OK);

    let openapi_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/openapi.json")
            .to_request(),
    )
    .await;
    assert_eq!(openapi_resp.status(), StatusCode::OK);
    let doc: Value = test::read_body_json(openapi_resp).await;
    assert_eq!(doc["info"]["version"], "1.0.0");
    assert!(doc["paths"].get("/api/v1/users").is_some());
}

#[actix_web::test]
async fn production_hides_swagger_failed_logins_are_audited_and_api_rate_limit_applies() {
    // Verify pagination skip never overflows for extreme page values
    let extreme_page = PaginationParams {
        page: Some(u64::MAX),
        page_size: Some(u64::MAX),
        sort_by: None,
        sort_order: None,
    };
    assert!(extreme_page.skip() <= 10_000_000);

    // 1. Production mode disables OpenAPI / Swagger UI by default and enforces API rate limit
    let mut state = create_test_app_state().await.unwrap();
    let mut prod_settings = (*state.settings).clone();
    prod_settings.app.environment = "production".to_string();
    prod_settings.app.enable_swagger_ui = None;
    prod_settings.rate_limit.enabled = true;
    prod_settings.rate_limit.api_requests_per_minute = 3;
    prod_settings.rate_limit.auth_requests_per_minute = 10;
    state.settings = std::sync::Arc::new(prod_settings);

    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    let openapi_prod = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/openapi.json")
            .to_request(),
    )
    .await;
    assert_eq!(openapi_prod.status(), StatusCode::NOT_FOUND);

    // 2. Failed login with unknown email & wrong password both return 401 and record auth.login_failed
    let unknown_login = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({
                "email": "ghost@test.local",
                "password": "WrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(unknown_login.status(), StatusCode::UNAUTHORIZED);

    let wrong_pw_login = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({
                "email": "superadmin@test.local",
                "password": "WrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(wrong_pw_login.status(), StatusCode::UNAUTHORIZED);

    // 3. 4th request to /api/v1 within 1 minute hits API rate limit (429 TOO_MANY_REQUESTS)
    let rate_limited = test::call_service(
        &app,
        test::TestRequest::get().uri("/api/v1/users").to_request(),
    )
    .await;
    assert_eq!(rate_limited.status(), StatusCode::TOO_MANY_REQUESTS);

    // Verify both failed login attempts were persisted in audit_logs
    let (entries, total) = state
        .audit_repo
        .find_all(
            &auth_backend::repository::audit::audit_log_repository::AuditLogFilter {
                action: Some("auth.login_failed".to_string()),
                limit: 10,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(total, 2);
    assert_eq!(entries.len(), 2);
}

#[actix_web::test]
async fn cf_connecting_ip_isolates_rate_limit_buckets_behind_cloudflared_and_traefik() {
    let mut state = create_test_app_state().await.unwrap();
    let mut settings = (*state.settings).clone();
    settings.rate_limit.enabled = true;
    settings.rate_limit.trust_proxy_headers = true;
    settings.rate_limit.api_requests_per_minute = 2;
    settings.rate_limit.auth_requests_per_minute = 10;
    state.settings = std::sync::Arc::new(settings);

    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // Visitor A makes 2 requests behind the same Traefik/cloudflared X-Forwarded-For IP (10.0.3.13)
    for _ in 0..2 {
        let resp = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/users")
                .insert_header(("X-Forwarded-For", "10.0.3.13"))
                .insert_header(("CF-Connecting-IP", "203.0.113.10"))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // Visitor A's 3rd request is rate limited (429)
    let visitor_a_limited = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/users")
            .insert_header(("X-Forwarded-For", "10.0.3.13"))
            .insert_header(("CF-Connecting-IP", "203.0.113.10"))
            .to_request(),
    )
    .await;
    assert_eq!(visitor_a_limited.status(), StatusCode::TOO_MANY_REQUESTS);

    // Visitor B (different CF-Connecting-IP, same internal X-Forwarded-For) is NOT affected
    let visitor_b_ok = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("X-Forwarded-For", "10.0.3.13"))
            .insert_header(("CF-Connecting-IP", "198.51.100.25"))
            .set_json(json!({
                "email": "superadmin@test.local",
                "password": "SuperAdmin123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(visitor_b_ok.status(), StatusCode::OK);

    // Verify audit log recorded Visitor B's real CF-Connecting-IP, not 10.0.3.13
    let (entries, _) = state
        .audit_repo
        .find_all(
            &auth_backend::repository::audit::audit_log_repository::AuditLogFilter {
                action: Some("auth.login".to_string()),
                limit: 5,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        entries[0].details.as_ref().unwrap()["ip_address"],
        "198.51.100.25"
    );
}
