#![allow(clippy::unwrap_used, clippy::expect_used)]

use actix_web::{App, http::StatusCode, test, web};
use serde_json::{Value, json};

use auth_backend::{app::configure_app_services, test_support::create_test_app_state};

#[actix_web::test]
async fn audit_logs_record_permission_role_user_and_auth_events_and_enforce_rbac() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // 1. Login as Super Administrator (generates auth.login)
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

    // 2. Register a regular user (generates auth.registered) and verify they cannot read audit logs
    let reg_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Regular",
                "last_name": "Member",
                "email": "regular@test.local",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(reg_resp.status(), StatusCode::CREATED);
    let reg_body: Value = test::read_body_json(reg_resp).await;
    let regular_token = reg_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let forbidden_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/audit-logs")
            .insert_header(("Authorization", format!("Bearer {regular_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(forbidden_resp.status(), StatusCode::FORBIDDEN);

    // 3. Create, update, and delete a custom permission
    let create_perm = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/permissions")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "resource": "billing",
                "action": "read",
                "description": "View invoices"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(create_perm.status(), StatusCode::CREATED);
    let perm_id = test::read_body_json::<Value, _>(create_perm).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let update_perm = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/permissions/{perm_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "description": "View and download invoices"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(update_perm.status(), StatusCode::OK);

    // 4. Create, update, and delete a custom role
    let create_role = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/roles")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "name": "Auditor",
                "description": "Read-only compliance auditor",
                "permission_ids": [perm_id]
            }))
            .to_request(),
    )
    .await;
    assert_eq!(create_role.status(), StatusCode::CREATED);
    let role_id = test::read_body_json::<Value, _>(create_role).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let update_role = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/roles/{role_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "description": "Updated compliance auditor"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(update_role.status(), StatusCode::OK);

    let delete_role = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/v1/roles/{role_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(delete_role.status(), StatusCode::NO_CONTENT);

    let delete_perm = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/v1/permissions/{perm_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(delete_perm.status(), StatusCode::NO_CONTENT);

    // 5. List all audit logs as Super Administrator
    let list_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/audit-logs?page=1&page_size=50")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body: Value = test::read_body_json(list_resp).await;
    let items = list_body["data"]["items"].as_array().unwrap();
    let actions: Vec<&str> = items
        .iter()
        .filter_map(|item| item["action"].as_str())
        .collect();

    assert!(actions.contains(&"auth.login"));
    assert!(actions.contains(&"auth.registered"));
    assert!(actions.contains(&"permission.created"));
    assert!(actions.contains(&"permission.updated"));
    assert!(actions.contains(&"permission.deleted"));
    assert!(actions.contains(&"role.created"));
    assert!(actions.contains(&"role.updated"));
    assert!(actions.contains(&"role.deleted"));

    // 6. Filter by resource_type=permission
    let perm_logs_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/audit-logs?resource_type=permission")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(perm_logs_resp.status(), StatusCode::OK);
    let perm_logs_body: Value = test::read_body_json(perm_logs_resp).await;
    let perm_items = perm_logs_body["data"]["items"].as_array().unwrap();
    assert_eq!(perm_items.len(), 3);
    assert!(
        perm_items
            .iter()
            .all(|i| i["resource_type"] == "permission")
    );

    // 7. Fetch a single audit log entry by ID
    let first_id = perm_items[0]["id"].as_str().unwrap();
    let get_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/audit-logs/{first_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_body: Value = test::read_body_json(get_resp).await;
    assert_eq!(get_body["data"]["id"], first_id);
    assert_eq!(get_body["data"]["actor_email"], "superadmin@test.local");
}
