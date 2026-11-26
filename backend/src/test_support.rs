#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use tokio::sync::RwLock;

use crate::{
    app::AppState,
    common::{errors::AppError, pagination::SortOrder},
    config::Settings,
    database::{mongodb::bson_dt_to_chrono, seed::seed_system_rbac},
    repository::{
        audit::{
            audit_log_repository::{AuditLogFilter, AuditLogRepository, DailyAuditBucket},
            dao::audit_log::AuditLog,
        },
        auth::{
            dao::refresh_token::RefreshToken, refresh_token_repository::RefreshTokenRepository,
        },
        organisation::{
            dao::organisation::{Organisation, OrganisationMemberStatus},
            organisation_repository::{
                OrganisationFilter, OrganisationMembershipCounts, OrganisationRepository,
            },
        },
        permission::{
            dao::permission::Permission,
            permission_repository::{PermissionFilter, PermissionRepository},
        },
        role::{
            dao::role::Role,
            role_repository::{RoleFilter, RoleRepository},
        },
        user::{
            dao::user::{AccountStatus, User},
            user_repository::{UserFilter, UserRepository},
        },
    },
    security::{jwt::JwtService, keys::JwtKeyPair},
};

#[derive(Default)]
pub struct InMemoryUserRepository {
    users: RwLock<HashMap<ObjectId, User>>,
}

#[async_trait]
impl UserRepository for InMemoryUserRepository {
    async fn create(&self, user: &User) -> Result<User, AppError> {
        let mut guard = self.users.write().await;
        if guard.values().any(|u| u.email == user.email) {
            return Err(AppError::conflict(
                "DUPLICATE_RESOURCE",
                "Email already exists",
            ));
        }
        guard.insert(user.id, user.clone());
        Ok(user.clone())
    }

    async fn find_by_id(&self, id: ObjectId) -> Result<Option<User>, AppError> {
        Ok(self.users.read().await.get(&id).cloned())
    }

    async fn find_by_ids(&self, ids: &[ObjectId]) -> Result<Vec<User>, AppError> {
        let guard = self.users.read().await;
        Ok(ids.iter().filter_map(|id| guard.get(id).cloned()).collect())
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, AppError> {
        Ok(self
            .users
            .read()
            .await
            .values()
            .find(|u| u.email == email)
            .cloned())
    }

    async fn find_by_role_id(&self, role_id: ObjectId) -> Result<Vec<User>, AppError> {
        let mut items: Vec<User> = self
            .users
            .read()
            .await
            .values()
            .filter(|u| u.roles.contains(&role_id))
            .cloned()
            .collect();
        items.sort_by(|a, b| {
            b.created_at
                .timestamp_millis()
                .cmp(&a.created_at.timestamp_millis())
        });
        items.truncate(500);
        Ok(items)
    }

    async fn find_all(&self, filter: &UserFilter) -> Result<(Vec<User>, u64), AppError> {
        let guard = self.users.read().await;
        let mut items: Vec<User> = guard
            .values()
            .filter(|u| {
                if let Some(status) = filter.status
                    && u.status != status
                {
                    return false;
                }
                if let Some(ref search) = filter.search {
                    let email_lower = u.email.to_lowercase();
                    let first_lower = u.first_name.to_lowercase();
                    let last_lower = u.last_name.to_lowercase();
                    let matches_all = search.split_whitespace().take(10).all(|term| {
                        let t = term.to_lowercase();
                        email_lower.contains(&t)
                            || first_lower.contains(&t)
                            || last_lower.contains(&t)
                    });
                    if !matches_all {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        items.sort_by(|a, b| {
            let ord = match filter.sort_by.as_str() {
                "email" => a.email.cmp(&b.email),
                "first_name" => a.first_name.cmp(&b.first_name),
                "last_name" => a.last_name.cmp(&b.last_name),
                "status" => a.status.as_str().cmp(b.status.as_str()),
                "updated_at" => a
                    .updated_at
                    .timestamp_millis()
                    .cmp(&b.updated_at.timestamp_millis()),
                "last_login_at" => a
                    .last_login_at
                    .map(|d| d.timestamp_millis())
                    .cmp(&b.last_login_at.map(|d| d.timestamp_millis())),
                _ => a
                    .created_at
                    .timestamp_millis()
                    .cmp(&b.created_at.timestamp_millis()),
            };
            if matches!(filter.sort_order, Some(SortOrder::Desc) | None) {
                ord.reverse()
            } else {
                ord
            }
        });

        let total = items.len() as u64;
        let start = usize::try_from(filter.skip).unwrap_or(0);
        let limit = usize::try_from(filter.limit).unwrap_or(100);
        let page = items.into_iter().skip(start).take(limit).collect();
        Ok((page, total))
    }

    async fn update(&self, user: &User) -> Result<User, AppError> {
        let mut guard = self.users.write().await;
        if guard
            .values()
            .any(|u| u.id != user.id && u.email == user.email)
        {
            return Err(AppError::conflict(
                "DUPLICATE_RESOURCE",
                "Email already exists",
            ));
        }
        if guard.contains_key(&user.id) {
            guard.insert(user.id, user.clone());
        }
        Ok(user.clone())
    }

    async fn record_login(
        &self,
        id: ObjectId,
        now: BsonDateTime,
    ) -> Result<Option<User>, AppError> {
        let mut guard = self.users.write().await;
        if let Some(user) = guard.get_mut(&id)
            && user.status == AccountStatus::Active
        {
            user.last_login_at = Some(now);
            user.updated_at = now;
            return Ok(Some(user.clone()));
        }
        Ok(None)
    }

    async fn update_password(
        &self,
        id: ObjectId,
        password_hash: &str,
        now: BsonDateTime,
    ) -> Result<bool, AppError> {
        let mut guard = self.users.write().await;
        if let Some(user) = guard.get_mut(&id)
            && user.status == AccountStatus::Active
        {
            user.password_hash = password_hash.to_string();
            user.token_version = user.token_version.saturating_add(1);
            user.updated_at = now;
            return Ok(true);
        }
        Ok(false)
    }

    async fn delete(&self, id: ObjectId) -> Result<bool, AppError> {
        Ok(self.users.write().await.remove(&id).is_some())
    }

    async fn remove_role_from_all_users(&self, role_id: ObjectId) -> Result<(), AppError> {
        let mut guard = self.users.write().await;
        let now = BsonDateTime::now();
        for user in guard.values_mut() {
            let before = user.roles.len();
            user.roles.retain(|r| *r != role_id);
            if user.roles.len() != before {
                user.updated_at = now;
            }
        }
        Ok(())
    }

    async fn count_active_users_with_role(&self, role_id: ObjectId) -> Result<u64, AppError> {
        let count = self
            .users
            .read()
            .await
            .values()
            .filter(|u| u.status == AccountStatus::Active && u.roles.contains(&role_id))
            .count() as u64;
        Ok(count)
    }

    async fn count_by_role_id(&self, role_id: ObjectId) -> Result<u64, AppError> {
        Ok(self
            .users
            .read()
            .await
            .values()
            .filter(|u| u.roles.contains(&role_id))
            .count() as u64)
    }

    async fn count_by_roles(&self) -> Result<HashMap<ObjectId, u64>, AppError> {
        let guard = self.users.read().await;
        let mut counts: HashMap<ObjectId, u64> = HashMap::new();
        for user in guard.values() {
            for role_id in &user.roles {
                *counts.entry(*role_id).or_insert(0) += 1;
            }
        }
        Ok(counts)
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.users.read().await.len() as u64)
    }

    async fn count_by_status(&self, status: AccountStatus) -> Result<u64, AppError> {
        Ok(self
            .users
            .read()
            .await
            .values()
            .filter(|u| u.status == status)
            .count() as u64)
    }

    async fn count_created_since(&self, since: BsonDateTime) -> Result<u64, AppError> {
        let since_ms = since.timestamp_millis();
        Ok(self
            .users
            .read()
            .await
            .values()
            .filter(|u| u.created_at.timestamp_millis() >= since_ms)
            .count() as u64)
    }
}

#[derive(Default)]
pub struct InMemoryRoleRepository {
    roles: RwLock<HashMap<ObjectId, Role>>,
}

#[async_trait]
impl RoleRepository for InMemoryRoleRepository {
    async fn create(&self, role: &Role) -> Result<Role, AppError> {
        let mut guard = self.roles.write().await;
        if guard.values().any(|r| r.name == role.name) {
            return Err(AppError::conflict(
                "DUPLICATE_RESOURCE",
                "Role already exists",
            ));
        }
        guard.insert(role.id, role.clone());
        Ok(role.clone())
    }

    async fn find_by_id(&self, id: ObjectId) -> Result<Option<Role>, AppError> {
        Ok(self.roles.read().await.get(&id).cloned())
    }

    async fn find_by_ids(&self, ids: &[ObjectId]) -> Result<Vec<Role>, AppError> {
        let guard = self.roles.read().await;
        Ok(ids.iter().filter_map(|id| guard.get(id).cloned()).collect())
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Role>, AppError> {
        Ok(self
            .roles
            .read()
            .await
            .values()
            .find(|r| r.name == name)
            .cloned())
    }

    async fn find_by_permission_id(&self, permission_id: ObjectId) -> Result<Vec<Role>, AppError> {
        Ok(self
            .roles
            .read()
            .await
            .values()
            .filter(|r| r.permissions.contains(&permission_id))
            .cloned()
            .collect())
    }

    async fn find_all(&self, filter: &RoleFilter) -> Result<(Vec<Role>, u64), AppError> {
        let guard = self.roles.read().await;
        let mut items: Vec<Role> = guard
            .values()
            .filter(|r| {
                if let Some(ref search) = filter.search {
                    let s = search.trim().to_lowercase();
                    if !s.is_empty()
                        && !r.name.to_lowercase().contains(&s)
                        && !r.description.to_lowercase().contains(&s)
                    {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        items.sort_by(|a, b| {
            let ord = match filter.sort_by.as_str() {
                "created_at" => a
                    .created_at
                    .timestamp_millis()
                    .cmp(&b.created_at.timestamp_millis()),
                "updated_at" => a
                    .updated_at
                    .timestamp_millis()
                    .cmp(&b.updated_at.timestamp_millis()),
                _ => a.name.cmp(&b.name),
            };
            if matches!(filter.sort_order, Some(SortOrder::Desc)) {
                ord.reverse()
            } else {
                ord
            }
        });

        let total = items.len() as u64;
        let start = usize::try_from(filter.skip).unwrap_or(0);
        let limit = usize::try_from(filter.limit).unwrap_or(100);
        let page = items.into_iter().skip(start).take(limit).collect();
        Ok((page, total))
    }

    async fn update(&self, role: &Role) -> Result<Role, AppError> {
        let mut guard = self.roles.write().await;
        if guard.contains_key(&role.id) {
            guard.insert(role.id, role.clone());
        }
        Ok(role.clone())
    }

    async fn delete(&self, id: ObjectId) -> Result<bool, AppError> {
        Ok(self.roles.write().await.remove(&id).is_some())
    }

    async fn remove_permission_from_all_roles(
        &self,
        permission_id: ObjectId,
    ) -> Result<(), AppError> {
        let mut guard = self.roles.write().await;
        let now = BsonDateTime::now();
        for role in guard.values_mut() {
            let before = role.permissions.len();
            role.permissions.retain(|p| *p != permission_id);
            if role.permissions.len() != before {
                role.updated_at = now;
            }
        }
        Ok(())
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.roles.read().await.len() as u64)
    }

    async fn count_by_system(&self, is_system: bool) -> Result<u64, AppError> {
        Ok(self
            .roles
            .read()
            .await
            .values()
            .filter(|r| r.is_system == is_system)
            .count() as u64)
    }
}

#[derive(Default)]
pub struct InMemoryPermissionRepository {
    permissions: RwLock<HashMap<ObjectId, Permission>>,
}

#[async_trait]
impl PermissionRepository for InMemoryPermissionRepository {
    async fn create(&self, permission: &Permission) -> Result<Permission, AppError> {
        let mut guard = self.permissions.write().await;
        if guard.values().any(|p| p.name == permission.name) {
            return Err(AppError::conflict(
                "DUPLICATE_RESOURCE",
                "Permission already exists",
            ));
        }
        guard.insert(permission.id, permission.clone());
        Ok(permission.clone())
    }

    async fn find_by_id(&self, id: ObjectId) -> Result<Option<Permission>, AppError> {
        Ok(self.permissions.read().await.get(&id).cloned())
    }

    async fn find_by_ids(&self, ids: &[ObjectId]) -> Result<Vec<Permission>, AppError> {
        let guard = self.permissions.read().await;
        Ok(ids.iter().filter_map(|id| guard.get(id).cloned()).collect())
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Permission>, AppError> {
        Ok(self
            .permissions
            .read()
            .await
            .values()
            .find(|p| p.name == name)
            .cloned())
    }

    async fn find_all(
        &self,
        filter: &PermissionFilter,
    ) -> Result<(Vec<Permission>, u64), AppError> {
        let guard = self.permissions.read().await;
        let mut items: Vec<Permission> = guard
            .values()
            .filter(|p| {
                if let Some(ref resource) = filter.resource
                    && !resource.trim().is_empty()
                    && p.resource != resource.trim()
                {
                    return false;
                }
                if let Some(ref search) = filter.search {
                    let s = search.trim().to_lowercase();
                    if !s.is_empty()
                        && !p.name.to_lowercase().contains(&s)
                        && !p.description.to_lowercase().contains(&s)
                    {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        items.sort_by(|a, b| {
            let ord = match filter.sort_by.as_str() {
                "resource" => a.resource.cmp(&b.resource),
                "action" => a.action.cmp(&b.action),
                "created_at" => a
                    .created_at
                    .timestamp_millis()
                    .cmp(&b.created_at.timestamp_millis()),
                "updated_at" => a
                    .updated_at
                    .timestamp_millis()
                    .cmp(&b.updated_at.timestamp_millis()),
                _ => a.name.cmp(&b.name),
            };
            if matches!(filter.sort_order, Some(SortOrder::Desc)) {
                ord.reverse()
            } else {
                ord
            }
        });
        let total = items.len() as u64;
        let start = usize::try_from(filter.skip).unwrap_or(0);
        let limit = usize::try_from(filter.limit).unwrap_or(100);
        let page = items.into_iter().skip(start).take(limit).collect();
        Ok((page, total))
    }

    async fn update(&self, permission: &Permission) -> Result<Permission, AppError> {
        let mut guard = self.permissions.write().await;
        if guard.contains_key(&permission.id) {
            guard.insert(permission.id, permission.clone());
        }
        Ok(permission.clone())
    }

    async fn delete(&self, id: ObjectId) -> Result<bool, AppError> {
        Ok(self.permissions.write().await.remove(&id).is_some())
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.permissions.read().await.len() as u64)
    }

    async fn count_by_system(&self, is_system: bool) -> Result<u64, AppError> {
        Ok(self
            .permissions
            .read()
            .await
            .values()
            .filter(|p| p.is_system == is_system)
            .count() as u64)
    }

    async fn count_by_resource(&self) -> Result<Vec<(String, u64)>, AppError> {
        let guard = self.permissions.read().await;
        let mut counts: HashMap<String, u64> = HashMap::new();
        for p in guard.values() {
            *counts.entry(p.resource.clone()).or_insert(0) += 1;
        }
        let mut list: Vec<(String, u64)> = counts.into_iter().collect();
        list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        list.truncate(25);
        Ok(list)
    }
}

#[derive(Default)]
pub struct InMemoryRefreshTokenRepository {
    tokens: RwLock<HashMap<String, RefreshToken>>,
}

#[async_trait]
impl RefreshTokenRepository for InMemoryRefreshTokenRepository {
    async fn create(&self, token: &RefreshToken) -> Result<(), AppError> {
        let mut guard = self.tokens.write().await;
        if guard.contains_key(&token.token_hash) {
            return Err(AppError::conflict(
                "DUPLICATE_RESOURCE",
                "A resource with the provided unique value already exists",
            ));
        }
        guard.insert(token.token_hash.clone(), token.clone());
        Ok(())
    }

    async fn find_by_hash(&self, token_hash: &str) -> Result<Option<RefreshToken>, AppError> {
        Ok(self.tokens.read().await.get(token_hash).cloned())
    }

    async fn rotate(
        &self,
        old_token_hash: &str,
        new_token: &RefreshToken,
    ) -> Result<bool, AppError> {
        let mut guard = self.tokens.write().await;
        let Some(existing) = guard.get_mut(old_token_hash) else {
            return Ok(false);
        };
        if existing.revoked_at.is_some() {
            return Ok(false);
        }
        existing.revoked_at = Some(BsonDateTime::now());
        existing.replaced_by_hash = Some(new_token.token_hash.clone());
        guard.insert(new_token.token_hash.clone(), new_token.clone());
        Ok(true)
    }

    async fn revoke_by_hash(&self, token_hash: &str) -> Result<(), AppError> {
        if let Some(existing) = self.tokens.write().await.get_mut(token_hash)
            && existing.revoked_at.is_none()
        {
            existing.revoked_at = Some(BsonDateTime::now());
        }
        Ok(())
    }

    async fn revoke_family(&self, family_id: &str) -> Result<(), AppError> {
        let now = BsonDateTime::now();
        let mut guard = self.tokens.write().await;
        for token in guard.values_mut() {
            if token.family_id == family_id && token.revoked_at.is_none() {
                token.revoked_at = Some(now);
            }
        }
        Ok(())
    }

    async fn revoke_all_for_user(&self, user_id: ObjectId) -> Result<(), AppError> {
        let now = BsonDateTime::now();
        let mut guard = self.tokens.write().await;
        for token in guard.values_mut() {
            if token.user_id == user_id && token.revoked_at.is_none() {
                token.revoked_at = Some(now);
            }
        }
        Ok(())
    }

    async fn prune_revoked_in_family(
        &self,
        family_id: &str,
        keep_latest: usize,
    ) -> Result<(), AppError> {
        let mut guard = self.tokens.write().await;
        let mut revoked: Vec<(String, i64, ObjectId)> = guard
            .iter()
            .filter_map(|(hash, token)| {
                if token.family_id == family_id
                    && let Some(revoked_at) = token.revoked_at
                {
                    Some((hash.clone(), revoked_at.timestamp_millis(), token.id))
                } else {
                    None
                }
            })
            .collect();

        revoked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));
        for (hash, _, _) in revoked.into_iter().skip(keep_latest) {
            guard.remove(&hash);
        }
        Ok(())
    }

    async fn delete_family(&self, family_id: &str) -> Result<(), AppError> {
        self.tokens
            .write()
            .await
            .retain(|_, token| token.family_id != family_id);
        Ok(())
    }

    async fn delete_all_for_user(&self, user_id: ObjectId) -> Result<(), AppError> {
        self.tokens
            .write()
            .await
            .retain(|_, token| token.user_id != user_id);
        Ok(())
    }

    async fn count_for_family(&self, family_id: &str) -> Result<u64, AppError> {
        Ok(self
            .tokens
            .read()
            .await
            .values()
            .filter(|t| t.family_id == family_id)
            .count() as u64)
    }

    async fn count_active_all(&self) -> Result<u64, AppError> {
        let now_ms = BsonDateTime::now().timestamp_millis();
        Ok(self
            .tokens
            .read()
            .await
            .values()
            .filter(|t| t.revoked_at.is_none() && t.expires_at.timestamp_millis() > now_ms)
            .count() as u64)
    }

    async fn count_active_for_user(&self, user_id: ObjectId) -> Result<u64, AppError> {
        let now_ms = BsonDateTime::now().timestamp_millis();
        Ok(self
            .tokens
            .read()
            .await
            .values()
            .filter(|t| {
                t.user_id == user_id
                    && t.revoked_at.is_none()
                    && t.expires_at.timestamp_millis() > now_ms
            })
            .count() as u64)
    }
}

#[derive(Default)]
pub struct InMemoryAuditLogRepository {
    entries: RwLock<Vec<AuditLog>>,
}

#[async_trait]
impl AuditLogRepository for InMemoryAuditLogRepository {
    async fn create(&self, entry: &AuditLog) -> Result<AuditLog, AppError> {
        self.entries.write().await.push(entry.clone());
        Ok(entry.clone())
    }

    async fn find_by_id(&self, id: ObjectId) -> Result<Option<AuditLog>, AppError> {
        Ok(self
            .entries
            .read()
            .await
            .iter()
            .find(|e| e.id == id)
            .cloned())
    }

    async fn find_all(&self, filter: &AuditLogFilter) -> Result<(Vec<AuditLog>, u64), AppError> {
        let guard = self.entries.read().await;
        let mut items: Vec<AuditLog> = guard
            .iter()
            .filter(|e| {
                if let Some(ref rt) = filter.resource_type
                    && !rt.trim().is_empty()
                    && e.resource_type != rt.trim()
                {
                    return false;
                }
                if let Some(ref action) = filter.action
                    && !action.trim().is_empty()
                    && e.action != action.trim()
                {
                    return false;
                }
                if let Some(ref actor_email) = filter.actor_email {
                    let needle = actor_email.trim().to_lowercase();
                    if !needle.is_empty()
                        && !e
                            .actor_email
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&needle)
                    {
                        return false;
                    }
                }
                if let Some(ref search) = filter.search {
                    let s = search.trim().to_lowercase();
                    if !s.is_empty()
                        && !e.action.to_lowercase().contains(&s)
                        && !e.resource_type.to_lowercase().contains(&s)
                        && !e
                            .resource_name
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&s)
                        && !e
                            .actor_email
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&s)
                        && !e.summary.to_lowercase().contains(&s)
                    {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        items.sort_by(|a, b| {
            let ord = a
                .created_at
                .timestamp_millis()
                .cmp(&b.created_at.timestamp_millis())
                .then_with(|| a.id.cmp(&b.id));
            if matches!(filter.sort_order, Some(SortOrder::Desc) | None) {
                ord.reverse()
            } else {
                ord
            }
        });

        let total = items.len() as u64;
        let start = usize::try_from(filter.skip).unwrap_or(0);
        let limit = usize::try_from(filter.limit).unwrap_or(100);
        let page = items.into_iter().skip(start).take(limit).collect();
        Ok((page, total))
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.entries.read().await.len() as u64)
    }

    async fn count_since(&self, since: BsonDateTime) -> Result<u64, AppError> {
        let since_ms = since.timestamp_millis();
        Ok(self
            .entries
            .read()
            .await
            .iter()
            .filter(|e| e.created_at.timestamp_millis() >= since_ms)
            .count() as u64)
    }

    async fn count_action_since(&self, action: &str, since: BsonDateTime) -> Result<u64, AppError> {
        let since_ms = since.timestamp_millis();
        Ok(self
            .entries
            .read()
            .await
            .iter()
            .filter(|e| e.action == action && e.created_at.timestamp_millis() >= since_ms)
            .count() as u64)
    }

    async fn aggregate_daily_activity(
        &self,
        since: BsonDateTime,
    ) -> Result<Vec<DailyAuditBucket>, AppError> {
        let since_ms = since.timestamp_millis();
        let guard = self.entries.read().await;
        let mut by_day: HashMap<String, DailyAuditBucket> = HashMap::new();

        for entry in guard
            .iter()
            .filter(|e| e.created_at.timestamp_millis() >= since_ms)
        {
            let date = bson_dt_to_chrono(entry.created_at)
                .format("%Y-%m-%d")
                .to_string();
            let bucket = by_day
                .entry(date.clone())
                .or_insert_with(|| DailyAuditBucket {
                    date,
                    total: 0,
                    auth_success: 0,
                    auth_failed: 0,
                    admin_changes: 0,
                });
            bucket.total += 1;
            if matches!(entry.action.as_str(), "auth.login" | "auth.registered") {
                bucket.auth_success += 1;
            }
            if matches!(
                entry.action.as_str(),
                "auth.login_failed"
                    | "auth.refresh_token_reused"
                    | "auth.refresh_token_transport_mismatch"
            ) {
                bucket.auth_failed += 1;
            }
            if matches!(
                entry.resource_type.as_str(),
                "user" | "role" | "permission" | "organisation"
            ) {
                bucket.admin_changes += 1;
            }
        }

        let mut list: Vec<DailyAuditBucket> = by_day.into_values().collect();
        list.sort_by(|a, b| a.date.cmp(&b.date));
        list.truncate(31);
        Ok(list)
    }

    async fn count_by_resource_type_since(
        &self,
        since: BsonDateTime,
    ) -> Result<Vec<(String, u64)>, AppError> {
        let since_ms = since.timestamp_millis();
        let guard = self.entries.read().await;
        let mut counts: HashMap<String, u64> = HashMap::new();
        for entry in guard
            .iter()
            .filter(|e| e.created_at.timestamp_millis() >= since_ms)
        {
            *counts.entry(entry.resource_type.clone()).or_insert(0) += 1;
        }
        let mut list: Vec<(String, u64)> = counts.into_iter().collect();
        list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        list.truncate(10);
        Ok(list)
    }
}

#[derive(Default)]
pub struct InMemoryOrganisationRepository {
    organisations: RwLock<HashMap<ObjectId, Organisation>>,
}

#[async_trait]
impl OrganisationRepository for InMemoryOrganisationRepository {
    async fn create(&self, organisation: &Organisation) -> Result<Organisation, AppError> {
        let mut guard = self.organisations.write().await;
        if guard
            .values()
            .any(|o| o.name.eq_ignore_ascii_case(&organisation.name))
        {
            return Err(AppError::conflict(
                "DUPLICATE_RESOURCE",
                "A resource with the provided unique value already exists",
            ));
        }
        guard.insert(organisation.id, organisation.clone());
        Ok(organisation.clone())
    }

    async fn find_by_id(&self, id: ObjectId) -> Result<Option<Organisation>, AppError> {
        Ok(self.organisations.read().await.get(&id).cloned())
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Organisation>, AppError> {
        let needle = name.trim();
        Ok(self
            .organisations
            .read()
            .await
            .values()
            .find(|o| o.name.eq_ignore_ascii_case(needle))
            .cloned())
    }

    async fn find_by_member_user_id(
        &self,
        user_id: ObjectId,
    ) -> Result<Vec<Organisation>, AppError> {
        let mut items: Vec<Organisation> = self
            .organisations
            .read()
            .await
            .values()
            .filter(|o| o.members.iter().any(|m| m.user_id == user_id))
            .cloned()
            .collect();
        items.sort_by(|a, b| {
            b.created_at
                .timestamp_millis()
                .cmp(&a.created_at.timestamp_millis())
        });
        items.truncate(1000);
        Ok(items)
    }

    async fn find_all(
        &self,
        filter: &OrganisationFilter,
    ) -> Result<(Vec<Organisation>, u64), AppError> {
        let guard = self.organisations.read().await;
        let mut items: Vec<Organisation> = guard
            .values()
            .filter(|o| {
                if let Some(ref search) = filter.search {
                    let s = search.trim().to_lowercase();
                    if !s.is_empty() && !o.name.to_lowercase().contains(&s) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        items.sort_by(|a, b| {
            let ord = match filter.sort_by.as_str() {
                "name" => a.name.cmp(&b.name),
                "updated_at" => a
                    .updated_at
                    .timestamp_millis()
                    .cmp(&b.updated_at.timestamp_millis()),
                _ => a
                    .created_at
                    .timestamp_millis()
                    .cmp(&b.created_at.timestamp_millis()),
            };
            if matches!(filter.sort_order, Some(SortOrder::Desc) | None) {
                ord.reverse()
            } else {
                ord
            }
        });

        let total = items.len() as u64;
        let start = usize::try_from(filter.skip).unwrap_or(0);
        let limit = usize::try_from(filter.limit).unwrap_or(100);
        let page = items.into_iter().skip(start).take(limit).collect();
        Ok((page, total))
    }

    async fn update(&self, organisation: &Organisation) -> Result<Organisation, AppError> {
        let mut guard = self.organisations.write().await;
        let Some(existing) = guard.get(&organisation.id) else {
            return Err(AppError::conflict(
                "CONCURRENT_MODIFICATION",
                "Organisation was modified concurrently by another request; please retry",
            ));
        };
        if existing.version != organisation.version {
            return Err(AppError::conflict(
                "CONCURRENT_MODIFICATION",
                "Organisation was modified concurrently by another request; please retry",
            ));
        }
        if guard
            .values()
            .any(|o| o.id != organisation.id && o.name.eq_ignore_ascii_case(&organisation.name))
        {
            return Err(AppError::conflict(
                "DUPLICATE_RESOURCE",
                "A resource with the provided unique value already exists",
            ));
        }
        let mut next = organisation.clone();
        next.version = existing.version.saturating_add(1);
        guard.insert(next.id, next.clone());
        Ok(next)
    }

    async fn delete(&self, id: ObjectId) -> Result<bool, AppError> {
        Ok(self.organisations.write().await.remove(&id).is_some())
    }

    async fn remove_member_from_all_organisations(
        &self,
        user_id: ObjectId,
    ) -> Result<(), AppError> {
        let mut guard = self.organisations.write().await;
        let now = BsonDateTime::now();
        for org in guard.values_mut() {
            let before = org.members.len();
            org.members.retain(|m| m.user_id != user_id);
            if org.members.len() != before {
                org.updated_at = now;
                org.version = org.version.saturating_add(1);
            }
        }
        Ok(())
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.organisations.read().await.len() as u64)
    }

    async fn count_created_since(&self, since: BsonDateTime) -> Result<u64, AppError> {
        let since_ms = since.timestamp_millis();
        Ok(self
            .organisations
            .read()
            .await
            .values()
            .filter(|o| o.created_at.timestamp_millis() >= since_ms)
            .count() as u64)
    }

    async fn membership_counts(&self) -> Result<OrganisationMembershipCounts, AppError> {
        let guard = self.organisations.read().await;
        let mut counts = OrganisationMembershipCounts::default();
        for org in guard.values() {
            for member in &org.members {
                counts.total += 1;
                match member.status {
                    OrganisationMemberStatus::Active => counts.active += 1,
                    OrganisationMemberStatus::Invited => counts.invited += 1,
                    OrganisationMemberStatus::Suspended => counts.suspended += 1,
                }
            }
        }
        Ok(counts)
    }

    async fn find_largest(&self, limit: u64) -> Result<Vec<Organisation>, AppError> {
        let mut items: Vec<Organisation> =
            self.organisations.read().await.values().cloned().collect();
        items.sort_by(|a, b| {
            b.members
                .len()
                .cmp(&a.members.len())
                .then_with(|| a.name.cmp(&b.name))
        });
        items.truncate(usize::try_from(limit).unwrap_or(8));
        Ok(items)
    }
}

pub async fn create_test_app_state() -> Result<AppState, AppError> {
    let mut settings = Settings::default();
    settings.rate_limit.enabled = false;
    settings.bootstrap.super_admin_email = "superadmin@test.local".to_string();
    settings.bootstrap.super_admin_password = "SuperAdmin123!".to_string();

    let settings = Arc::new(settings);
    let keys = JwtKeyPair::generate_ephemeral()?;
    let jwt_service = Arc::new(JwtService::new(keys, &settings.jwt));

    let user_repo: Arc<dyn UserRepository> = Arc::new(InMemoryUserRepository::default());
    let role_repo: Arc<dyn RoleRepository> = Arc::new(InMemoryRoleRepository::default());
    let permission_repo: Arc<dyn PermissionRepository> =
        Arc::new(InMemoryPermissionRepository::default());
    let refresh_token_repo: Arc<dyn RefreshTokenRepository> =
        Arc::new(InMemoryRefreshTokenRepository::default());
    let organisation_repo: Arc<dyn OrganisationRepository> =
        Arc::new(InMemoryOrganisationRepository::default());
    let audit_repo: Arc<dyn AuditLogRepository> = Arc::new(InMemoryAuditLogRepository::default());

    seed_system_rbac(
        &permission_repo,
        &role_repo,
        &user_repo,
        &settings.bootstrap,
    )
    .await?;

    Ok(AppState::from_repositories(
        settings,
        None,
        jwt_service,
        user_repo,
        role_repo,
        permission_repo,
        refresh_token_repo,
        organisation_repo,
        audit_repo,
    ))
}
