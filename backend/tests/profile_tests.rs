#![allow(clippy::unwrap_used, clippy::expect_used)]

use actix_web::{App, http::StatusCode, test, web};
use serde_json::{Value, json};

use auth_backend::{app::configure_app_services, test_support::create_test_app_state};

#[actix_web::test]
async fn self_service_account_deletion_verifies_password_protects_last_super_admin_and_audits() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // 1. Login as the only Super Administrator
    let sa_login = test::call_service(
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
    assert_eq!(sa_login.status(), StatusCode::OK);
    let sa_token = test::read_body_json::<Value, _>(sa_login).await["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    // 2. Last active Super Administrator cannot delete own account
    let sa_delete = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/v1/profile")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({ "current_password": "SuperAdmin123!" }))
            .to_request(),
    )
    .await;
    assert_eq!(sa_delete.status(), StatusCode::CONFLICT);
    let sa_delete_body: Value = test::read_body_json(sa_delete).await;
    assert_eq!(
        sa_delete_body["error"]["code"],
        "LAST_SUPER_ADMIN_PROTECTED"
    );

    // 3. Register a standard user
    let reg = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Self",
                "last_name": "Delete",
                "email": "selfdelete@test.local",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(reg.status(), StatusCode::CREATED);
    let user_token = test::read_body_json::<Value, _>(reg).await["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    // 4. Wrong current password is rejected with 400
    let wrong_pw = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/v1/profile")
            .insert_header(("Authorization", format!("Bearer {user_token}")))
            .set_json(json!({ "current_password": "WrongPassword123!" }))
            .to_request(),
    )
    .await;
    assert_eq!(wrong_pw.status(), StatusCode::BAD_REQUEST);
    let wrong_pw_body: Value = test::read_body_json(wrong_pw).await;
    assert_eq!(wrong_pw_body["error"]["code"], "INVALID_CURRENT_PASSWORD");

    // 5. Correct password permanently deletes the account and clears cookies
    let ok_delete = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/v1/profile")
            .insert_header(("Authorization", format!("Bearer {user_token}")))
            .set_json(json!({ "current_password": "StrongPassword123!" }))
            .to_request(),
    )
    .await;
    assert_eq!(ok_delete.status(), StatusCode::OK);
    assert!(
        ok_delete
            .response()
            .cookies()
            .any(|c| c.name() == "refresh_token" && c.value().is_empty())
    );

    // 6. Former access token is now rejected and user cannot sign in
    let me_after = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/auth/me")
            .insert_header(("Authorization", format!("Bearer {user_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(me_after.status(), StatusCode::UNAUTHORIZED);

    // 7. Audit log recorded profile.account_deleted
    let logs = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/audit-logs?action=profile.account_deleted")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(logs.status(), StatusCode::OK);
    let logs_body: Value = test::read_body_json(logs).await;
    let items = logs_body["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["actor_email"], "selfdelete@test.local");
}
