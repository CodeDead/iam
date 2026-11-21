#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::env;

use auth_backend::{app::AppState, config::Settings};

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

    // Verify seeded system RBAC entities exist in MongoDB
    assert!(state.permission_repo.count_all().await.unwrap() >= 16);
    assert!(state.role_repo.count_all().await.unwrap() >= 4);
    assert!(
        state
            .user_repo
            .find_by_email("sa@mongo-test.local")
            .await
            .unwrap()
            .is_some()
    );

    // Cleanup ephemeral test database
    db.database.drop().await.unwrap();
}
