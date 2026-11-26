#![allow(clippy::unwrap_used, clippy::expect_used)]

use actix_web::{App, http::StatusCode, test, web};
use serde_json::{Value, json};

use auth_backend::{
    app::configure_app_services,
    database::seed::{ROLE_ADMIN, ROLE_MODERATOR, ROLE_SUPER_ADMIN, ROLE_USER},
    test_support::create_test_app_state,
};

macro_rules! login_as {
    ($app:expr, $email:expr, $password:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({ "email": $email, "password": $password }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        let token = body["data"]["access_token"].as_str().unwrap().to_string();
        (token, body["data"]["user"].clone())
    }};
}

#[actix_web::test]
async fn user_crud_authorization_disabled_account_and_privilege_escalation_rules() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    let (sa_token, sa_user) = login_as!(app, "superadmin@test.local", "SuperAdmin123!");
    let sa_id = sa_user["id"].as_str().unwrap().to_string();

    let admin_role_id = state
        .role_repo
        .find_by_name(ROLE_ADMIN)
        .await
        .unwrap()
        .unwrap()
        .id
        .to_hex();
    let super_admin_role_id = state
        .role_repo
        .find_by_name(ROLE_SUPER_ADMIN)
        .await
        .unwrap()
        .unwrap()
        .id
        .to_hex();
    let user_role_id = state
        .role_repo
        .find_by_name(ROLE_USER)
        .await
        .unwrap()
        .unwrap()
        .id
        .to_hex();

    // Register a regular user
    let reg_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Regular",
                "last_name": "User",
                "email": "regular@test.local",
                "password": "UserPass123!",
                "password_confirmation": "UserPass123!"
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
    let regular_id = reg_body["data"]["user"]["id"].as_str().unwrap().to_string();

    // Scenario 1: Unauthenticated user cannot list users (401)
    let resp = test::call_service(
        &app,
        test::TestRequest::get().uri("/api/v1/users").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Scenario 2: Regular user cannot list all users without users.read (403)
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/users")
            .insert_header(("Authorization", format!("Bearer {regular_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Scenario 4: Regular user cannot assign themselves an Administrator role (403)
    let resp = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/users/{regular_id}/roles"))
            .insert_header(("Authorization", format!("Bearer {regular_token}")))
            .set_json(json!({ "role_ids": [admin_role_id] }))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Super admin creates an Administrator user
    let create_admin_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/users")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "first_name": "Admin",
                "last_name": "One",
                "email": "admin1@test.local",
                "password": "AdminPass123!",
                "role_ids": [admin_role_id]
            }))
            .to_request(),
    )
    .await;
    assert_eq!(create_admin_resp.status(), StatusCode::CREATED);
    let admin_body: Value = test::read_body_json(create_admin_resp).await;
    let admin_id = admin_body["data"]["id"].as_str().unwrap().to_string();

    let (admin_token, _) = login_as!(app, "admin1@test.local", "AdminPass123!");

    // Scenario 6: Administrator (who has users.assign_roles but NOT users.manage_self_roles) cannot modify own roles!
    let self_role_resp = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/users/{admin_id}/roles"))
            .insert_header(("Authorization", format!("Bearer {admin_token}")))
            .set_json(json!({ "role_ids": [admin_role_id, user_role_id] }))
            .to_request(),
    )
    .await;
    assert_eq!(self_role_resp.status(), StatusCode::FORBIDDEN);

    // Administrator cannot assign Super Administrator role to anyone (privilege escalation blocked)
    let escalate_resp = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/users/{regular_id}/roles"))
            .insert_header(("Authorization", format!("Bearer {admin_token}")))
            .set_json(json!({ "role_ids": [super_admin_role_id] }))
            .to_request(),
    )
    .await;
    assert_eq!(escalate_resp.status(), StatusCode::FORBIDDEN);

    // Scenario 8: Cannot delete or disable the last active Super Administrator
    let delete_sa_by_admin = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/v1/users/{sa_id}"))
            .insert_header(("Authorization", format!("Bearer {admin_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(delete_sa_by_admin.status(), StatusCode::FORBIDDEN);

    // Super admin cannot demote themselves when they are the last active super admin
    let demote_last_sa = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/users/{sa_id}/roles"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({ "role_ids": [admin_role_id] }))
            .to_request(),
    )
    .await;
    assert_eq!(demote_last_sa.status(), StatusCode::CONFLICT);

    // Scenario 7: Disabling regular user prevents authentication AND invalidates existing access token
    let disable_resp = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/api/v1/users/{regular_id}/status"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({ "status": "disabled" }))
            .to_request(),
    )
    .await;
    assert_eq!(disable_resp.status(), StatusCode::OK);

    // Existing token fails immediately
    let me_disabled = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/auth/me")
            .insert_header(("Authorization", format!("Bearer {regular_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(me_disabled.status(), StatusCode::FORBIDDEN);

    // Fresh login fails with 403 ACCOUNT_DISABLED
    let login_disabled = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({
                "email": "regular@test.local",
                "password": "UserPass123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(login_disabled.status(), StatusCode::FORBIDDEN);

    // Pagination, search & status filter (including full "first last" name search)
    let list_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/users?status=disabled&search=regular&page=1&page_size=10")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body: Value = test::read_body_json(list_resp).await;
    assert_eq!(list_body["data"]["total"], 1);
    assert_eq!(list_body["data"]["items"][0]["email"], "regular@test.local");

    let full_name_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/users?search=Regular%20User")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(full_name_resp.status(), StatusCode::OK);
    let full_name_body: Value = test::read_body_json(full_name_resp).await;
    assert_eq!(full_name_body["data"]["total"], 1);
    assert_eq!(
        full_name_body["data"]["items"][0]["email"],
        "regular@test.local"
    );

    let mismatch_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/users?search=Regular%20Nonexistent")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(mismatch_resp.status(), StatusCode::OK);
    let mismatch_body: Value = test::read_body_json(mismatch_resp).await;
    assert_eq!(mismatch_body["data"]["total"], 0);

    // Profile endpoints: cannot mass-assign roles or status
    let profile_mass_assign = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/api/v1/profile")
            .insert_header(("Authorization", format!("Bearer {admin_token}")))
            .set_json(json!({
                "first_name": "Updated",
                "status": "locked"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(profile_mass_assign.status(), StatusCode::BAD_REQUEST);

    // Valid profile update works
    let profile_ok = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/api/v1/profile")
            .insert_header(("Authorization", format!("Bearer {admin_token}")))
            .set_json(json!({
                "first_name": "UpdatedAdmin",
                "last_name": "One"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(profile_ok.status(), StatusCode::OK);

    // Password change revokes old access token immediately
    let pw_change = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri("/api/v1/profile/password")
            .insert_header(("Authorization", format!("Bearer {admin_token}")))
            .set_json(json!({
                "current_password": "AdminPass123!",
                "new_password": "NewAdminPass456!",
                "new_password_confirmation": "NewAdminPass456!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(pw_change.status(), StatusCode::OK);

    let old_token_after_pw = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/profile")
            .insert_header(("Authorization", format!("Bearer {admin_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(old_token_after_pw.status(), StatusCode::UNAUTHORIZED);

    // Dashboard stats filtered by permissions
    let sa_stats = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/dashboard/stats")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(sa_stats.status(), StatusCode::OK);
    let stats_body: Value = test::read_body_json(sa_stats).await;
    assert!(stats_body["data"]["users"]["total"].as_u64().unwrap() >= 3);
    assert!(stats_body["data"]["roles"]["total"].as_u64().unwrap() >= 4);

    // Moderator (holds users.update) CANNOT disable or edit an Administrator (higher privilege)
    let moderator_role_id = state
        .role_repo
        .find_by_name(ROLE_MODERATOR)
        .await
        .unwrap()
        .unwrap()
        .id
        .to_hex();
    let create_mod = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/users")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "first_name": "Mod",
                "last_name": "One",
                "email": "mod1@test.local",
                "password": "ModPassword123!",
                "role_ids": [moderator_role_id]
            }))
            .to_request(),
    )
    .await;
    assert_eq!(create_mod.status(), StatusCode::CREATED);
    let (mod_token, _) = login_as!(app, "mod1@test.local", "ModPassword123!");

    let mod_disable_admin = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/api/v1/users/{admin_id}/status"))
            .insert_header(("Authorization", format!("Bearer {mod_token}")))
            .set_json(json!({ "status": "disabled" }))
            .to_request(),
    )
    .await;
    assert_eq!(mod_disable_admin.status(), StatusCode::FORBIDDEN);

    let mod_edit_admin = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/users/{admin_id}"))
            .insert_header(("Authorization", format!("Bearer {mod_token}")))
            .set_json(json!({ "first_name": "Hijacked" }))
            .to_request(),
    )
    .await;
    assert_eq!(mod_edit_admin.status(), StatusCode::FORBIDDEN);
}
