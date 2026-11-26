use async_trait::async_trait;
use futures_util::TryStreamExt;
use mongodb::{
    Collection, Database,
    bson::{DateTime as BsonDateTime, Document, doc, oid::ObjectId},
    options::FindOptions,
};

use crate::{
    common::{errors::AppError, pagination::SortOrder},
    database::mongodb::case_insensitive_collation,
    repository::{
        organisation::dao::organisation::{Organisation, OrganisationMemberStatus},
        permission::permission_repository::regex_escape,
    },
};

pub const ORGANISATIONS_COLLECTION: &str = "organisations";

#[derive(Debug, Clone, Default)]
pub struct OrganisationFilter {
    pub search: Option<String>,
    pub skip: u64,
    pub limit: u64,
    pub sort_by: String,
    pub sort_order: Option<SortOrder>,
}

#[derive(Debug, Clone, Default)]
pub struct OrganisationMembershipCounts {
    pub total: u64,
    pub active: u64,
    pub invited: u64,
    pub suspended: u64,
}

#[async_trait]
pub trait OrganisationRepository: Send + Sync {
    async fn create(&self, organisation: &Organisation) -> Result<Organisation, AppError>;
    async fn find_by_id(&self, id: ObjectId) -> Result<Option<Organisation>, AppError>;
    async fn find_by_name(&self, name: &str) -> Result<Option<Organisation>, AppError>;
    async fn find_by_member_user_id(
        &self,
        user_id: ObjectId,
    ) -> Result<Vec<Organisation>, AppError>;
    async fn find_all(
        &self,
        filter: &OrganisationFilter,
    ) -> Result<(Vec<Organisation>, u64), AppError>;
    async fn update(&self, organisation: &Organisation) -> Result<Organisation, AppError>;
    async fn delete(&self, id: ObjectId) -> Result<bool, AppError>;
    async fn remove_member_from_all_organisations(&self, user_id: ObjectId)
    -> Result<(), AppError>;
    async fn count_all(&self) -> Result<u64, AppError>;
    async fn count_created_since(&self, since: BsonDateTime) -> Result<u64, AppError>;
    async fn membership_counts(&self) -> Result<OrganisationMembershipCounts, AppError>;
    async fn find_largest(&self, limit: u64) -> Result<Vec<Organisation>, AppError>;
}

pub struct MongoOrganisationRepository {
    collection: Collection<Organisation>,
}

impl MongoOrganisationRepository {
    pub fn new(db: &Database) -> Self {
        Self {
            collection: db.collection(ORGANISATIONS_COLLECTION),
        }
    }
}

#[async_trait]
impl OrganisationRepository for MongoOrganisationRepository {
    async fn create(&self, organisation: &Organisation) -> Result<Organisation, AppError> {
        self.collection.insert_one(organisation).await?;
        Ok(organisation.clone())
    }

    async fn find_by_id(&self, id: ObjectId) -> Result<Option<Organisation>, AppError> {
        Ok(self.collection.find_one(doc! { "_id": id }).await?)
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Organisation>, AppError> {
        let trimmed = name.trim();
        Ok(self
            .collection
            .find_one(doc! { "name": trimmed })
            .collation(case_insensitive_collation())
            .await?)
    }

    async fn find_by_member_user_id(
        &self,
        user_id: ObjectId,
    ) -> Result<Vec<Organisation>, AppError> {
        let options = FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .limit(1000)
            .build();
        let cursor = self
            .collection
            .find(doc! { "members.user_id": user_id })
            .with_options(options)
            .await?;
        let items: Vec<Organisation> = cursor.try_collect().await?;
        Ok(items)
    }

    async fn find_all(
        &self,
        filter: &OrganisationFilter,
    ) -> Result<(Vec<Organisation>, u64), AppError> {
        let mut query = Document::new();
        if let Some(ref search) = filter.search {
            let trimmed = search.trim();
            if !trimmed.is_empty() {
                let escaped = regex_escape(trimmed);
                query.insert("name", doc! { "$regex": escaped, "$options": "i" });
            }
        }

        let total = self.collection.count_documents(query.clone()).await?;
        let sort_field = match filter.sort_by.as_str() {
            "name" | "created_at" | "updated_at" => filter.sort_by.as_str(),
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
        let items: Vec<Organisation> = cursor.try_collect().await?;
        Ok((items, total))
    }

    async fn update(&self, organisation: &Organisation) -> Result<Organisation, AppError> {
        let expected_version = organisation.version;
        let mut next = organisation.clone();
        next.version = expected_version.saturating_add(1);

        let expected_i64 = i64::try_from(expected_version).unwrap_or(i64::MAX);
        let filter = if expected_version == 0 {
            doc! {
                "_id": organisation.id,
                "$or": [
                    { "version": expected_i64 },
                    { "version": { "$exists": false } }
                ]
            }
        } else {
            doc! {
                "_id": organisation.id,
                "version": expected_i64
            }
        };

        let result = self.collection.replace_one(filter, &next).await?;
        if result.matched_count == 0 {
            return Err(AppError::conflict(
                "CONCURRENT_MODIFICATION",
                "Organisation was modified concurrently by another request; please retry",
            ));
        }
        Ok(next)
    }

    async fn delete(&self, id: ObjectId) -> Result<bool, AppError> {
        let res = self.collection.delete_one(doc! { "_id": id }).await?;
        Ok(res.deleted_count > 0)
    }

    async fn remove_member_from_all_organisations(
        &self,
        user_id: ObjectId,
    ) -> Result<(), AppError> {
        self.collection
            .update_many(
                doc! { "members.user_id": user_id },
                doc! {
                    "$pull": { "members": { "user_id": user_id } },
                    "$set": { "updated_at": BsonDateTime::now() },
                    "$inc": { "version": 1 }
                },
            )
            .await?;
        Ok(())
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.collection.count_documents(doc! {}).await?)
    }

    async fn count_created_since(&self, since: BsonDateTime) -> Result<u64, AppError> {
        Ok(self
            .collection
            .count_documents(doc! { "created_at": { "$gte": since } })
            .await?)
    }

    async fn membership_counts(&self) -> Result<OrganisationMembershipCounts, AppError> {
        let pipeline = vec![
            doc! { "$unwind": "$members" },
            doc! { "$group": { "_id": "$members.status", "count": { "$sum": 1 } } },
        ];
        let cursor = self.collection.aggregate(pipeline).await?;
        let docs: Vec<Document> = cursor.try_collect().await?;
        let mut counts = OrganisationMembershipCounts::default();
        for d in docs {
            let count = d
                .get_i64("count")
                .ok()
                .and_then(|v| u64::try_from(v).ok())
                .or_else(|| d.get_i32("count").ok().and_then(|v| u64::try_from(v).ok()))
                .unwrap_or(0);
            counts.total = counts.total.saturating_add(count);
            if let Ok(status) = d.get_str("_id") {
                if status == OrganisationMemberStatus::Active.as_str() {
                    counts.active = counts.active.saturating_add(count);
                } else if status == OrganisationMemberStatus::Invited.as_str() {
                    counts.invited = counts.invited.saturating_add(count);
                } else if status == OrganisationMemberStatus::Suspended.as_str() {
                    counts.suspended = counts.suspended.saturating_add(count);
                }
            }
        }
        Ok(counts)
    }

    async fn find_largest(&self, limit: u64) -> Result<Vec<Organisation>, AppError> {
        let limit_i64 = i64::try_from(limit).unwrap_or(8);
        let pipeline = vec![
            doc! {
                "$addFields": {
                    "_member_count": { "$size": { "$ifNull": ["$members", []] } }
                }
            },
            doc! { "$sort": { "_member_count": -1, "name": 1 } },
            doc! { "$limit": limit_i64 },
            doc! { "$project": { "_member_count": 0 } },
        ];
        let cursor = self.collection.aggregate(pipeline).await?;
        let docs: Vec<Document> = cursor.try_collect().await?;
        let mut orgs = Vec::with_capacity(docs.len());
        for d in docs {
            if let Ok(org) = mongodb::bson::from_document::<Organisation>(d) {
                orgs.push(org);
            }
        }
        Ok(orgs)
    }
}
