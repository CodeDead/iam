#![allow(clippy::unwrap_used, clippy::expect_used)]

use actix_web::{App, http::StatusCode, test, web};
use serde_json::{Value, json};

use auth_backend::{app::configure_app_services, test_support::create_test_app_state};

#[actix_web::test]
async fn permission_crud_and_system_permission_protection() {
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
    let login_body: Value = test::read_body_json(login_resp).await;
    let sa_token = login_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    // Create custom permission
    let create_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/permissions")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "resource": "reports",
                "action": "export",
                "description": "Export compliance reports"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created_body: Value = test::read_body_json(create_resp).await;
    assert_eq!(created_body["data"]["name"], "reports.export");
    assert_eq!(created_body["data"]["is_system"], false);
    let perm_id = created_body["data"]["id"].as_str().unwrap().to_string();

    // Duplicate permission returns 409
    let dup_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/permissions")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "resource": "reports",
                "action": "export",
                "description": "Duplicate"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(dup_resp.status(), StatusCode::CONFLICT);

    // Update custom permission
    let update_resp = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/permissions/{perm_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "description": "Export financial and compliance reports"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(update_resp.status(), StatusCode::OK);

    // System permission cannot be modified or deleted
    let sys_perm_id = state
        .permission_repo
        .find_by_name("users.read")
        .await
        .unwrap()
        .unwrap()
        .id
        .to_hex();

    let update_sys = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/permissions/{sys_perm_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({ "description": "Tampered" }))
            .to_request(),
    )
    .await;
    assert_eq!(update_sys.status(), StatusCode::FORBIDDEN);

    let delete_sys = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/v1/permissions/{sys_perm_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(delete_sys.status(), StatusCode::FORBIDDEN);

    // Associated roles lookup works
    let assoc_roles = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/permissions/{sys_perm_id}/roles"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(assoc_roles.status(), StatusCode::OK);
    let roles_body: Value = test::read_body_json(assoc_roles).await;
    assert!(!roles_body["data"].as_array().unwrap().is_empty());

    // Delete custom permission
    let delete_custom = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/v1/permissions/{perm_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(delete_custom.status(), StatusCode::NO_CONTENT);
}
