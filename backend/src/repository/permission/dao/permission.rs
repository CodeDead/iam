use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub name: String,
    pub description: String,
    pub resource: String,
    pub action: String,
    pub is_system: bool,
    pub created_at: BsonDateTime,
    pub updated_at: BsonDateTime,
}
