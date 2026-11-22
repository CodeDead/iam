#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::env;

use auth_backend::{
    app::AppState,
    config::Settings,
    database::seed::SYSTEM_PERMISSIONS,
    repository::{
        auth::dao::refresh_token::RefreshToken,
        organisation::dao::organisation::{
            Organisation, OrganisationMember, OrganisationMemberRole, OrganisationMemberStatus,
        },
    },
};
use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};

/// Full MongoDB repository & index integration test.
/// Runs automatically when `MONGODB_TEST_URI` is set in the environment, e.g.:
/// `MONGODB_TEST_URI=mongodb://localhost:27017 cargo test --test mongodb_integration_tests`
#[tokio::test]
async fn mongodb_repositories_indexes_and_seeding() {
    let Ok(uri) = env::var("MONGODB_TEST_URI") else {
        eprintln!("Skipping MongoDB integration test (set MONGODB_TEST_URI to enable)");
        return;
    };

    let mut settings = Settings::default();
    settings.mongodb.uri = uri;
    settings.mongodb.database = format!("user_mgmt_test_{}", uuid::Uuid::new_v4().simple());
    settings.bootstrap.super_admin_email = "sa@mongo-test.local".to_string();
    settings.bootstrap.super_admin_password = "MongoTest123!".to_string();

    let state = AppState::from_mongodb(settings).await.unwrap();
    let db = state.mongodb.as_ref().unwrap();
    db.ping().await.unwrap();

    // 1. Verify seeded system RBAC entities exist in MongoDB
    assert!(
        state.permission_repo.count_all().await.unwrap() >= SYSTEM_PERMISSIONS.len() as u64,
        "Expected all system permissions to be seeded"
    );
    assert!(state.role_repo.count_all().await.unwrap() >= 4);
    let sa = state
        .user_repo
        .find_by_email("sa@mongo-test.local")
        .await
        .unwrap()
        .expect("Super Administrator should be bootstrapped");

    // 2. Verify UserRepository batch lookup, atomic record_login, and count_by_roles
    let batch_users = state.user_repo.find_by_ids(&[sa.id]).await.unwrap();
    assert_eq!(batch_users.len(), 1);
    let logged_in = state
        .user_repo
        .record_login(sa.id, BsonDateTime::now())
        .await
        .unwrap();
    assert!(logged_in.is_some());
    let role_counts = state.user_repo.count_by_roles().await.unwrap();
    assert!(!role_counts.is_empty());

    // 3. Verify OrganisationRepository case-insensitive unique index, find_by_name,
    //    optimistic concurrency control, membership_counts, and backfill_missing_owners
    let now = BsonDateTime::now();
    let org = Organisation {
        id: ObjectId::new(),
        name: "Mongo Integration Org".to_string(),
        members: vec![OrganisationMember {
            user_id: sa.id,
            role: OrganisationMemberRole::Member,
            status: OrganisationMemberStatus::Active,
            joined_at: now,
        }],
        version: 0,
        created_at: now,
        updated_at: now,
    };
    let created = state.organisation_repo.create(&org).await.unwrap();
    assert!(
        state
            .organisation_repo
            .find_by_name("mongo integration org")
            .await
            .unwrap()
            .is_some()
    );

    // Case-insensitive duplicate must fail with 409 Conflict at the database index level
    let dup = Organisation {
        id: ObjectId::new(),
        name: "MONGO INTEGRATION ORG".to_string(),
        members: vec![],
        version: 0,
        created_at: now,
        updated_at: now,
    };
    assert!(state.organisation_repo.create(&dup).await.is_err());

    // Backfill missing owner promotes the sole active member to Owner
    let repaired = state
        .organisation_repo
        .backfill_missing_owners()
        .await
        .unwrap();
    assert_eq!(repaired, 1);

    // Stale version update must fail with CONCURRENT_MODIFICATION
    let stale_err = state.organisation_repo.update(&created).await;
    assert!(stale_err.is_err());

    let counts = state.organisation_repo.membership_counts().await.unwrap();
    assert_eq!(counts.active, 1);

    // remove_member_from_all_organisations must also increment version for optimistic concurrency
    let latest_org = state
        .organisation_repo
        .find_by_id(created.id)
        .await
        .unwrap()
        .unwrap();
    state
        .organisation_repo
        .remove_member_from_all_organisations(sa.id)
        .await
        .unwrap();
    let after_pull = state
        .organisation_repo
        .find_by_id(created.id)
        .await
        .unwrap()
        .unwrap();
    assert!(after_pull.members.is_empty());
    assert!(after_pull.version > latest_org.version);
    assert!(state.organisation_repo.update(&latest_org).await.is_err());

    // 4. Verify RefreshTokenRepository rotation & reuse rejection
    let token_a = RefreshToken {
        id: ObjectId::new(),
        user_id: sa.id,
        token_hash: "hash_a".to_string(),
        family_id: "fam_mongo".to_string(),
        expires_at: BsonDateTime::from_millis(now.timestamp_millis() + 60_000),
        revoked_at: None,
        replaced_by_hash: None,
        user_agent: None,
        ip_address: None,
        created_at: now,
    };
    state.refresh_token_repo.create(&token_a).await.unwrap();

    let token_b = RefreshToken {
        id: ObjectId::new(),
        token_hash: "hash_b".to_string(),
        ..token_a.clone()
    };
    assert!(
        state
            .refresh_token_repo
            .rotate("hash_a", &token_b)
            .await
            .unwrap()
    );
    // Reusing already-rotated token_a returns Ok(false)
    let token_c = RefreshToken {
        id: ObjectId::new(),
        token_hash: "hash_c".to_string(),
        ..token_a
    };
    assert!(
        !state
            .refresh_token_repo
            .rotate("hash_a", &token_c)
            .await
            .unwrap()
    );

    // Cleanup ephemeral test database
    db.database.drop().await.unwrap();
}
