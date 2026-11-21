#![allow(clippy::unwrap_used, clippy::expect_used)]

use actix_web::{App, http::StatusCode, test, web};
use serde_json::{Value, json};

use auth_backend::{
    app::configure_app_services, database::seed::ROLE_USER, test_support::create_test_app_state,
};

#[actix_web::test]
async fn role_crud_system_role_protection_and_immediate_permission_revocation() {
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
    let login_body: Value = test::read_body_json(login_resp).await;
    let sa_token = login_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    // Register a regular user
    let reg_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Bob",
                "last_name": "Builder",
                "email": "bob@test.local",
                "password": "BobPassword123!",
                "password_confirmation": "BobPassword123!"
            }))
            .to_request(),
    )
    .await;
    let reg_body: Value = test::read_body_json(reg_resp).await;
    let bob_token = reg_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let bob_id = reg_body["data"]["user"]["id"].as_str().unwrap().to_string();

    // Scenario 3: Regular user cannot create roles (403)
    let forbidden_create = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/roles")
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .set_json(json!({
                "name": "Hacker",
                "description": "Unauthorized role",
                "permission_ids": []
            }))
            .to_request(),
    )
    .await;
    assert_eq!(forbidden_create.status(), StatusCode::FORBIDDEN);

    let users_read_id = state
        .permission_repo
        .find_by_name("users.read")
        .await
        .unwrap()
        .unwrap()
        .id
        .to_hex();
    let profile_read_id = state
        .permission_repo
        .find_by_name("profile.read")
        .await
        .unwrap()
        .unwrap()
        .id
        .to_hex();

    // Super Admin creates custom role "Auditor" with users.read
    let create_role_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/roles")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "name": "Auditor",
                "description": "Read-only user auditor",
                "permission_ids": [users_read_id, profile_read_id]
            }))
            .to_request(),
    )
    .await;
    assert_eq!(create_role_resp.status(), StatusCode::CREATED);
    let role_body: Value = test::read_body_json(create_role_resp).await;
    let auditor_role_id = role_body["data"]["id"].as_str().unwrap().to_string();

    // Assign Auditor role to Bob
    let assign_resp = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/users/{bob_id}/roles"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({ "role_ids": [auditor_role_id] }))
            .to_request(),
    )
    .await;
    assert_eq!(assign_resp.status(), StatusCode::OK);

    // Bob can now list users with his existing JWT (cache invalidated on role assignment!)
    let bob_list_ok = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/users")
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(bob_list_ok.status(), StatusCode::OK);

    // Scenario 5: Bob cannot modify role permissions without roles.update (403)
    let bob_mod_perms = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/roles/{auditor_role_id}/permissions"))
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .set_json(json!({ "permission_ids": [] }))
            .to_request(),
    )
    .await;
    assert_eq!(bob_mod_perms.status(), StatusCode::FORBIDDEN);

    // Scenario 9: Super Admin revokes users.read from Auditor role -> Bob IMMEDIATELY loses access!
    let revoke_perm_resp = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/roles/{auditor_role_id}/permissions"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({ "permission_ids": [profile_read_id] }))
            .to_request(),
    )
    .await;
    assert_eq!(revoke_perm_resp.status(), StatusCode::OK);

    let bob_list_revoked = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/users")
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(bob_list_revoked.status(), StatusCode::FORBIDDEN);

    // Protected system role cannot be deleted
    let system_user_role_id = state
        .role_repo
        .find_by_name(ROLE_USER)
        .await
        .unwrap()
        .unwrap()
        .id
        .to_hex();
    let delete_sys_role = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/v1/roles/{system_user_role_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(delete_sys_role.status(), StatusCode::FORBIDDEN);

    // Custom role can be deleted cleanly
    let delete_custom = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/v1/roles/{auditor_role_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(delete_custom.status(), StatusCode::NO_CONTENT);
}
