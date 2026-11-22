#![allow(clippy::unwrap_used, clippy::expect_used)]

use actix_web::{App, http::StatusCode, test, web};
use serde_json::{Value, json};

use auth_backend::{app::configure_app_services, test_support::create_test_app_state};

#[actix_web::test]
async fn organisation_registration_crud_membership_dashboard_and_rbac_work() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // 1. Self-service registration with an organisation account ("Acme Inc.")
    let reg_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Alice",
                "last_name": "Founder",
                "email": "alice@acme.test",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!",
                "organisation_name": "Acme Inc."
            }))
            .to_request(),
    )
    .await;
    assert_eq!(reg_resp.status(), StatusCode::CREATED);
    let reg_body: Value = test::read_body_json(reg_resp).await;
    let alice_token = reg_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let alice_id = reg_body["data"]["user"]["id"].as_str().unwrap().to_string();
    assert_eq!(reg_body["data"]["organisation"]["name"], "Acme Inc.");
    assert_eq!(reg_body["data"]["organisation"]["status"], "active");
    assert_eq!(reg_body["data"]["organisation"]["member_count"], 1);
    let acme_id = reg_body["data"]["organisation"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // 2. Duplicate organisation name during registration is rejected with 409 (and creates no orphan user)
    let dup_reg = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Bob",
                "last_name": "Clone",
                "email": "bob@acme.test",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!",
                "organisation_name": "acme inc."
            }))
            .to_request(),
    )
    .await;
    assert_eq!(dup_reg.status(), StatusCode::CONFLICT);

    // 3. Standard user Alice can view her own organisations via /organisations/me and dashboard,
    //    but cannot list or modify global organisations.
    let my_orgs_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/organisations/me")
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(my_orgs_resp.status(), StatusCode::OK);
    let my_orgs: Value = test::read_body_json(my_orgs_resp).await;
    assert_eq!(my_orgs["data"].as_array().unwrap().len(), 1);
    assert_eq!(my_orgs["data"][0]["name"], "Acme Inc.");

    let alice_dash = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/dashboard/stats")
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(alice_dash.status(), StatusCode::OK);
    let alice_dash_body: Value = test::read_body_json(alice_dash).await;
    assert_eq!(
        alice_dash_body["data"]["my_organisations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(alice_dash_body["data"].get("organisations").is_none());

    let forbidden_list = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/organisations")
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(forbidden_list.status(), StatusCode::FORBIDDEN);

    // 4. Super Administrator logs in, creates another org, adds Alice, updates status, and checks dashboard
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
    let sa_token = test::read_body_json::<Value, _>(sa_login).await["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let create_org = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/organisations")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({ "name": "Globex Corp" }))
            .to_request(),
    )
    .await;
    assert_eq!(create_org.status(), StatusCode::CREATED);
    let globex_id = test::read_body_json::<Value, _>(create_org).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Add Alice by email to Globex Corp with invited status
    let add_member = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{globex_id}/members"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "email": "alice@acme.test",
                "status": "invited"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(add_member.status(), StatusCode::OK);
    let added_body: Value = test::read_body_json(add_member).await;
    assert_eq!(added_body["data"]["member_count"], 2);
    assert_eq!(added_body["data"]["active_member_count"], 1);

    // Update Alice's membership status in Globex Corp to suspended
    let patch_member = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!(
                "/api/v1/organisations/{globex_id}/members/{alice_id}"
            ))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({ "status": "suspended" }))
            .to_request(),
    )
    .await;
    assert_eq!(patch_member.status(), StatusCode::OK);

    // Super Admin dashboard includes global organisation metrics and largest_organisations
    let sa_dash = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/dashboard/stats")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(sa_dash.status(), StatusCode::OK);
    let sa_dash_body: Value = test::read_body_json(sa_dash).await;
    let org_stats = &sa_dash_body["data"]["organisations"];
    assert_eq!(org_stats["total"], 2);
    assert_eq!(org_stats["new_last_7_days"], 2);
    assert_eq!(org_stats["total_memberships"], 3);
    assert_eq!(org_stats["active_memberships"], 2);
    assert_eq!(org_stats["suspended_memberships"], 1);
    assert_eq!(
        org_stats["largest_organisations"].as_array().unwrap()[0]["name"],
        "Globex Corp"
    );

    // Update Acme Inc. name
    let update_org = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/api/v1/organisations/{acme_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({ "name": "Acme Corporation" }))
            .to_request(),
    )
    .await;
    assert_eq!(update_org.status(), StatusCode::OK);

    // Remove Alice from Globex Corp
    let remove_member = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!(
                "/api/v1/organisations/{globex_id}/members/{alice_id}"
            ))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(remove_member.status(), StatusCode::OK);
    let removed_body: Value = test::read_body_json(remove_member).await;
    assert_eq!(removed_body["data"]["member_count"], 1);

    // Delete Globex Corp
    let del_org = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/v1/organisations/{globex_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(del_org.status(), StatusCode::NO_CONTENT);
}

#[actix_web::test]
async fn org_self_service_email_invites_multi_org_isolation_and_data_minimization() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // 1. Alice registers Organisation "Alpha Org" (she is Owner, standard User role globally)
    let alice_reg = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Alice",
                "last_name": "Alpha",
                "email": "alice@alpha.test",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!",
                "organisation_name": "Alpha Org"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(alice_reg.status(), StatusCode::CREATED);
    let alice_body: Value = test::read_body_json(alice_reg).await;
    let alice_token = alice_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let alpha_id = alice_body["data"]["organisation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(alice_body["data"]["organisation"]["role"], "owner");

    // 2. Carol registers Organisation "Beta Org"
    let carol_reg = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Carol",
                "last_name": "Beta",
                "email": "carol@beta.test",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!",
                "organisation_name": "Beta Org"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(carol_reg.status(), StatusCode::CREATED);
    let carol_body: Value = test::read_body_json(carol_reg).await;
    let carol_token = carol_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let beta_id = carol_body["data"]["organisation"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // 3. Bob registers a personal account (no org yet)
    let bob_reg = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Bob",
                "last_name": "Private",
                "email": "bob@shared.test",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(bob_reg.status(), StatusCode::CREATED);
    let bob_body: Value = test::read_body_json(bob_reg).await;
    let bob_token = bob_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let bob_id = bob_body["data"]["user"]["id"].as_str().unwrap().to_string();

    // 4. Tenant isolation: Alice CANNOT view Beta Org
    let cross_tenant_get = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/organisations/{beta_id}"))
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(cross_tenant_get.status(), StatusCode::FORBIDDEN);

    // 5. Alice CANNOT add by raw user_id (must use signup email)
    let add_by_id = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{alpha_id}/members"))
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .set_json(json!({ "user_id": bob_id }))
            .to_request(),
    )
    .await;
    assert_eq!(add_by_id.status(), StatusCode::FORBIDDEN);

    // 6. Alice adds Bob to Alpha Org using his signup email -> starts as "invited"
    //    AND Bob's first_name / last_name are REDACTED until he accepts!
    let invite_alpha = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{alpha_id}/members"))
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .set_json(json!({ "email": "bob@shared.test" }))
            .to_request(),
    )
    .await;
    assert_eq!(invite_alpha.status(), StatusCode::OK);
    let alpha_after_invite: Value = test::read_body_json(invite_alpha).await;
    let bob_entry = alpha_after_invite["data"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["user_id"] == bob_id)
        .unwrap();
    assert_eq!(bob_entry["status"], "invited");
    assert_eq!(bob_entry["role"], "member");
    assert!(bob_entry.get("first_name").is_none());
    assert!(bob_entry.get("last_name").is_none());

    // Alice CANNOT force Bob to "active" without Bob accepting (directly OR via Invited -> Suspended -> Active)
    let force_active = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!(
                "/api/v1/organisations/{alpha_id}/members/{bob_id}"
            ))
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .set_json(json!({ "status": "active" }))
            .to_request(),
    )
    .await;
    assert_eq!(force_active.status(), StatusCode::FORBIDDEN);

    let force_suspend_invited = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!(
                "/api/v1/organisations/{alpha_id}/members/{bob_id}"
            ))
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .set_json(json!({ "status": "suspended" }))
            .to_request(),
    )
    .await;
    assert_eq!(force_suspend_invited.status(), StatusCode::FORBIDDEN);

    // 6b. Alice can also cancel a pending invitation before the user accepts it
    let cancel_invite = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!(
                "/api/v1/organisations/{alpha_id}/members/{bob_id}"
            ))
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(cancel_invite.status(), StatusCode::OK);
    let after_cancel: Value = test::read_body_json(cancel_invite).await;
    assert_eq!(after_cancel["data"]["member_count"], 1);

    // Re-invite Bob to Alpha Org so the rest of the multi-org acceptance flow runs
    let reinvite_alpha = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{alpha_id}/members"))
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .set_json(json!({ "email": "bob@shared.test" }))
            .to_request(),
    )
    .await;
    assert_eq!(reinvite_alpha.status(), StatusCode::OK);

    // 7. Carol ALSO invites Bob by signup email to Beta Org (multi-organisation membership!)
    let invite_beta = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{beta_id}/members"))
            .insert_header(("Authorization", format!("Bearer {carol_token}")))
            .set_json(json!({ "email": "bob@shared.test" }))
            .to_request(),
    )
    .await;
    assert_eq!(invite_beta.status(), StatusCode::OK);

    // 8. Bob accepts BOTH invitations and belongs to both Alpha Org and Beta Org
    for org_id in [&alpha_id, &beta_id] {
        let accept = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v1/organisations/{org_id}/accept"))
                .insert_header(("Authorization", format!("Bearer {bob_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(accept.status(), StatusCode::OK);
    }

    let bob_orgs = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/organisations/me")
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .to_request(),
    )
    .await;
    let bob_orgs_body: Value = test::read_body_json(bob_orgs).await;
    assert_eq!(bob_orgs_body["data"].as_array().unwrap().len(), 2);

    // Once Bob accepted, Alice can see Bob's display name in Alpha Org roster (but not profile/roles)
    let alpha_get = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/organisations/{alpha_id}"))
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(alpha_get.status(), StatusCode::OK);
    let alpha_get_body: Value = test::read_body_json(alpha_get).await;
    let bob_active = alpha_get_body["data"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["user_id"] == bob_id)
        .unwrap();
    assert_eq!(bob_active["status"], "active");
    assert_eq!(bob_active["first_name"], "Bob");
    assert_eq!(bob_active["last_name"], "Private");

    // Regular member Bob CANNOT view Alpha Org's full member roster
    let member_roster_attempt = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/organisations/{alpha_id}"))
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(member_roster_attempt.status(), StatusCode::FORBIDDEN);

    // 9. Alice removes Bob from Alpha Org; Bob remains active in Beta Org
    let remove_bob = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!(
                "/api/v1/organisations/{alpha_id}/members/{bob_id}"
            ))
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(remove_bob.status(), StatusCode::OK);

    let bob_orgs_after_remove = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/organisations/me")
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .to_request(),
    )
    .await;
    let remaining: Value = test::read_body_json(bob_orgs_after_remove).await;
    assert_eq!(remaining["data"].as_array().unwrap().len(), 1);
    assert_eq!(remaining["data"][0]["name"], "Beta Org");

    // 10. Bob leaves Beta Org voluntarily
    let leave_beta = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{beta_id}/leave"))
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(leave_beta.status(), StatusCode::OK);

    // 11. Carol CANNOT delete Alpha Org (403 tenant isolation),
    //     but Alice (regular user who owns Alpha Org) CAN delete Alpha Org (204)
    let cross_tenant_del = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/v1/organisations/{alpha_id}"))
            .insert_header(("Authorization", format!("Bearer {carol_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(cross_tenant_del.status(), StatusCode::FORBIDDEN);

    let owner_del = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/v1/organisations/{alpha_id}"))
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(owner_del.status(), StatusCode::NO_CONTENT);

    let alice_orgs_after_del = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/organisations/me")
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .to_request(),
    )
    .await;
    let alice_remaining: Value = test::read_body_json(alice_orgs_after_del).await;
    assert!(alice_remaining["data"].as_array().unwrap().is_empty());

    // 12. Personal user Bob can create his own company later, becomes active owner,
    //     and cannot attach arbitrary initial_member_ids without global permissions
    let bob_bad_create = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/organisations")
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .set_json(json!({
                "name": "Bob Hacked Corp",
                "initial_member_ids": [bob_id]
            }))
            .to_request(),
    )
    .await;
    assert_eq!(bob_bad_create.status(), StatusCode::FORBIDDEN);

    let bob_create = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/organisations")
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .set_json(json!({ "name": "Bob Ventures" }))
            .to_request(),
    )
    .await;
    assert_eq!(bob_create.status(), StatusCode::CREATED);
    let bob_org: Value = test::read_body_json(bob_create).await;
    assert_eq!(bob_org["data"]["name"], "Bob Ventures");
    assert_eq!(bob_org["data"]["member_count"], 1);
    assert_eq!(bob_org["data"]["members"][0]["role"], "owner");
    assert_eq!(bob_org["data"]["members"][0]["status"], "active");
}

#[actix_web::test]
async fn account_deletion_blocks_last_owner_of_shared_org_and_auto_deletes_solo_org() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // Login as Super Administrator for admin verification
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
    let sa_token = test::read_body_json::<Value, _>(sa_login).await["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    // 1. Solo founder creates "Solo Corp" and deletes their account -> org is auto-deleted
    let solo_reg = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Solo",
                "last_name": "Founder",
                "email": "solo@solo.test",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!",
                "organisation_name": "Solo Corp"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(solo_reg.status(), StatusCode::CREATED);
    let solo_body: Value = test::read_body_json(solo_reg).await;
    let solo_token = solo_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let solo_org_id = solo_body["data"]["organisation"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Sole active member cannot leave and create an empty org (409)
    let solo_leave = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{solo_org_id}/leave"))
            .insert_header(("Authorization", format!("Bearer {solo_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(solo_leave.status(), StatusCode::CONFLICT);

    // Self-deleting solo account succeeds and auto-deletes "Solo Corp"
    let solo_del = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/v1/profile")
            .insert_header(("Authorization", format!("Bearer {solo_token}")))
            .set_json(json!({ "current_password": "StrongPassword123!" }))
            .to_request(),
    )
    .await;
    assert_eq!(solo_del.status(), StatusCode::OK);

    let solo_org_check = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/organisations/{solo_org_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(solo_org_check.status(), StatusCode::NOT_FOUND);

    // 2. Shared org: Alice creates "Shared Co", invites Bob who accepts
    let alice_reg = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Alice",
                "last_name": "Owner",
                "email": "alice@shared.test",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!",
                "organisation_name": "Shared Co"
            }))
            .to_request(),
    )
    .await;
    let alice_body: Value = test::read_body_json(alice_reg).await;
    let alice_token = alice_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let alice_id = alice_body["data"]["user"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let shared_org_id = alice_body["data"]["organisation"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let bob_reg = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Bob",
                "last_name": "Teammate",
                "email": "bob@shared.test",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    let bob_body: Value = test::read_body_json(bob_reg).await;
    let bob_token = bob_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let bob_id = bob_body["data"]["user"]["id"].as_str().unwrap().to_string();

    let invite = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{shared_org_id}/members"))
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .set_json(json!({ "email": "bob@shared.test" }))
            .to_request(),
    )
    .await;
    assert_eq!(invite.status(), StatusCode::OK);

    let accept = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{shared_org_id}/accept"))
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(accept.status(), StatusCode::OK);

    // Alice cannot self-delete while last active owner of Shared Co (409)
    let blocked_self_del = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/v1/profile")
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .set_json(json!({ "current_password": "StrongPassword123!" }))
            .to_request(),
    )
    .await;
    assert_eq!(blocked_self_del.status(), StatusCode::CONFLICT);
    let err_body: Value = test::read_body_json(blocked_self_del).await;
    assert_eq!(err_body["error"]["code"], "LAST_ORG_OWNER_PROTECTED");
    assert!(
        err_body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Shared Co")
    );

    // Super Admin is also blocked from deleting Alice before transferring ownership (409)
    let blocked_admin_del = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/v1/users/{alice_id}"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(blocked_admin_del.status(), StatusCode::CONFLICT);

    // Alice promotes Bob to owner, then Alice can delete her account and Shared Co stays alive with Bob
    let promote = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!(
                "/api/v1/organisations/{shared_org_id}/members/{bob_id}"
            ))
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .set_json(json!({ "role": "owner" }))
            .to_request(),
    )
    .await;
    assert_eq!(promote.status(), StatusCode::OK);

    let alice_del = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/v1/profile")
            .insert_header(("Authorization", format!("Bearer {alice_token}")))
            .set_json(json!({ "current_password": "StrongPassword123!" }))
            .to_request(),
    )
    .await;
    assert_eq!(alice_del.status(), StatusCode::OK);

    let shared_org_after = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/organisations/{shared_org_id}"))
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(shared_org_after.status(), StatusCode::OK);
    let org_after_body: Value = test::read_body_json(shared_org_after).await;
    assert_eq!(org_after_body["data"]["member_count"], 1);
    assert_eq!(org_after_body["data"]["members"][0]["user_id"], bob_id);
    assert_eq!(org_after_body["data"]["members"][0]["role"], "owner");

    // 3. Admin cannot create an empty/ownerless org via include_creator: false without initial_member_ids
    let empty_org = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/organisations")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({ "name": "Ownerless Corp", "include_creator": false }))
            .to_request(),
    )
    .await;
    assert_eq!(empty_org.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 4. Deleting a non-owner (invited) member's account must NOT delete an organisation
    //    even if the owner happens to be suspended.
    let invitee_reg = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Temp",
                "last_name": "Invitee",
                "email": "invitee@shared.test",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    let invitee_body: Value = test::read_body_json(invitee_reg).await;
    let invitee_token = invitee_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let inv_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{shared_org_id}/members"))
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .set_json(json!({ "email": "invitee@shared.test" }))
            .to_request(),
    )
    .await;
    assert_eq!(inv_resp.status(), StatusCode::OK);

    let invitee_del = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/v1/profile")
            .insert_header(("Authorization", format!("Bearer {invitee_token}")))
            .set_json(json!({ "current_password": "StrongPassword123!" }))
            .to_request(),
    )
    .await;
    assert_eq!(invitee_del.status(), StatusCode::OK);

    let org_still_alive = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/organisations/{shared_org_id}"))
            .insert_header(("Authorization", format!("Bearer {bob_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(org_still_alive.status(), StatusCode::OK);
    let alive_body: Value = test::read_body_json(org_still_alive).await;
    assert_eq!(alive_body["data"]["member_count"], 1);
}

#[actix_web::test]
async fn organisation_initial_owner_security_and_user_memberships_work() {
    let state = create_test_app_state().await.unwrap();
    let app = test::init_service(
        App::new().configure(|cfg| configure_app_services(cfg, web::Data::new(state.clone()))),
    )
    .await;

    // Login as Super Administrator
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
    let sa_token = test::read_body_json::<Value, _>(sa_login).await["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    // Register founder Eve, teammate Frank, and disabled user Gina
    let eve_reg = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Eve",
                "last_name": "Owner",
                "email": "eve@delegated.test",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(eve_reg.status(), StatusCode::CREATED);
    let eve_body: Value = test::read_body_json(eve_reg).await;
    let eve_token = eve_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let eve_id = eve_body["data"]["user"]["id"].as_str().unwrap().to_string();

    let frank_reg = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Frank",
                "last_name": "Member",
                "email": "frank@delegated.test",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(frank_reg.status(), StatusCode::CREATED);
    let frank_body: Value = test::read_body_json(frank_reg).await;
    let frank_token = frank_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let frank_id = frank_body["data"]["user"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let gina_reg = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(json!({
                "first_name": "Gina",
                "last_name": "Disabled",
                "email": "gina@delegated.test",
                "password": "StrongPassword123!",
                "password_confirmation": "StrongPassword123!"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(gina_reg.status(), StatusCode::CREATED);
    let gina_id = test::read_body_json::<Value, _>(gina_reg).await["data"]["user"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Disable Gina
    let disable_gina = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/api/v1/users/{gina_id}/status"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({ "status": "disabled" }))
            .to_request(),
    )
    .await;
    assert_eq!(disable_gina.status(), StatusCode::OK);

    // 1. Standard user cannot pass initial_owner_email or include_creator: false (403)
    let eve_forbidden_create = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/organisations")
            .insert_header(("Authorization", format!("Bearer {eve_token}")))
            .set_json(json!({
                "name": "Spoofed Corp",
                "initial_owner_email": "frank@delegated.test"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(eve_forbidden_create.status(), StatusCode::FORBIDDEN);

    let eve_forbidden_omit = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/organisations")
            .insert_header(("Authorization", format!("Bearer {eve_token}")))
            .set_json(json!({
                "name": "Omitted Creator Corp",
                "include_creator": false
            }))
            .to_request(),
    )
    .await;
    assert_eq!(eve_forbidden_omit.status(), StatusCode::FORBIDDEN);

    // 1b. Admin cannot assign a disabled user as initial owner via initial_owner_email or initial_member_ids (422)
    let disabled_owner_email = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/organisations")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "name": "Disabled Owner Corp",
                "include_creator": false,
                "initial_owner_email": "gina@delegated.test"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(
        disabled_owner_email.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let disabled_owner_id = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/organisations")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "name": "Disabled Owner Id Corp",
                "include_creator": false,
                "initial_member_ids": [gina_id]
            }))
            .to_request(),
    )
    .await;
    assert_eq!(disabled_owner_id.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 2. Super Admin creates org with include_creator: false and initial_owner_email
    let delegated_create = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/organisations")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "name": "Delegated LLC",
                "include_creator": false,
                "initial_owner_email": "eve@delegated.test"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(delegated_create.status(), StatusCode::CREATED);
    let delegated_body: Value = test::read_body_json(delegated_create).await;
    let org_id = delegated_body["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(delegated_body["data"]["member_count"], 1);
    assert_eq!(delegated_body["data"]["active_member_count"], 1);
    assert_eq!(delegated_body["data"]["members"][0]["user_id"], eve_id);
    assert_eq!(delegated_body["data"]["members"][0]["role"], "owner");
    assert_eq!(delegated_body["data"]["members"][0]["status"], "active");

    // Verify audit log for delegated creation records include_creator and initial_owner_email
    let audit_created = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/audit-logs?action=organisation.created")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(audit_created.status(), StatusCode::OK);
    let audit_created_body: Value = test::read_body_json(audit_created).await;
    assert_eq!(
        audit_created_body["data"]["items"][0]["details"]["include_creator"],
        false
    );
    assert_eq!(
        audit_created_body["data"]["items"][0]["details"]["initial_owner_email"],
        "eve@delegated.test"
    );

    // 2b. Passing both user_id and email to add_member is rejected with 422
    let ambiguous_add = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{org_id}/members"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .set_json(json!({
                "user_id": frank_id,
                "email": "frank@delegated.test"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(ambiguous_add.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 3. Self-service owner Eve cannot invite disabled account Gina (returns 404, no status oracle)
    let invite_disabled = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{org_id}/members"))
            .insert_header(("Authorization", format!("Bearer {eve_token}")))
            .set_json(json!({ "email": "gina@delegated.test" }))
            .to_request(),
    )
    .await;
    assert_eq!(invite_disabled.status(), StatusCode::NOT_FOUND);

    // 4. Eve invites Frank; Eve CANNOT promote pending (invited) Frank to owner (403)
    let invite_frank = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{org_id}/members"))
            .insert_header(("Authorization", format!("Bearer {eve_token}")))
            .set_json(json!({ "email": "frank@delegated.test" }))
            .to_request(),
    )
    .await;
    assert_eq!(invite_frank.status(), StatusCode::OK);

    let promote_invited = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!(
                "/api/v1/organisations/{org_id}/members/{frank_id}"
            ))
            .insert_header(("Authorization", format!("Bearer {eve_token}")))
            .set_json(json!({ "role": "owner" }))
            .to_request(),
    )
    .await;
    assert_eq!(promote_invited.status(), StatusCode::FORBIDDEN);

    // 5. Frank declines the invitation -> audit log records organisation.invitation_declined
    let decline_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{org_id}/leave"))
            .insert_header(("Authorization", format!("Bearer {frank_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(decline_resp.status(), StatusCode::OK);

    let audit_declined = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/audit-logs?action=organisation.invitation_declined")
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(audit_declined.status(), StatusCode::OK);
    let audit_body: Value = test::read_body_json(audit_declined).await;
    assert_eq!(audit_body["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        audit_body["data"]["items"][0]["action"],
        "organisation.invitation_declined"
    );

    // 6. Re-invite Frank, Frank accepts, Eve suspends Frank -> Frank's display name remains visible
    let reinvite_frank = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{org_id}/members"))
            .insert_header(("Authorization", format!("Bearer {eve_token}")))
            .set_json(json!({ "email": "frank@delegated.test" }))
            .to_request(),
    )
    .await;
    assert_eq!(reinvite_frank.status(), StatusCode::OK);

    let accept_frank = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/organisations/{org_id}/accept"))
            .insert_header(("Authorization", format!("Bearer {frank_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(accept_frank.status(), StatusCode::OK);

    let suspend_frank = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!(
                "/api/v1/organisations/{org_id}/members/{frank_id}"
            ))
            .insert_header(("Authorization", format!("Bearer {eve_token}")))
            .set_json(json!({ "status": "suspended" }))
            .to_request(),
    )
    .await;
    assert_eq!(suspend_frank.status(), StatusCode::OK);
    let suspended_body: Value = test::read_body_json(suspend_frank).await;
    let frank_member = suspended_body["data"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["user_id"] == frank_id)
        .unwrap();
    assert_eq!(frank_member["status"], "suspended");
    assert_eq!(frank_member["first_name"], "Frank");
    assert_eq!(frank_member["last_name"], "Member");

    // 7. GET /api/v1/users/{id}/organisations enforces RBAC and exposes active_member_count & active_owner_count
    let forbidden_user_orgs = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/users/{eve_id}/organisations"))
            .insert_header(("Authorization", format!("Bearer {eve_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(forbidden_user_orgs.status(), StatusCode::FORBIDDEN);

    let admin_user_orgs = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/users/{eve_id}/organisations"))
            .insert_header(("Authorization", format!("Bearer {sa_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(admin_user_orgs.status(), StatusCode::OK);
    let user_orgs_body: Value = test::read_body_json(admin_user_orgs).await;
    let items = user_orgs_body["data"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "Delegated LLC");
    assert_eq!(items[0]["member_count"], 2);
    assert_eq!(items[0]["active_member_count"], 1);
    assert_eq!(items[0]["active_owner_count"], 1);

    // 8. Deleting Frank's account pulls him from Delegated LLC and increments org.version
    //    so concurrent optimistic updates cannot re-insert a deleted user.
    let org_oid = mongodb::bson::oid::ObjectId::parse_str(&org_id).unwrap();
    let before_org = state
        .organisation_repo
        .find_by_id(org_oid)
        .await
        .unwrap()
        .unwrap();
    let frank_del = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/v1/profile")
            .insert_header(("Authorization", format!("Bearer {frank_token}")))
            .set_json(json!({ "current_password": "StrongPassword123!" }))
            .to_request(),
    )
    .await;
    assert_eq!(frank_del.status(), StatusCode::OK);
    let after_org = state
        .organisation_repo
        .find_by_id(org_oid)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_org.members.len(), 1);
    assert!(after_org.version > before_org.version);
    // Attempting to save the stale snapshot before Frank's deletion must fail with 409
    assert!(state.organisation_repo.update(&before_org).await.is_err());
}
