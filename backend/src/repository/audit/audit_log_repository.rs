use async_trait::async_trait;
use futures_util::TryStreamExt;
use mongodb::{
    Collection, Database,
    bson::{DateTime as BsonDateTime, Document, doc, oid::ObjectId},
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

#[derive(Debug, Clone)]
pub struct DailyAuditBucket {
    pub date: String,
    pub total: u64,
    pub auth_success: u64,
    pub auth_failed: u64,
    pub admin_changes: u64,
}

#[async_trait]
pub trait AuditLogRepository: Send + Sync {
    async fn create(&self, entry: &AuditLog) -> Result<AuditLog, AppError>;
    async fn find_by_id(&self, id: ObjectId) -> Result<Option<AuditLog>, AppError>;
    async fn find_all(&self, filter: &AuditLogFilter) -> Result<(Vec<AuditLog>, u64), AppError>;
    async fn count_all(&self) -> Result<u64, AppError>;
    async fn count_since(&self, since: BsonDateTime) -> Result<u64, AppError>;
    async fn count_action_since(&self, action: &str, since: BsonDateTime) -> Result<u64, AppError>;
    async fn aggregate_daily_activity(
        &self,
        since: BsonDateTime,
    ) -> Result<Vec<DailyAuditBucket>, AppError>;
    async fn count_by_resource_type_since(
        &self,
        since: BsonDateTime,
    ) -> Result<Vec<(String, u64)>, AppError>;
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

    async fn count_since(&self, since: BsonDateTime) -> Result<u64, AppError> {
        Ok(self
            .collection
            .count_documents(doc! { "created_at": { "$gte": since } })
            .await?)
    }

    async fn count_action_since(&self, action: &str, since: BsonDateTime) -> Result<u64, AppError> {
        Ok(self
            .collection
            .count_documents(doc! {
                "action": action,
                "created_at": { "$gte": since }
            })
            .await?)
    }

    async fn aggregate_daily_activity(
        &self,
        since: BsonDateTime,
    ) -> Result<Vec<DailyAuditBucket>, AppError> {
        let pipeline = vec![
            doc! { "$match": { "created_at": { "$gte": since } } },
            doc! {
                "$group": {
                    "_id": {
                        "$dateToString": {
                            "format": "%Y-%m-%d",
                            "date": "$created_at"
                        }
                    },
                    "total": { "$sum": 1 },
                    "auth_success": {
                        "$sum": {
                            "$cond": [
                                { "$in": ["$action", ["auth.login", "auth.registered"]] },
                                1,
                                0
                            ]
                        }
                    },
                    "auth_failed": {
                        "$sum": {
                            "$cond": [
                                { "$in": ["$action", ["auth.login_failed", "auth.refresh_token_reused"]] },
                                1,
                                0
                            ]
                        }
                    },
                    "admin_changes": {
                        "$sum": {
                            "$cond": [
                                { "$in": ["$resource_type", ["user", "role", "permission"]] },
                                1,
                                0
                            ]
                        }
                    }
                }
            },
            doc! { "$sort": { "_id": 1 } },
            doc! { "$limit": 31 },
        ];

        let cursor = self.collection.aggregate(pipeline).await?;
        let docs: Vec<Document> = cursor.try_collect().await?;
        let mut buckets = Vec::with_capacity(docs.len());
        for d in docs {
            if let Ok(date) = d.get_str("_id") {
                buckets.push(DailyAuditBucket {
                    date: date.to_string(),
                    total: bson_count_to_u64(&d, "total"),
                    auth_success: bson_count_to_u64(&d, "auth_success"),
                    auth_failed: bson_count_to_u64(&d, "auth_failed"),
                    admin_changes: bson_count_to_u64(&d, "admin_changes"),
                });
            }
        }
        Ok(buckets)
    }

    async fn count_by_resource_type_since(
        &self,
        since: BsonDateTime,
    ) -> Result<Vec<(String, u64)>, AppError> {
        let pipeline = vec![
            doc! { "$match": { "created_at": { "$gte": since } } },
            doc! { "$group": { "_id": "$resource_type", "count": { "$sum": 1 } } },
            doc! { "$sort": { "count": -1, "_id": 1 } },
            doc! { "$limit": 10 },
        ];

        let cursor = self.collection.aggregate(pipeline).await?;
        let docs: Vec<Document> = cursor.try_collect().await?;
        let mut results = Vec::with_capacity(docs.len());
        for d in docs {
            if let Ok(resource_type) = d.get_str("_id") {
                results.push((resource_type.to_string(), bson_count_to_u64(&d, "count")));
            }
        }
        Ok(results)
    }
}

fn bson_count_to_u64(doc: &Document, field: &str) -> u64 {
    doc.get_i64(field)
        .ok()
        .and_then(|v| u64::try_from(v).ok())
        .or_else(|| doc.get_i32(field).ok().and_then(|v| u64::try_from(v).ok()))
        .unwrap_or(0)
}
