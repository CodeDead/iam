use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RefreshTokenTransport {
    #[default]
    Cookie,
    Body,
}

impl RefreshTokenTransport {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cookie => "cookie",
            Self::Body => "body",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub user_id: ObjectId,
    pub token_hash: String,
    pub family_id: String,
    #[serde(default)]
    pub transport: RefreshTokenTransport,
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
