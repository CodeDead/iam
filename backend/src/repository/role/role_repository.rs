use async_trait::async_trait;
use futures_util::TryStreamExt;
use mongodb::{
    Collection, Database,
    bson::{DateTime as BsonDateTime, Document, doc, oid::ObjectId},
    options::FindOptions,
};

use crate::{
    common::{errors::AppError, pagination::SortOrder},
    repository::{permission::permission_repository::regex_escape, role::dao::role::Role},
};

pub const ROLES_COLLECTION: &str = "roles";

#[derive(Debug, Clone, Default)]
pub struct RoleFilter {
    pub search: Option<String>,
    pub skip: u64,
    pub limit: u64,
    pub sort_by: String,
    pub sort_order: Option<SortOrder>,
}

#[async_trait]
pub trait RoleRepository: Send + Sync {
    async fn create(&self, role: &Role) -> Result<Role, AppError>;
    async fn find_by_id(&self, id: ObjectId) -> Result<Option<Role>, AppError>;
    async fn find_by_ids(&self, ids: &[ObjectId]) -> Result<Vec<Role>, AppError>;
    async fn find_by_name(&self, name: &str) -> Result<Option<Role>, AppError>;
    async fn find_by_permission_id(&self, permission_id: ObjectId) -> Result<Vec<Role>, AppError>;
    async fn find_all(&self, filter: &RoleFilter) -> Result<(Vec<Role>, u64), AppError>;
    async fn update(&self, role: &Role) -> Result<Role, AppError>;
    async fn delete(&self, id: ObjectId) -> Result<bool, AppError>;
    async fn remove_permission_from_all_roles(
        &self,
        permission_id: ObjectId,
    ) -> Result<(), AppError>;
    async fn count_all(&self) -> Result<u64, AppError>;
    async fn count_by_system(&self, is_system: bool) -> Result<u64, AppError>;
}

pub struct MongoRoleRepository {
    collection: Collection<Role>,
}

impl MongoRoleRepository {
    pub fn new(db: &Database) -> Self {
        Self {
            collection: db.collection(ROLES_COLLECTION),
        }
    }
}

#[async_trait]
impl RoleRepository for MongoRoleRepository {
    async fn create(&self, role: &Role) -> Result<Role, AppError> {
        self.collection.insert_one(role).await?;
        Ok(role.clone())
    }

    async fn find_by_id(&self, id: ObjectId) -> Result<Option<Role>, AppError> {
        Ok(self.collection.find_one(doc! { "_id": id }).await?)
    }

    async fn find_by_ids(&self, ids: &[ObjectId]) -> Result<Vec<Role>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let cursor = self.collection.find(doc! { "_id": { "$in": ids } }).await?;
        let roles: Vec<Role> = cursor.try_collect().await?;
        Ok(roles)
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Role>, AppError> {
        Ok(self.collection.find_one(doc! { "name": name }).await?)
    }

    async fn find_by_permission_id(&self, permission_id: ObjectId) -> Result<Vec<Role>, AppError> {
        let cursor = self
            .collection
            .find(doc! { "permissions": permission_id })
            .await?;
        let roles: Vec<Role> = cursor.try_collect().await?;
        Ok(roles)
    }

    async fn find_all(&self, filter: &RoleFilter) -> Result<(Vec<Role>, u64), AppError> {
        let mut query = Document::new();
        if let Some(ref search) = filter.search {
            let trimmed = search.trim();
            if !trimmed.is_empty() {
                let escaped = regex_escape(trimmed);
                query.insert(
                    "$or",
                    vec![
                        doc! { "name": { "$regex": &escaped, "$options": "i" } },
                        doc! { "description": { "$regex": &escaped, "$options": "i" } },
                    ],
                );
            }
        }

        let total = self.collection.count_documents(query.clone()).await?;
        let sort_field = match filter.sort_by.as_str() {
            "name" | "created_at" | "updated_at" | "is_system" => filter.sort_by.as_str(),
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
        let items: Vec<Role> = cursor.try_collect().await?;
        Ok((items, total))
    }

    async fn update(&self, role: &Role) -> Result<Role, AppError> {
        self.collection
            .replace_one(doc! { "_id": role.id }, role)
            .await?;
        Ok(role.clone())
    }

    async fn delete(&self, id: ObjectId) -> Result<bool, AppError> {
        let res = self.collection.delete_one(doc! { "_id": id }).await?;
        Ok(res.deleted_count > 0)
    }

    async fn remove_permission_from_all_roles(
        &self,
        permission_id: ObjectId,
    ) -> Result<(), AppError> {
        self.collection
            .update_many(
                doc! { "permissions": permission_id },
                doc! {
                    "$pull": { "permissions": permission_id },
                    "$set": { "updated_at": BsonDateTime::now() }
                },
            )
            .await?;
        Ok(())
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.collection.count_documents(doc! {}).await?)
    }

    async fn count_by_system(&self, is_system: bool) -> Result<u64, AppError> {
        Ok(self
            .collection
            .count_documents(doc! { "is_system": is_system })
            .await?)
    }
}
