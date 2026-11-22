use async_trait::async_trait;
use futures_util::TryStreamExt;
use mongodb::{
    Client, Collection, Database,
    bson::{DateTime as BsonDateTime, doc, oid::ObjectId},
    options::FindOptions,
};
use tracing::warn;

use crate::{common::errors::AppError, repository::auth::dao::refresh_token::RefreshToken};

pub const REFRESH_TOKENS_COLLECTION: &str = "refresh_tokens";

#[async_trait]
pub trait RefreshTokenRepository: Send + Sync {
    async fn create(&self, token: &RefreshToken) -> Result<(), AppError>;
    async fn find_by_hash(&self, token_hash: &str) -> Result<Option<RefreshToken>, AppError>;
    async fn rotate(
        &self,
        old_token_hash: &str,
        new_token: &RefreshToken,
    ) -> Result<bool, AppError>;
    async fn revoke_by_hash(&self, token_hash: &str) -> Result<(), AppError>;
    async fn revoke_family(&self, family_id: &str) -> Result<(), AppError>;
    async fn revoke_all_for_user(&self, user_id: ObjectId) -> Result<(), AppError>;
    async fn prune_revoked_in_family(
        &self,
        family_id: &str,
        keep_latest: usize,
    ) -> Result<(), AppError>;
    async fn delete_family(&self, family_id: &str) -> Result<(), AppError>;
    async fn delete_all_for_user(&self, user_id: ObjectId) -> Result<(), AppError>;
    async fn count_for_family(&self, family_id: &str) -> Result<u64, AppError>;
    async fn count_active_all(&self) -> Result<u64, AppError>;
    async fn count_active_for_user(&self, user_id: ObjectId) -> Result<u64, AppError>;
}

pub struct MongoRefreshTokenRepository {
    client: Client,
    collection: Collection<RefreshToken>,
    supports_transactions: bool,
}

impl MongoRefreshTokenRepository {
    pub fn new(client: Client, db: &Database, supports_transactions: bool) -> Self {
        Self {
            client,
            collection: db.collection(REFRESH_TOKENS_COLLECTION),
            supports_transactions,
        }
    }
}

#[async_trait]
impl RefreshTokenRepository for MongoRefreshTokenRepository {
    async fn create(&self, token: &RefreshToken) -> Result<(), AppError> {
        self.collection.insert_one(token).await?;
        Ok(())
    }

    async fn find_by_hash(&self, token_hash: &str) -> Result<Option<RefreshToken>, AppError> {
        Ok(self
            .collection
            .find_one(doc! { "token_hash": token_hash })
            .await?)
    }

    async fn rotate(
        &self,
        old_token_hash: &str,
        new_token: &RefreshToken,
    ) -> Result<bool, AppError> {
        let now = BsonDateTime::now();

        if self.supports_transactions {
            let mut session = self.client.start_session().await?;
            if let Err(err) = session.start_transaction().await {
                warn!(error = %err, "Failed to start MongoDB transaction; falling back to sequential rotation");
            } else {
                let update_res = self
                    .collection
                    .update_one(
                        doc! {
                            "token_hash": old_token_hash,
                            "revoked_at": null,
                        },
                        doc! {
                            "$set": {
                                "revoked_at": now,
                                "replaced_by_hash": &new_token.token_hash,
                            }
                        },
                    )
                    .session(&mut session)
                    .await?;

                if update_res.modified_count == 0 {
                    let _ = session.abort_transaction().await;
                    return Ok(false);
                }

                self.collection
                    .insert_one(new_token)
                    .session(&mut session)
                    .await?;
                session.commit_transaction().await?;
                return Ok(true);
            }
        }

        // Atomic conditional update ensures only one concurrent refresh can consume old_token_hash.
        let update_res = self
            .collection
            .update_one(
                doc! {
                    "token_hash": old_token_hash,
                    "revoked_at": null,
                },
                doc! {
                    "$set": {
                        "revoked_at": now,
                        "replaced_by_hash": &new_token.token_hash,
                    }
                },
            )
            .await?;

        if update_res.modified_count == 0 {
            return Ok(false);
        }

        self.collection.insert_one(new_token).await?;
        Ok(true)
    }

    async fn revoke_by_hash(&self, token_hash: &str) -> Result<(), AppError> {
        self.collection
            .update_one(
                doc! { "token_hash": token_hash, "revoked_at": null },
                doc! { "$set": { "revoked_at": BsonDateTime::now() } },
            )
            .await?;
        Ok(())
    }

    async fn revoke_family(&self, family_id: &str) -> Result<(), AppError> {
        self.collection
            .update_many(
                doc! { "family_id": family_id, "revoked_at": null },
                doc! { "$set": { "revoked_at": BsonDateTime::now() } },
            )
            .await?;
        Ok(())
    }

    async fn revoke_all_for_user(&self, user_id: ObjectId) -> Result<(), AppError> {
        self.collection
            .update_many(
                doc! { "user_id": user_id, "revoked_at": null },
                doc! { "$set": { "revoked_at": BsonDateTime::now() } },
            )
            .await?;
        Ok(())
    }

    async fn prune_revoked_in_family(
        &self,
        family_id: &str,
        keep_latest: usize,
    ) -> Result<(), AppError> {
        let skip = u64::try_from(keep_latest).unwrap_or(5);
        let options = FindOptions::builder()
            .sort(doc! { "revoked_at": -1, "_id": -1 })
            .skip(skip)
            .build();

        let cursor = self
            .collection
            .find(doc! { "family_id": family_id, "revoked_at": { "$ne": null } })
            .with_options(options)
            .await?;

        let stale_ids: Vec<ObjectId> = cursor
            .try_collect::<Vec<RefreshToken>>()
            .await?
            .into_iter()
            .map(|t| t.id)
            .collect();

        if !stale_ids.is_empty() {
            self.collection
                .delete_many(doc! { "_id": { "$in": stale_ids } })
                .await?;
        }
        Ok(())
    }

    async fn delete_family(&self, family_id: &str) -> Result<(), AppError> {
        self.collection
            .delete_many(doc! { "family_id": family_id })
            .await?;
        Ok(())
    }

    async fn delete_all_for_user(&self, user_id: ObjectId) -> Result<(), AppError> {
        self.collection
            .delete_many(doc! { "user_id": user_id })
            .await?;
        Ok(())
    }

    async fn count_for_family(&self, family_id: &str) -> Result<u64, AppError> {
        Ok(self
            .collection
            .count_documents(doc! { "family_id": family_id })
            .await?)
    }

    async fn count_active_all(&self) -> Result<u64, AppError> {
        let now = BsonDateTime::now();
        Ok(self
            .collection
            .count_documents(doc! {
                "revoked_at": null,
                "expires_at": { "$gt": now }
            })
            .await?)
    }

    async fn count_active_for_user(&self, user_id: ObjectId) -> Result<u64, AppError> {
        let now = BsonDateTime::now();
        Ok(self
            .collection
            .count_documents(doc! {
                "user_id": user_id,
                "revoked_at": null,
                "expires_at": { "$gt": now }
            })
            .await?)
    }
}
