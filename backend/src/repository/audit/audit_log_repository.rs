use async_trait::async_trait;
use futures_util::TryStreamExt;
use mongodb::{
    Collection, Database,
    bson::{Document, doc, oid::ObjectId},
    options::FindOptions,
};

use crate::{
    common::{errors::AppError, pagination::SortOrder},
    repository::{
        audit::dao::audit_log::AuditLog, permission::permission_repository::regex_escape,
    },
};

pub const AUDIT_LOGS_COLLECTION: &str = "audit_logs";

#[derive(Debug, Clone, Default)]
pub struct AuditLogFilter {
    pub search: Option<String>,
    pub resource_type: Option<String>,
    pub action: Option<String>,
    pub actor_email: Option<String>,
    pub skip: u64,
    pub limit: u64,
    pub sort_order: Option<SortOrder>,
}

#[async_trait]
pub trait AuditLogRepository: Send + Sync {
    async fn create(&self, entry: &AuditLog) -> Result<AuditLog, AppError>;
    async fn find_by_id(&self, id: ObjectId) -> Result<Option<AuditLog>, AppError>;
    async fn find_all(&self, filter: &AuditLogFilter) -> Result<(Vec<AuditLog>, u64), AppError>;
    async fn count_all(&self) -> Result<u64, AppError>;
}

pub struct MongoAuditLogRepository {
    collection: Collection<AuditLog>,
}

impl MongoAuditLogRepository {
    pub fn new(db: &Database) -> Self {
        Self {
            collection: db.collection(AUDIT_LOGS_COLLECTION),
        }
    }
}

#[async_trait]
impl AuditLogRepository for MongoAuditLogRepository {
    async fn create(&self, entry: &AuditLog) -> Result<AuditLog, AppError> {
        self.collection.insert_one(entry).await?;
        Ok(entry.clone())
    }

    async fn find_by_id(&self, id: ObjectId) -> Result<Option<AuditLog>, AppError> {
        let result = self.collection.find_one(doc! { "_id": id }).await?;
        Ok(result)
    }

    async fn find_all(&self, filter: &AuditLogFilter) -> Result<(Vec<AuditLog>, u64), AppError> {
        let mut query = Document::new();

        if let Some(ref resource_type) = filter.resource_type {
            let trimmed = resource_type.trim();
            if !trimmed.is_empty() {
                query.insert("resource_type", trimmed);
            }
        }

        if let Some(ref action) = filter.action {
            let trimmed = action.trim();
            if !trimmed.is_empty() {
                query.insert("action", trimmed);
            }
        }

        if let Some(ref actor_email) = filter.actor_email {
            let trimmed = actor_email.trim();
            if !trimmed.is_empty() {
                let escaped = regex_escape(trimmed);
                query.insert("actor_email", doc! { "$regex": escaped, "$options": "i" });
            }
        }

        if let Some(ref search) = filter.search {
            let trimmed = search.trim();
            if !trimmed.is_empty() {
                let escaped = regex_escape(trimmed);
                query.insert(
                    "$or",
                    vec![
                        doc! { "action": { "$regex": &escaped, "$options": "i" } },
                        doc! { "resource_type": { "$regex": &escaped, "$options": "i" } },
                        doc! { "resource_name": { "$regex": &escaped, "$options": "i" } },
                        doc! { "actor_email": { "$regex": &escaped, "$options": "i" } },
                        doc! { "summary": { "$regex": &escaped, "$options": "i" } },
                    ],
                );
            }
        }

        let total = self.collection.count_documents(query.clone()).await?;
        let sort_dir = filter.sort_order.unwrap_or(SortOrder::Desc).as_mongo_dir();
        let limit = i64::try_from(filter.limit).unwrap_or(100);

        let options = FindOptions::builder()
            .skip(filter.skip)
            .limit(limit)
            .sort(doc! { "created_at": sort_dir, "_id": sort_dir })
            .build();

        let cursor = self.collection.find(query).with_options(options).await?;
        let items: Vec<AuditLog> = cursor.try_collect().await?;
        Ok((items, total))
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.collection.count_documents(doc! {}).await?)
    }
}
