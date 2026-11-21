use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub user_id: ObjectId,
    pub token_hash: String,
    pub family_id: String,
    pub expires_at: BsonDateTime,
    #[serde(default)]
    pub revoked_at: Option<BsonDateTime>,
    #[serde(default)]
    pub replaced_by_hash: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub ip_address: Option<String>,
    pub created_at: BsonDateTime,
}

impl RefreshToken {
    pub fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }

    pub fn is_expired(&self, now: BsonDateTime) -> bool {
        self.expires_at.timestamp_millis() <= now.timestamp_millis()
    }
}
