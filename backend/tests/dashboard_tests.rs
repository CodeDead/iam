#![allow(clippy::unwrap_used, clippy::expect_used)]

use actix_web::{App, http::StatusCode, test, web};
use serde_json::{Value, json};

use auth_backend::{app::configure_app_services, test_support::create_test_app_state};

#[actix_web::test]
async fn dashboard_stats_returns_rich_metrics_for_super_admin_and_enforces_strict_rbac() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // 1. Login as Super Administrator
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
    let login_body: Value = test::read_body_json(login_resp).await;
    let sa_token = login_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    // 2. Trigger one failed login attempt to verify security alert metrics
    let bad_login = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({
                "email": "superadmin@test.local",
                "password": "WrongPassword999!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(bad_login.status(), StatusCode::UNAUTHORIZED);

    // 3. Register a standard User (only profile.* permissions)
    let reg_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Standard",
                "last_name": "Member",
                "email": "member@test.local",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(reg_resp.status(), StatusCode::CREATED);
    let reg_body: Value = test::read_body_json(reg_resp).await;
    let member_token = reg_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    // 4. Super Admin dashboard returns full stats, charts data, and recent events (without raw details)
    let sa_dash_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/dashboard/stats")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(sa_dash_resp.status(), StatusCode::OK);
    let sa_dash: Value = test::read_body_json(sa_dash_resp).await;
    let data = &sa_dash["data"];

    assert_eq!(data["my_active_sessions"], 1);
    assert_eq!(data["users"]["total"], 2);
    assert_eq!(data["users"]["active"], 2);
    assert_eq!(data["users"]["new_last_7_days"], 2);
    assert_eq!(data["users"]["active_sessions"], 2);

    assert_eq!(data["roles"]["total"], 4);
    assert_eq!(data["roles"]["system"], 4);
    assert_eq!(data["roles"]["custom"], 0);
    let role_dist = data["roles"]["distribution"].as_array().unwrap();
    assert_eq!(role_dist.len(), 4);
    assert!(role_dist.iter().all(|r| r["user_count"].is_u64()));

    assert!(data["permissions"]["total"].as_u64().unwrap() >= 18);
    assert!(data["permissions"]["by_resource"].as_array().unwrap().len() >= 5);

    let audit = &data["audit"];
    assert!(audit["total"].as_u64().unwrap() >= 3);
    assert!(audit["events_last_24h"].as_u64().unwrap() >= 3);
    assert_eq!(audit["failed_logins_last_24h"], 1);
    assert_eq!(audit["token_reuse_alerts_last_24h"], 0);
    assert_eq!(audit["token_reuse_alerts_last_7d"], 0);
    assert!(audit.get("latest_token_reuse_at").is_none());
    assert_eq!(audit["daily_activity"].as_array().unwrap().len(), 7);
    assert!(!audit["by_resource_type"].as_array().unwrap().is_empty());

    let recent_events = audit["recent_events"].as_array().unwrap();
    assert!(!recent_events.is_empty());
    // Ensure raw sensitive details (IP / User-Agent / payloads) are NOT exposed in dashboard summary
    assert!(recent_events.iter().all(|e| e.get("details").is_none()));

    // 5. Standard User receives ONLY their own session count; all privileged sections strictly omitted
    let member_dash_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/dashboard/stats")
            .insert_header(("Authorization", format!("Bearer {member_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(member_dash_resp.status(), StatusCode::OK);
    let member_dash: Value = test::read_body_json(member_dash_resp).await;
    let member_data = &member_dash["data"];

    assert_eq!(member_data["my_active_sessions"], 1);
    assert!(member_data.get("users").is_none());
    assert!(member_data.get("roles").is_none());
    assert!(member_data.get("permissions").is_none());
    assert!(member_data.get("audit").is_none());
}

#[actix_web::test]
async fn dashboard_roles_distribution_omits_user_counts_without_users_read_permission() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // Login as Super Administrator
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
    let sa_token = test::read_body_json::<Value, _>(login_resp).await["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    // Find roles.read permission ID
    let perms_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/permissions?search=roles.read")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    let perms_body: Value = test::read_body_json(perms_resp).await;
    let roles_read_id = perms_body["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "roles.read")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create a custom role with ONLY roles.read
    let create_role = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/roles")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "name": "Role Viewer",
                "description": "Can inspect roles only",
                "permission_ids": [roles_read_id]
            }))
            .to_request(),
    )
    .await;
    assert_eq!(create_role.status(), StatusCode::CREATED);
    let role_viewer_id = test::read_body_json::<Value, _>(create_role).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create user with Role Viewer role
    let create_user = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/users")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "first_name": "Role",
                "last_name": "Viewer",
                "email": "roleviewer@test.local",
                "password": "StrongPassword123!",
                "role_ids": [role_viewer_id]
            }))
            .to_request(),
    )
    .await;
    assert_eq!(create_user.status(), StatusCode::CREATED);

    // Login as Role Viewer
    let rv_login = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({
                "email": "roleviewer@test.local",
                "password": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    let rv_token = test::read_body_json::<Value, _>(rv_login).await["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    // Fetch dashboard stats as Role Viewer
    let rv_dash = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/dashboard/stats")
            .insert_header(("Authorization", format!("Bearer {rv_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(rv_dash.status(), StatusCode::OK);
    let body: Value = test::read_body_json(rv_dash).await;
    let data = &body["data"];

    // Has roles (including custom count = 1), but NO users, permissions, audit, or per-role user_count
    assert_eq!(data["roles"]["total"], 5);
    assert_eq!(data["roles"]["custom"], 1);
    let dist = data["roles"]["distribution"].as_array().unwrap();
    assert!(!dist.is_empty());
    assert!(dist.iter().all(|r| r.get("user_count").is_none()));
    assert!(data.get("users").is_none());
    assert!(data.get("permissions").is_none());
    assert!(data.get("audit").is_none());
}
