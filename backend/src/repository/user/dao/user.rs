use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    Active,
    Disabled,
    Locked,
    PendingVerification,
}

impl AccountStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
            Self::Locked => "locked",
            Self::PendingVerification => "pending_verification",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct UserProfileInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub department: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub password_hash: String,
    pub status: AccountStatus,
    pub email_verified: bool,
    pub roles: Vec<ObjectId>,
    #[serde(default)]
    pub token_version: u64,
    #[serde(default)]
    pub profile: UserProfileInfo,
    pub created_at: BsonDateTime,
    pub updated_at: BsonDateTime,
    #[serde(default)]
    pub last_login_at: Option<BsonDateTime>,
}
