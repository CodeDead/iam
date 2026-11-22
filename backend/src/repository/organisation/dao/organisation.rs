use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrganisationMemberStatus {
    Active,
    Invited,
    Suspended,
}

impl OrganisationMemberStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Invited => "invited",
            Self::Suspended => "suspended",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrganisationMemberRole {
    Owner,
    Admin,
    #[default]
    Member,
}

impl OrganisationMemberRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    pub fn can_manage_members(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganisationMember {
    pub user_id: ObjectId,
    #[serde(default)]
    pub role: OrganisationMemberRole,
    pub status: OrganisationMemberStatus,
    pub joined_at: BsonDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organisation {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub name: String,
    #[serde(default)]
    pub members: Vec<OrganisationMember>,
    #[serde(default)]
    pub version: u64,
    pub created_at: BsonDateTime,
    #[serde(default = "BsonDateTime::now")]
    pub updated_at: BsonDateTime,
}
