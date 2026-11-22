use std::time::Duration;

use chrono::{DateTime, Utc};
use mongodb::{
    Client, Database, IndexModel,
    bson::{DateTime as BsonDateTime, Document, doc},
    options::{ClientOptions, Collation, CollationStrength, IndexOptions},
};
use tracing::{info, warn};

use crate::{
    common::errors::AppError,
    config::settings::MongoDbSettings,
    repository::{
        audit::audit_log_repository::AUDIT_LOGS_COLLECTION,
        auth::refresh_token_repository::REFRESH_TOKENS_COLLECTION,
        organisation::organisation_repository::ORGANISATIONS_COLLECTION,
        permission::permission_repository::PERMISSIONS_COLLECTION,
        role::role_repository::ROLES_COLLECTION, user::user_repository::USERS_COLLECTION,
    },
};

#[derive(Clone)]
pub struct MongoDb {
    pub client: Client,
    pub database: Database,
    pub supports_transactions: bool,
}

impl MongoDb {
    pub async fn connect(
        settings: &MongoDbSettings,
        audit_log_retention_days: u64,
        revoked_token_retention_seconds: u64,
    ) -> Result<Self, AppError> {
        let mut client_options = ClientOptions::parse(&settings.uri).await?;
        client_options.max_pool_size = Some(settings.max_pool_size);
        client_options.min_pool_size = Some(settings.min_pool_size);
        client_options.app_name = Some("auth-backend".to_string());

        let client = Client::with_options(client_options)?;
        let database = client.database(&settings.database);

        database.run_command(doc! { "ping": 1 }).await?;
        info!(database = %settings.database, "Connected to MongoDB");

        let supports_transactions = detect_transaction_support(&client).await;
        if supports_transactions {
            info!(
                "MongoDB replica set / sharded cluster detected: multi-document transactions enabled"
            );
        } else {
            warn!(
                "MongoDB standalone deployment detected: multi-document transactions unavailable; configure a replica set for production"
            );
        }

        let mongo = Self {
            client,
            database,
            supports_transactions,
        };
        mongo
            .ensure_indexes(audit_log_retention_days, revoked_token_retention_seconds)
            .await?;
        Ok(mongo)
    }

    pub async fn ping(&self) -> Result<(), AppError> {
        self.database.run_command(doc! { "ping": 1 }).await?;
        Ok(())
    }

    pub async fn ensure_indexes(
        &self,
        audit_log_retention_days: u64,
        revoked_token_retention_seconds: u64,
    ) -> Result<(), AppError> {
        // Users collection indexes
        let users = self.database.collection::<Document>(USERS_COLLECTION);
        users
            .create_indexes(vec![
                IndexModel::builder()
                    .keys(doc! { "email": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("email_unique".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "status": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("status_idx".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "roles": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("roles_idx".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "created_at": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("created_at_desc".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await?;

        // Roles collection indexes
        let roles = self.database.collection::<Document>(ROLES_COLLECTION);
        roles
            .create_indexes(vec![
                IndexModel::builder()
                    .keys(doc! { "name": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("role_name_unique".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "permissions": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("permissions_idx".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "is_system": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("is_system_idx".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await?;

        // Permissions collection indexes
        let permissions = self.database.collection::<Document>(PERMISSIONS_COLLECTION);
        permissions
            .create_indexes(vec![
                IndexModel::builder()
                    .keys(doc! { "name": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("permission_name_unique".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "resource": 1, "action": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("resource_action_unique".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "is_system": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("perm_is_system_idx".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await?;

        // Refresh tokens collection indexes (including TTL index for automatic expiration)
        let refresh_tokens = self
            .database
            .collection::<Document>(REFRESH_TOKENS_COLLECTION);
        refresh_tokens
            .create_indexes(vec![
                IndexModel::builder()
                    .keys(doc! { "token_hash": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("token_hash_unique".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("user_id_idx".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "family_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("family_id_idx".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "family_id": 1, "revoked_at": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("family_revoked_at_idx".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "expires_at": 1 })
                    .options(
                        IndexOptions::builder()
                            .expire_after(Duration::from_secs(0))
                            .name("expires_at_ttl".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await?;

        if revoked_token_retention_seconds > 0 {
            let ttl_index = IndexModel::builder()
                .keys(doc! { "revoked_at": 1 })
                .options(
                    IndexOptions::builder()
                        .expire_after(Duration::from_secs(revoked_token_retention_seconds))
                        .name("revoked_at_ttl".to_string())
                        .build(),
                )
                .build();

            if let Err(err) = refresh_tokens.create_index(ttl_index).await {
                let expire_i64 = i64::try_from(revoked_token_retention_seconds).unwrap_or(i64::MAX);
                let coll_mod = self
                    .database
                    .run_command(doc! {
                        "collMod": REFRESH_TOKENS_COLLECTION,
                        "index": {
                            "name": "revoked_at_ttl",
                            "expireAfterSeconds": expire_i64,
                        }
                    })
                    .await;
                if let Err(mod_err) = coll_mod {
                    warn!(
                        create_error = %err,
                        coll_mod_error = %mod_err,
                        "Failed to create or update refresh_tokens revoked_at TTL index"
                    );
                }
            }
        } else {
            let _ = refresh_tokens.drop_index("revoked_at_ttl").await;
        }

        // Organisations collection indexes
        let organisations = self
            .database
            .collection::<Document>(ORGANISATIONS_COLLECTION);
        let org_name_index = IndexModel::builder()
            .keys(doc! { "name": 1 })
            .options(
                IndexOptions::builder()
                    .unique(true)
                    .collation(case_insensitive_collation())
                    .name("org_name_unique".to_string())
                    .build(),
            )
            .build();
        if let Err(err) = organisations.create_index(org_name_index.clone()).await {
            // Upgrade legacy case-sensitive org_name_unique index to case-insensitive collation.
            let _ = organisations.drop_index("org_name_unique").await;
            if let Err(retry_err) = organisations.create_index(org_name_index).await {
                warn!(
                    initial_error = %err,
                    retry_error = %retry_err,
                    "Failed to create case-insensitive org_name_unique index"
                );
                return Err(retry_err.into());
            }
        }
        organisations
            .create_indexes(vec![
                IndexModel::builder()
                    .keys(doc! { "members.user_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("org_members_user_id_idx".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "created_at": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("org_created_at_desc".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await?;

        // Audit logs collection indexes
        let audit_logs = self.database.collection::<Document>(AUDIT_LOGS_COLLECTION);
        audit_logs
            .create_indexes(vec![
                IndexModel::builder()
                    .keys(doc! { "created_at": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("audit_created_at_desc".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "action": 1, "created_at": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("audit_action_created_at".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "resource_type": 1, "created_at": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("audit_resource_type_created_at".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "actor_id": 1, "created_at": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("audit_actor_id_created_at".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await?;

        if audit_log_retention_days > 0 {
            let expire_secs = audit_log_retention_days.saturating_mul(86_400);
            let ttl_index = IndexModel::builder()
                .keys(doc! { "created_at": 1 })
                .options(
                    IndexOptions::builder()
                        .expire_after(Duration::from_secs(expire_secs))
                        .name("audit_created_at_ttl".to_string())
                        .build(),
                )
                .build();

            if let Err(err) = audit_logs.create_index(ttl_index).await {
                let expire_i64 = i64::try_from(expire_secs).unwrap_or(i64::MAX);
                let coll_mod = self
                    .database
                    .run_command(doc! {
                        "collMod": AUDIT_LOGS_COLLECTION,
                        "index": {
                            "name": "audit_created_at_ttl",
                            "expireAfterSeconds": expire_i64,
                        }
                    })
                    .await;
                if let Err(mod_err) = coll_mod {
                    warn!(
                        create_error = %err,
                        coll_mod_error = %mod_err,
                        "Failed to create or update audit_logs TTL index"
                    );
                }
            }
        } else {
            let _ = audit_logs.drop_index("audit_created_at_ttl").await;
        }

        info!("MongoDB indexes verified");
        Ok(())
    }
}

pub fn case_insensitive_collation() -> Collation {
    Collation::builder()
        .locale("en")
        .strength(CollationStrength::Secondary)
        .build()
}

async fn detect_transaction_support(client: &Client) -> bool {
    let admin = client.database("admin");
    match admin.run_command(doc! { "hello": 1 }).await {
        Ok(doc) => doc.contains_key("setName") || doc.get_str("msg").ok() == Some("isdbgrid"),
        Err(_) => false,
    }
}

pub fn bson_dt_to_chrono(dt: BsonDateTime) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(dt.timestamp_millis()).unwrap_or_else(Utc::now)
}

pub fn chrono_to_bson_dt(dt: DateTime<Utc>) -> BsonDateTime {
    BsonDateTime::from_millis(dt.timestamp_millis())
}
