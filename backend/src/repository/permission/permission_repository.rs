use async_trait::async_trait;
use futures_util::TryStreamExt;
use mongodb::{
    Collection, Database,
    bson::{Document, doc, oid::ObjectId},
    options::FindOptions,
};

use crate::{
    common::{errors::AppError, pagination::SortOrder},
    repository::permission::dao::permission::Permission,
};

pub const PERMISSIONS_COLLECTION: &str = "permissions";

#[derive(Debug, Clone, Default)]
pub struct PermissionFilter {
    pub search: Option<String>,
    pub resource: Option<String>,
    pub skip: u64,
    pub limit: u64,
    pub sort_by: String,
    pub sort_order: Option<SortOrder>,
}

#[async_trait]
pub trait PermissionRepository: Send + Sync {
    async fn create(&self, permission: &Permission) -> Result<Permission, AppError>;
    async fn find_by_id(&self, id: ObjectId) -> Result<Option<Permission>, AppError>;
    async fn find_by_ids(&self, ids: &[ObjectId]) -> Result<Vec<Permission>, AppError>;
    async fn find_by_name(&self, name: &str) -> Result<Option<Permission>, AppError>;
    async fn find_all(&self, filter: &PermissionFilter)
    -> Result<(Vec<Permission>, u64), AppError>;
    async fn update(&self, permission: &Permission) -> Result<Permission, AppError>;
    async fn delete(&self, id: ObjectId) -> Result<bool, AppError>;
    async fn count_all(&self) -> Result<u64, AppError>;
}

pub struct MongoPermissionRepository {
    collection: Collection<Permission>,
}

impl MongoPermissionRepository {
    pub fn new(db: &Database) -> Self {
        Self {
            collection: db.collection(PERMISSIONS_COLLECTION),
        }
    }
}

#[async_trait]
impl PermissionRepository for MongoPermissionRepository {
    async fn create(&self, permission: &Permission) -> Result<Permission, AppError> {
        self.collection.insert_one(permission).await?;
        Ok(permission.clone())
    }

    async fn find_by_id(&self, id: ObjectId) -> Result<Option<Permission>, AppError> {
        let result = self.collection.find_one(doc! { "_id": id }).await?;
        Ok(result)
    }

    async fn find_by_ids(&self, ids: &[ObjectId]) -> Result<Vec<Permission>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let cursor = self.collection.find(doc! { "_id": { "$in": ids } }).await?;
        let items: Vec<Permission> = cursor.try_collect().await?;
        Ok(items)
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Permission>, AppError> {
        let result = self.collection.find_one(doc! { "name": name }).await?;
        Ok(result)
    }

    async fn find_all(
        &self,
        filter: &PermissionFilter,
    ) -> Result<(Vec<Permission>, u64), AppError> {
        let mut query = Document::new();
        if let Some(ref resource) = filter.resource {
            let trimmed = resource.trim();
            if !trimmed.is_empty() {
                query.insert("resource", trimmed);
            }
        }
        if let Some(ref search) = filter.search {
            let trimmed = search.trim();
            if !trimmed.is_empty() {
                let escaped = regex_escape(trimmed);
                query.insert(
                    "$or",
                    vec![
                        doc! { "name": { "$regex": &escaped, "$options": "i" } },
                        doc! { "description": { "$regex": &escaped, "$options": "i" } },
                        doc! { "resource": { "$regex": &escaped, "$options": "i" } },
                    ],
                );
            }
        }

        let total = self.collection.count_documents(query.clone()).await?;

        let sort_field = match filter.sort_by.as_str() {
            "name" | "resource" | "action" | "created_at" | "updated_at" => filter.sort_by.as_str(),
            _ => "name",
        };
        let sort_dir = filter.sort_order.unwrap_or(SortOrder::Asc).as_mongo_dir();

        let limit = i64::try_from(filter.limit).unwrap_or(100);
        let options = FindOptions::builder()
            .skip(filter.skip)
            .limit(limit)
            .sort(doc! { sort_field: sort_dir })
            .build();

        let cursor = self.collection.find(query).with_options(options).await?;
        let items: Vec<Permission> = cursor.try_collect().await?;
        Ok((items, total))
    }

    async fn update(&self, permission: &Permission) -> Result<Permission, AppError> {
        self.collection
            .replace_one(doc! { "_id": permission.id }, permission)
            .await?;
        Ok(permission.clone())
    }

    async fn delete(&self, id: ObjectId) -> Result<bool, AppError> {
        let res = self.collection.delete_one(doc! { "_id": id }).await?;
        Ok(res.deleted_count > 0)
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.collection.count_documents(doc! {}).await?)
    }
}

pub fn regex_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for ch in input.chars() {
        if matches!(
            ch,
            '\\' | '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}
