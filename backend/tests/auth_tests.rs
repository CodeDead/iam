#![allow(clippy::unwrap_used, clippy::expect_used)]

use actix_web::{App, cookie::Cookie, http::StatusCode, test, web};
use chrono::{Duration, Utc};
use mongodb::bson::oid::ObjectId;
use serde_json::{Value, json};

use auth_backend::{
    app::{build_cors, configure_app_services},
    common::pagination::PaginationParams,
    database::mongodb::chrono_to_bson_dt,
    middleware::RequestIdAndSecurityHeaders,
    repository::auth::dao::refresh_token::{RefreshToken, RefreshTokenTransport},
    security::csrf::sha256_hex,
    test_support::create_test_app_state,
};

fn extract_cookie<B>(resp: &actix_web::dev::ServiceResponse<B>, name: &str) -> Option<String> {
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
    assert!(body["data"].get("refresh_token").is_none());
    assert!(body["data"].get("refresh_expires_in").is_none());
    assert!(body["data"].get("refresh_expires_at").is_none());
    assert_eq!(body["data"]["csrf_token"], csrf_cookie_1);

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

    // 7. Refresh with valid cookie + matching CSRF header + {} JSON body (like web Axios client) rotates token
    let refresh_req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .cookie(Cookie::new("refresh_token", refresh_cookie_1.clone()))
        .cookie(Cookie::new("csrf_token", csrf_cookie_1.clone()))
        .insert_header(("X-CSRF-Token", csrf_cookie_1.clone()))
        .set_json(json!({}))
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
        transport: RefreshTokenTransport::Cookie,
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

#[actix_web::test]
async fn repeated_refresh_rotation_prunes_old_tokens_and_logout_deletes_family() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    let login_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({
                "email": "superadmin@test.local",
                "password": "SuperAdmin123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(login_resp.status(), StatusCode::OK);
    let mut current_refresh = extract_cookie(&login_resp, "refresh_token").unwrap();
    let mut current_csrf = extract_cookie(&login_resp, "csrf_token").unwrap();

    let family_id = state
        .refresh_token_repo
        .find_by_hash(&sha256_hex(&current_refresh))
        .await
        .unwrap()
        .unwrap()
        .family_id;

    // Simulate rapid F5 refreshes (12 rotations in a row)
    let mut previous_refresh = String::new();
    for _ in 0..12 {
        previous_refresh = current_refresh.clone();
        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/auth/refresh")
                .cookie(Cookie::new("refresh_token", current_refresh.clone()))
                .cookie(Cookie::new("csrf_token", current_csrf.clone()))
                .insert_header(("X-CSRF-Token", current_csrf.clone()))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        current_refresh = extract_cookie(&resp, "refresh_token").unwrap();
        current_csrf = extract_cookie(&resp, "csrf_token").unwrap();
    }

    // Stored tokens for this family must stay bounded: 1 active + at most 5 revoked = 6
    let count = state
        .refresh_token_repo
        .count_for_family(&family_id)
        .await
        .unwrap();
    assert_eq!(count, 6);

    // Recent rotated token reuse is still detected (401 REFRESH_TOKEN_REUSED)
    let reuse_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .cookie(Cookie::new("refresh_token", previous_refresh))
            .cookie(Cookie::new("csrf_token", current_csrf.clone()))
            .insert_header(("X-CSRF-Token", current_csrf.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(reuse_resp.status(), StatusCode::UNAUTHORIZED);
    let reuse_body: Value = test::read_body_json(reuse_resp).await;
    assert_eq!(reuse_body["error"]["code"], "REFRESH_TOKEN_REUSED");

    // Logout hard-deletes the entire token family (0 documents left behind)
    let logout_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/logout")
            .cookie(Cookie::new("refresh_token", current_refresh))
            .cookie(Cookie::new("csrf_token", current_csrf.clone()))
            .insert_header(("X-CSRF-Token", current_csrf))
            .to_request(),
    )
    .await;
    assert_eq!(logout_resp.status(), StatusCode::OK);
    assert_eq!(
        state
            .refresh_token_repo
            .count_for_family(&family_id)
            .await
            .unwrap(),
        0
    );
}

#[actix_web::test]
async fn body_refresh_token_lifecycle_rotation_reuse_logout_and_password_revocation() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new()
            .wrap(RequestIdAndSecurityHeaders)
            .configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // 1. Register via X-Auth-Transport: body returns refresh_token in JSON and emits zero auth cookies
    let reg_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .insert_header(("X-Auth-Transport", "body"))
            .set_json(json!({
                "first_name": "Desktop",
                "last_name": "Client",
                "email": "desktop.client@example.com",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(reg_resp.status(), StatusCode::CREATED);
    assert!(extract_cookie(&reg_resp, "refresh_token").is_none());
    assert!(extract_cookie(&reg_resp, "csrf_token").is_none());
    assert_eq!(
        reg_resp
            .headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap(),
        "no-store"
    );

    let reg_body: Value = test::read_body_json(reg_resp).await;
    let body_refresh_1 = reg_body["data"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(!body_refresh_1.is_empty());
    assert!(reg_body["data"]["refresh_expires_in"].as_i64().unwrap() > 0);
    assert!(reg_body["data"]["refresh_expires_at"].as_str().is_some());
    assert!(reg_body["data"].get("csrf_token").is_none());

    // 2. Rotate body refresh token via JSON body (no cookies or CSRF required)
    let refresh_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .set_json(json!({ "refresh_token": body_refresh_1 }))
            .to_request(),
    )
    .await;
    assert_eq!(refresh_resp.status(), StatusCode::OK);
    assert!(extract_cookie(&refresh_resp, "refresh_token").is_none());
    assert!(extract_cookie(&refresh_resp, "csrf_token").is_none());
    let refresh_body: Value = test::read_body_json(refresh_resp).await;
    let body_refresh_2 = refresh_body["data"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(body_refresh_1, body_refresh_2);
    assert!(refresh_body["data"].get("csrf_token").is_none());

    // 3. Reusing rotated body_refresh_1 fails with 401 REFRESH_TOKEN_REUSED and revokes family
    let reuse_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .set_json(json!({ "refresh_token": body_refresh_1 }))
            .to_request(),
    )
    .await;
    assert_eq!(reuse_resp.status(), StatusCode::UNAUTHORIZED);
    let reuse_body: Value = test::read_body_json(reuse_resp).await;
    assert_eq!(reuse_body["error"]["code"], "REFRESH_TOKEN_REUSED");

    // Family is revoked so body_refresh_2 is also rejected
    let revoked_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .set_json(json!({ "refresh_token": body_refresh_2 }))
            .to_request(),
    )
    .await;
    assert_eq!(revoked_resp.status(), StatusCode::UNAUTHORIZED);

    // 4. Login with X-Auth-Transport: body, then logout via JSON body deletes family
    let login_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("X-Auth-Transport", "body"))
            .set_json(json!({
                "email": "desktop.client@example.com",
                "password": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(login_resp.status(), StatusCode::OK);
    assert!(extract_cookie(&login_resp, "refresh_token").is_none());
    let login_body: Value = test::read_body_json(login_resp).await;
    let login_refresh = login_body["data"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();

    let logout_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/logout")
            .set_json(json!({ "refresh_token": login_refresh }))
            .to_request(),
    )
    .await;
    assert_eq!(logout_resp.status(), StatusCode::OK);
    assert!(extract_cookie(&logout_resp, "refresh_token").is_none());

    let after_logout_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .set_json(json!({ "refresh_token": login_refresh }))
            .to_request(),
    )
    .await;
    assert_eq!(after_logout_resp.status(), StatusCode::UNAUTHORIZED);

    // 5. Password change revokes active body refresh tokens too
    let login_resp_2 = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("X-Auth-Transport", "body"))
            .set_json(json!({
                "email": "desktop.client@example.com",
                "password": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    let login_body_2: Value = test::read_body_json(login_resp_2).await;
    let access_token = login_body_2["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let refresh_before_pw_change = login_body_2["data"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();

    let pw_resp = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri("/api/v1/profile/password")
            .insert_header(("Authorization", format!("Bearer {access_token}")))
            .set_json(json!({
                "current_password": "StrongPassword123!",
                "new_password": "NewStrongPassword456!",
                "new_password_confirmation": "NewStrongPassword456!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(pw_resp.status(), StatusCode::OK);

    let after_pw_refresh = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .set_json(json!({ "refresh_token": refresh_before_pw_change }))
            .to_request(),
    )
    .await;
    assert_eq!(after_pw_refresh.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn strict_transport_binding_and_ambiguity_guards_isolate_cookie_and_body_sessions() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // Issue one Cookie session and one Body session for the same user
    let cookie_login = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({
                "email": "superadmin@test.local",
                "password": "SuperAdmin123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(cookie_login.status(), StatusCode::OK);
    let cookie_refresh = extract_cookie(&cookie_login, "refresh_token").unwrap();
    let cookie_csrf = extract_cookie(&cookie_login, "csrf_token").unwrap();

    let body_login = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("X-Auth-Transport", "body"))
            .set_json(json!({
                "email": "superadmin@test.local",
                "password": "SuperAdmin123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(body_login.status(), StatusCode::OK);
    let body_login_json: Value = test::read_body_json(body_login).await;
    let body_refresh = body_login_json["data"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();

    // 1. Cookie session cannot downgrade to body transport via X-Auth-Transport: body header
    let downgrade_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .cookie(Cookie::new("refresh_token", cookie_refresh.clone()))
            .cookie(Cookie::new("csrf_token", cookie_csrf.clone()))
            .insert_header(("X-CSRF-Token", cookie_csrf.clone()))
            .insert_header(("X-Auth-Transport", "body"))
            .to_request(),
    )
    .await;
    assert_eq!(downgrade_resp.status(), StatusCode::BAD_REQUEST);
    let downgrade_body: Value = test::read_body_json(downgrade_resp).await;
    assert_eq!(
        downgrade_body["error"]["code"],
        "REFRESH_TOKEN_TRANSPORT_MISMATCH"
    );

    // 2. Cookie-issued token presented in JSON body is rejected with 401
    let cookie_in_body_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .set_json(json!({ "refresh_token": cookie_refresh }))
            .to_request(),
    )
    .await;
    assert_eq!(cookie_in_body_resp.status(), StatusCode::UNAUTHORIZED);

    // 3. Body-issued token presented in Cookie is rejected with 401
    let body_in_cookie_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .cookie(Cookie::new("refresh_token", body_refresh.clone()))
            .cookie(Cookie::new("csrf_token", "csrf-1"))
            .insert_header(("X-CSRF-Token", "csrf-1"))
            .to_request(),
    )
    .await;
    assert_eq!(body_in_cookie_resp.status(), StatusCode::UNAUTHORIZED);

    // 4. Supplying BOTH cookie and body refresh_token is rejected as ambiguous (400)
    let ambiguous_refresh = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .cookie(Cookie::new("refresh_token", cookie_refresh.clone()))
            .cookie(Cookie::new("csrf_token", cookie_csrf.clone()))
            .insert_header(("X-CSRF-Token", cookie_csrf.clone()))
            .set_json(json!({ "refresh_token": body_refresh }))
            .to_request(),
    )
    .await;
    assert_eq!(ambiguous_refresh.status(), StatusCode::BAD_REQUEST);
    let ambiguous_body: Value = test::read_body_json(ambiguous_refresh).await;
    assert_eq!(ambiguous_body["error"]["code"], "AMBIGUOUS_REFRESH_TOKEN");

    let ambiguous_logout = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/logout")
            .cookie(Cookie::new("refresh_token", cookie_refresh.clone()))
            .cookie(Cookie::new("csrf_token", cookie_csrf.clone()))
            .insert_header(("X-CSRF-Token", cookie_csrf.clone()))
            .set_json(json!({ "refresh_token": body_refresh }))
            .to_request(),
    )
    .await;
    assert_eq!(ambiguous_logout.status(), StatusCode::BAD_REQUEST);

    // 5. Cross-channel logout does NOT delete the other transport's session
    let wrong_channel_logout = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/logout")
            .set_json(json!({ "refresh_token": cookie_refresh }))
            .to_request(),
    )
    .await;
    assert_eq!(wrong_channel_logout.status(), StatusCode::OK);

    // Cookie session is still alive after mismatched body logout attempt
    let cookie_still_valid = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .cookie(Cookie::new("refresh_token", cookie_refresh))
            .cookie(Cookie::new("csrf_token", cookie_csrf.clone()))
            .insert_header(("X-CSRF-Token", cookie_csrf))
            .to_request(),
    )
    .await;
    assert_eq!(cookie_still_valid.status(), StatusCode::OK);

    // 6. Non-JSON Content-Type and invalid X-Auth-Transport header are rejected with 400
    let bad_ct = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .insert_header(("Content-Type", "text/plain"))
            .set_payload(format!("{{\"refresh_token\":\"{body_refresh}\"}}"))
            .to_request(),
    )
    .await;
    assert_eq!(bad_ct.status(), StatusCode::BAD_REQUEST);
    let bad_ct_body: Value = test::read_body_json(bad_ct).await;
    assert_eq!(bad_ct_body["error"]["code"], "INVALID_CONTENT_TYPE");

    let bad_header = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("X-Auth-Transport", "magic"))
            .set_json(json!({
                "email": "superadmin@test.local",
                "password": "SuperAdmin123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(bad_header.status(), StatusCode::BAD_REQUEST);
    let bad_header_body: Value = test::read_body_json(bad_header).await;
    assert_eq!(bad_header_body["error"]["code"], "INVALID_AUTH_TRANSPORT");

    // Verify mismatch audit warnings were recorded
    let (_, mismatch_count) = state
        .audit_repo
        .find_all(
            &auth_backend::repository::audit::audit_log_repository::AuditLogFilter {
                action: Some("auth.refresh_token_transport_mismatch".to_string()),
                limit: 10,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(mismatch_count, 3);
}

#[actix_web::test]
async fn webview_origin_allowlist_browser_guards_and_feature_toggle() {
    let mut state = create_test_app_state().await.unwrap();
    let mut settings = (*state.settings).clone();
    settings.server.allowed_body_token_origins = vec!["tauri://localhost".to_string()];
    state.settings = std::sync::Arc::new(settings.clone());

    let app = test::init_service(
        App::new()
            .wrap(build_cors(&settings))
            .configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // 1. Web SPA origin (localhost:5173) is forbidden from using body transport
    let web_spa_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("Origin", "http://localhost:5173"))
            .insert_header(("X-Auth-Transport", "body"))
            .set_json(json!({
                "email": "superadmin@test.local",
                "password": "SuperAdmin123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(web_spa_resp.status(), StatusCode::FORBIDDEN);
    let web_spa_body: Value = test::read_body_json(web_spa_resp).await;
    assert_eq!(web_spa_body["error"]["code"], "BROWSER_ORIGIN_FORBIDDEN");

    // 2. Browser Sec-Fetch-Site without allowlisted Origin is forbidden
    let fetch_meta_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("Sec-Fetch-Site", "same-origin"))
            .insert_header(("X-Auth-Transport", "body"))
            .set_json(json!({
                "email": "superadmin@test.local",
                "password": "SuperAdmin123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(fetch_meta_resp.status(), StatusCode::FORBIDDEN);

    // 3. Allowlisted WebView origin (tauri://localhost) passes CORS preflight AND full login/refresh/logout
    let preflight_resp = test::call_service(
        &app,
        test::TestRequest::default()
            .method(actix_web::http::Method::OPTIONS)
            .uri("/api/v1/auth/login")
            .insert_header(("Origin", "tauri://localhost"))
            .insert_header(("Access-Control-Request-Method", "POST"))
            .insert_header((
                "Access-Control-Request-Headers",
                "content-type, x-auth-transport",
            ))
            .to_request(),
    )
    .await;
    assert_eq!(preflight_resp.status(), StatusCode::OK);
    assert_eq!(
        preflight_resp
            .headers()
            .get("access-control-allow-origin")
            .unwrap()
            .to_str()
            .unwrap(),
        "tauri://localhost"
    );

    let webview_login = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("Origin", "tauri://localhost"))
            .insert_header(("Sec-Fetch-Site", "cross-site"))
            .insert_header(("X-Auth-Transport", "body"))
            .set_json(json!({
                "email": "superadmin@test.local",
                "password": "SuperAdmin123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(webview_login.status(), StatusCode::OK);
    assert!(extract_cookie(&webview_login, "refresh_token").is_none());
    let webview_body: Value = test::read_body_json(webview_login).await;
    let webview_refresh = webview_body["data"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();

    let webview_refresh_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .insert_header(("Origin", "tauri://localhost"))
            .insert_header(("Sec-Fetch-Site", "cross-site"))
            .set_json(json!({ "refresh_token": webview_refresh }))
            .to_request(),
    )
    .await;
    assert_eq!(webview_refresh_resp.status(), StatusCode::OK);
    let webview_refresh_body: Value = test::read_body_json(webview_refresh_resp).await;
    let rotated_refresh = webview_refresh_body["data"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();

    // 4. Disabling allow_body_refresh_tokens blocks new logins/refreshes (403) while still allowing body logout
    let mut disabled_settings = settings;
    disabled_settings.app.allow_body_refresh_tokens = false;
    state.settings = std::sync::Arc::new(disabled_settings);
    let disabled_app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    let disabled_login = test::call_service(
        &disabled_app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("X-Auth-Transport", "body"))
            .set_json(json!({
                "email": "superadmin@test.local",
                "password": "SuperAdmin123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(disabled_login.status(), StatusCode::FORBIDDEN);
    let disabled_body: Value = test::read_body_json(disabled_login).await;
    assert_eq!(
        disabled_body["error"]["code"],
        "BODY_REFRESH_TOKENS_DISABLED"
    );

    let disabled_refresh = test::call_service(
        &disabled_app,
        test::TestRequest::post()
            .uri("/api/v1/auth/refresh")
            .set_json(json!({ "refresh_token": rotated_refresh }))
            .to_request(),
    )
    .await;
    assert_eq!(disabled_refresh.status(), StatusCode::FORBIDDEN);

    let cleanup_logout = test::call_service(
        &disabled_app,
        test::TestRequest::post()
            .uri("/api/v1/auth/logout")
            .insert_header(("Origin", "tauri://localhost"))
            .set_json(json!({ "refresh_token": rotated_refresh }))
            .to_request(),
    )
    .await;
    assert_eq!(cleanup_logout.status(), StatusCode::OK);
}
