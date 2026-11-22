#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use tokio::sync::RwLock;

use crate::{
    app::AppState,
    common::{errors::AppError, pagination::SortOrder},
    config::Settings,
    database::seed::seed_system_rbac,
    repository::{
        audit::{
            audit_log_repository::{AuditLogFilter, AuditLogRepository},
            dao::audit_log::AuditLog,
        },
        auth::{
            dao::refresh_token::RefreshToken, refresh_token_repository::RefreshTokenRepository,
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
        Ok(self
            .users
            .read()
            .await
            .values()
            .filter(|u| u.roles.contains(&role_id))
            .cloned()
            .collect())
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
        guard.insert(user.id, user.clone());
        Ok(user.clone())
    }

    async fn delete(&self, id: ObjectId) -> Result<bool, AppError> {
        Ok(self.users.write().await.remove(&id).is_some())
    }

    async fn remove_role_from_all_users(&self, role_id: ObjectId) -> Result<(), AppError> {
        let mut guard = self.users.write().await;
        for user in guard.values_mut() {
            user.roles.retain(|r| *r != role_id);
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
            let ord = a.name.cmp(&b.name);
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
        self.roles.write().await.insert(role.id, role.clone());
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
        for role in guard.values_mut() {
            role.permissions.retain(|p| *p != permission_id);
        }
        Ok(())
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.roles.read().await.len() as u64)
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

        items.sort_by(|a, b| a.name.cmp(&b.name));
        let total = items.len() as u64;
        let start = usize::try_from(filter.skip).unwrap_or(0);
        let limit = usize::try_from(filter.limit).unwrap_or(100);
        let page = items.into_iter().skip(start).take(limit).collect();
        Ok((page, total))
    }

    async fn update(&self, permission: &Permission) -> Result<Permission, AppError> {
        self.permissions
            .write()
            .await
            .insert(permission.id, permission.clone());
        Ok(permission.clone())
    }

    async fn delete(&self, id: ObjectId) -> Result<bool, AppError> {
        Ok(self.permissions.write().await.remove(&id).is_some())
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.permissions.read().await.len() as u64)
    }
}

#[derive(Default)]
pub struct InMemoryRefreshTokenRepository {
    tokens: RwLock<HashMap<String, RefreshToken>>,
}

#[async_trait]
impl RefreshTokenRepository for InMemoryRefreshTokenRepository {
    async fn create(&self, token: &RefreshToken) -> Result<(), AppError> {
        self.tokens
            .write()
            .await
            .insert(token.token_hash.clone(), token.clone());
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

        if matches!(filter.sort_order, Some(SortOrder::Desc) | None) {
            items.reverse();
        }

        let total = items.len() as u64;
        let start = usize::try_from(filter.skip).unwrap_or(0);
        let limit = usize::try_from(filter.limit).unwrap_or(100);
        let page = items.into_iter().skip(start).take(limit).collect();
        Ok((page, total))
    }

    async fn count_all(&self) -> Result<u64, AppError> {
        Ok(self.entries.read().await.len() as u64)
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
        audit_repo,
    ))
}
