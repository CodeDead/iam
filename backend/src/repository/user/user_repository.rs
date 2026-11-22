use std::collections::HashMap;

use async_trait::async_trait;
use futures_util::TryStreamExt;
use mongodb::{
    Collection, Database,
    bson::{DateTime as BsonDateTime, Document, doc, oid::ObjectId},
    options::{FindOneAndUpdateOptions, FindOptions, ReturnDocument},
};

use crate::{
    common::{errors::AppError, pagination::SortOrder},
    repository::{
        permission::permission_repository::regex_escape,
        user::dao::user::{AccountStatus, User},
    },
};

pub const USERS_COLLECTION: &str = "users";

#[derive(Debug, Clone, Default)]
pub struct UserFilter {
    pub search: Option<String>,
    pub status: Option<AccountStatus>,
    pub skip: u64,
    pub limit: u64,
    pub sort_by: String,
    pub sort_order: Option<SortOrder>,
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn create(&self, user: &User) -> Result<User, AppError>;
    async fn find_by_id(&self, id: ObjectId) -> Result<Option<User>, AppError>;
    async fn find_by_ids(&self, ids: &[ObjectId]) -> Result<Vec<User>, AppError>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, AppError>;
    async fn find_by_role_id(&self, role_id: ObjectId) -> Result<Vec<User>, AppError>;
    async fn find_all(&self, filter: &UserFilter) -> Result<(Vec<User>, u64), AppError>;
    async fn update(&self, user: &User) -> Result<User, AppError>;
    async fn record_login(&self, id: ObjectId, now: BsonDateTime)
    -> Result<Option<User>, AppError>;
    async fn update_password(
        &self,
        id: ObjectId,
        password_hash: &str,
        now: BsonDateTime,
    ) -> Result<bool, AppError>;
    async fn delete(&self, id: ObjectId) -> Result<bool, AppError>;
    async fn remove_role_from_all_users(&self, role_id: ObjectId) -> Result<(), AppError>;
    async fn count_active_users_with_role(&self, role_id: ObjectId) -> Result<u64, AppError>;
    async fn count_by_role_id(&self, role_id: ObjectId) -> Result<u64, AppError>;
    async fn count_by_roles(&self) -> Result<HashMap<ObjectId, u64>, AppError>;
    async fn count_all(&self) -> Result<u64, AppError>;
    async fn count_by_status(&self, status: AccountStatus) -> Result<u64, AppError>;
    async fn count_created_since(&self, since: BsonDateTime) -> Result<u64, AppError>;
}

pub struct MongoUserRepository {
    collection: Collection<User>,
}

impl MongoUserRepository {
    pub fn new(db: &Database) -> Self {
        Self {
            collection: db.collection(USERS_COLLECTION),
        }
    }
}

#[async_trait]
impl UserRepository for MongoUserRepository {
    async fn create(&self, user: &User) -> Result<User, AppError> {
        self.collection.insert_one(user).await?;
        Ok(user.clone())
    }

    async fn find_by_id(&self, id: ObjectId) -> Result<Option<User>, AppError> {
        Ok(self.collection.find_one(doc! { "_id": id }).await?)
    }

    async fn find_by_ids(&self, ids: &[ObjectId]) -> Result<Vec<User>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let cursor = self.collection.find(doc! { "_id": { "$in": ids } }).await?;
        let users: Vec<User> = cursor.try_collect().await?;
        Ok(users)
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, AppError> {
        Ok(self.collection.find_one(doc! { "email": email }).await?)
    }

    async fn find_by_role_id(&self, role_id: ObjectId) -> Result<Vec<User>, AppError> {
        let options = FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .limit(500)
            .build();
        let cursor = self
            .collection
            .find(doc! { "roles": role_id })
            .with_options(options)
            .await?;
        let users: Vec<User> = cursor.try_collect().await?;
        Ok(users)
    }

    async fn find_all(&self, filter: &UserFilter) -> Result<(Vec<User>, u64), AppError> {
        let query = build_user_filter_query(filter);

        let total = self.collection.count_documents(query.clone()).await?;
        let sort_field = match filter.sort_by.as_str() {
            "first_name" | "last_name" | "email" | "status" | "created_at" | "updated_at"
            | "last_login_at" => filter.sort_by.as_str(),
            _ => "created_at",
        };
        let sort_dir = filter.sort_order.unwrap_or(SortOrder::Desc).as_mongo_dir();
        let limit = i64::try_from(filter.limit).unwrap_or(100);

        let options = FindOptions::builder()
            .skip(filter.skip)
            .limit(limit)
            .sort(doc! { sort_field: sort_dir })
            .build();

        let cursor = self.collection.find(query).with_options(options).await?;
        let items: Vec<User> = cursor.try_collect().await?;
        Ok((items, total))
    }

    async fn update(&self, user: &User) -> Result<User, AppError> {
        self.collection
            .replace_one(doc! { "_id": user.id }, user)
            .await?;
        Ok(user.clone())
    }

    async fn record_login(
        &self,
        id: ObjectId,
        now: BsonDateTime,
    ) -> Result<Option<User>, AppError> {
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();
        let updated = self
            .collection
            .find_one_and_update(
                doc! { "_id": id, "status": AccountStatus::Active.as_str() },
                doc! { "$set": { "last_login_at": now, "updated_at": now } },
            )
            .with_options(options)
            .await?;
        Ok(updated)
    }

    async fn update_password(
        &self,
        id: ObjectId,
        password_hash: &str,
        now: BsonDateTime,
    ) -> Result<bool, AppError> {
        let res = self
            .collection
            .update_one(
                doc! { "_id": id, "status": AccountStatus::Active.as_str() },
                doc! {
                    "$set": {
                        "password_hash": password_hash,
                        "updated_at": now,
                    },
                    "$inc": { "token_version": 1_i64 }
                },
            )
            .await?;
        Ok(res.matched_count > 0)
    }

    async fn delete(&self, id: ObjectId) -> Result<bool, AppError> {
        let res = self.collection.delete_one(doc! { "_id": id }).await?;
        Ok(res.deleted_count > 0)
    }

    async fn remove_role_from_all_users(&self, role_id: ObjectId) -> Result<(), AppError> {
        self.collection
            .update_many(
                doc! { "roles": role_id },
                doc! {
                    "$pull": { "roles": role_id },
                    "$set": { "updated_at": BsonDateTime::now() }
                },
            )
            .await?;
        Ok(())
    }

    async fn count_active_users_with_role(&self, role_id: ObjectId) -> Result<u64, AppError> {
        let count = self
            .collection
            .count_documents(doc! {
                "roles": role_id,
                "status": AccountStatus::Active.as_str()
            })
            .await?;
        Ok(count)
    }

    async fn count_by_role_id(&self, role_id: ObjectId) -> Result<u64, AppError> {
        Ok(self
            .collection
            .count_documents(doc! { "roles": role_id })
            .await?)
    }

    async fn count_by_roles(&self) -> Result<HashMap<ObjectId, u64>, AppError> {
        let pipeline = vec![
            doc! { "$unwind": "$roles" },
            doc! { "$group": { "_id": "$roles", "count": { "$sum": 1 } } },
        ];
        let cursor = self.collection.aggregate(pipeline).await?;
        let docs: Vec<Document> = cursor.try_collect().await?;
        let mut counts = HashMap::with_capacity(docs.len());
        for d in docs {
            if let Ok(role_id) = d.get_object_id("_id") {
                let count = d
                    .get_i64("count")
                    .ok()
                    .and_then(|v| u64::try_from(v).ok())
                    .or_else(|| d.get_i32("count").ok().and_then(|v| u64::try_from(v).ok()))
                    .unwrap_or(0);
                counts.insert(role_id, count);
            }
        }
        Ok(counts)
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.collection.count_documents(doc! {}).await?)
    }

    async fn count_by_status(&self, status: AccountStatus) -> Result<u64, AppError> {
        Ok(self
            .collection
            .count_documents(doc! { "status": status.as_str() })
            .await?)
    }

    async fn count_created_since(&self, since: BsonDateTime) -> Result<u64, AppError> {
        Ok(self
            .collection
            .count_documents(doc! { "created_at": { "$gte": since } })
            .await?)
    }
}

const MAX_SEARCH_TERMS: usize = 10;

pub(crate) fn build_user_filter_query(filter: &UserFilter) -> Document {
    let mut query = Document::new();
    if let Some(status) = filter.status {
        query.insert("status", status.as_str());
    }
    if let Some(ref search) = filter.search {
        let terms: Vec<&str> = search.split_whitespace().take(MAX_SEARCH_TERMS).collect();
        match terms.as_slice() {
            [] => {}
            [single] => {
                let escaped = regex_escape(single);
                query.insert(
                    "$or",
                    vec![
                        doc! { "email": { "$regex": &escaped, "$options": "i" } },
                        doc! { "first_name": { "$regex": &escaped, "$options": "i" } },
                        doc! { "last_name": { "$regex": &escaped, "$options": "i" } },
                    ],
                );
            }
            multi => {
                let term_clauses: Vec<Document> = multi
                    .iter()
                    .map(|term| {
                        let escaped = regex_escape(term);
                        doc! {
                            "$or": [
                                { "email": { "$regex": &escaped, "$options": "i" } },
                                { "first_name": { "$regex": &escaped, "$options": "i" } },
                                { "last_name": { "$regex": &escaped, "$options": "i" } },
                            ]
                        }
                    })
                    .collect();
                query.insert("$and", term_clauses);
            }
        }
    }
    query
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_multi_term_and_query_for_full_name_search() {
        let filter = UserFilter {
            search: Some("  John   Doe  ".to_string()),
            ..Default::default()
        };
        let query = build_user_filter_query(&filter);
        let and_clauses = query.get_array("$and").ok();
        assert_eq!(and_clauses.map(Vec::len), Some(2));
    }

    #[test]
    fn builds_single_or_query_for_single_term_search() {
        let filter = UserFilter {
            search: Some("John".to_string()),
            ..Default::default()
        };
        let query = build_user_filter_query(&filter);
        assert!(query.contains_key("$or"));
        assert!(!query.contains_key("$and"));
    }
}
