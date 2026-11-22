use std::sync::Arc;

use mongodb::bson::DateTime as BsonDateTime;
use serde_json::json;

use crate::{
    common::{
        errors::AppError,
        validation::{normalize_email, validate_dto},
    },
    database::seed::ROLE_SUPER_ADMIN,
    middleware::{AuthenticatedUser, PermissionCache},
    repository::{
        auth::refresh_token_repository::RefreshTokenRepository,
        permission::permission_repository::PermissionRepository,
        role::role_repository::RoleRepository,
        user::{
            dao::user::{AccountStatus, UserProfileInfo},
            user_repository::UserRepository,
        },
    },
    security::password::{hash_password, verify_password},
    services::audit::{AuditEvent, AuditService},
    web::dto::{
        profile::{ChangePasswordRequest, DeleteAccountRequest, UpdateProfileRequest},
        roles::RoleSummary,
        users::UserResponse,
    },
};

#[derive(Clone)]
pub struct ProfileService {
    user_repo: Arc<dyn UserRepository>,
    role_repo: Arc<dyn RoleRepository>,
    permission_repo: Arc<dyn PermissionRepository>,
    refresh_token_repo: Arc<dyn RefreshTokenRepository>,
    permission_cache: PermissionCache,
    audit_service: AuditService,
}

impl ProfileService {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        role_repo: Arc<dyn RoleRepository>,
        permission_repo: Arc<dyn PermissionRepository>,
        refresh_token_repo: Arc<dyn RefreshTokenRepository>,
        permission_cache: PermissionCache,
        audit_service: AuditService,
    ) -> Self {
        Self {
            user_repo,
            role_repo,
            permission_repo,
            refresh_token_repo,
            permission_cache,
            audit_service,
        }
    }

    pub async fn get_profile(&self, actor: &AuthenticatedUser) -> Result<UserResponse, AppError> {
        actor.require_permission("profile.read")?;
        let user = self
            .user_repo
            .find_by_id(actor.user_id)
            .await?
            .ok_or_else(|| AppError::not_found("USER_NOT_FOUND", "User profile not found"))?;

        let roles = self.role_repo.find_by_ids(&user.roles).await?;
        let summaries: Vec<RoleSummary> = roles.iter().map(RoleSummary::from).collect();
        let resolved = self
            .permission_cache
            .resolve_user_access(user.id, &user.roles, &self.role_repo, &self.permission_repo)
            .await?;
        let mut permissions: Vec<String> = resolved.permissions.into_iter().collect();
        permissions.sort();

        Ok(UserResponse::from_user(user, summaries, Some(permissions)))
    }

    pub async fn update_profile(
        &self,
        actor: &AuthenticatedUser,
        req: UpdateProfileRequest,
    ) -> Result<UserResponse, AppError> {
        actor.require_permission("profile.update")?;
        validate_dto(&req)?;
        if let Some(ref profile) = req.profile {
            validate_dto(profile)?;
        }

        let mut user = self
            .user_repo
            .find_by_id(actor.user_id)
            .await?
            .ok_or_else(|| AppError::not_found("USER_NOT_FOUND", "User profile not found"))?;

        let previous_email = user.email.clone();
        if let Some(first_name) = req.first_name {
            user.first_name = first_name.trim().to_string();
        }
        if let Some(last_name) = req.last_name {
            user.last_name = last_name.trim().to_string();
        }
        if let Some(email) = req.email {
            let normalized = normalize_email(&email);
            if normalized != user.email {
                if self.user_repo.find_by_email(&normalized).await?.is_some() {
                    return Err(AppError::conflict(
                        "EMAIL_ALREADY_EXISTS",
                        "Another account is already using this email address",
                    ));
                }
                user.email = normalized;
                user.email_verified = false;
            }
        }
        if let Some(profile) = req.profile {
            user.profile = UserProfileInfo::from(profile);
        }

        user.updated_at = BsonDateTime::now();
        let updated = self.user_repo.update(&user).await?;

        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "profile.updated",
                    "profile",
                    format!("Updated profile for '{}'", updated.email),
                )
                .with_resource(updated.id.to_hex(), updated.email.clone())
                .with_details(json!({
                    "previous_email": previous_email,
                    "email": updated.email,
                    "first_name": updated.first_name,
                    "last_name": updated.last_name,
                })),
            )
            .await;

        let roles = self.role_repo.find_by_ids(&updated.roles).await?;
        let summaries: Vec<RoleSummary> = roles.iter().map(RoleSummary::from).collect();
        let mut permissions: Vec<String> = actor.permissions.iter().cloned().collect();
        permissions.sort();

        Ok(UserResponse::from_user(
            updated,
            summaries,
            Some(permissions),
        ))
    }

    pub async fn change_password(
        &self,
        actor: &AuthenticatedUser,
        req: ChangePasswordRequest,
    ) -> Result<(), AppError> {
        actor.require_permission("profile.update")?;
        validate_dto(&req)?;

        let mut user = self
            .user_repo
            .find_by_id(actor.user_id)
            .await?
            .ok_or_else(|| AppError::not_found("USER_NOT_FOUND", "User not found"))?;

        let valid =
            verify_password(req.current_password.clone(), user.password_hash.clone()).await?;
        if !valid {
            return Err(AppError::bad_request(
                "INVALID_CURRENT_PASSWORD",
                "Current password is incorrect",
            ));
        }

        if req.current_password == req.new_password {
            return Err(AppError::bad_request(
                "SAME_PASSWORD",
                "New password must differ from your current password",
            ));
        }

        user.password_hash = hash_password(req.new_password).await?;
        user.token_version = user.token_version.saturating_add(1);
        user.updated_at = BsonDateTime::now();
        self.user_repo.update(&user).await?;
        self.refresh_token_repo.revoke_all_for_user(user.id).await?;
        self.permission_cache.invalidate_user(user.id).await;

        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "profile.password_changed",
                    "profile",
                    format!("Changed password for '{}'", user.email),
                )
                .with_resource(user.id.to_hex(), user.email.clone()),
            )
            .await;

        Ok(())
    }

    pub async fn delete_account(
        &self,
        actor: &AuthenticatedUser,
        req: DeleteAccountRequest,
    ) -> Result<(), AppError> {
        actor.require_permission("profile.delete")?;
        validate_dto(&req)?;

        let user = self
            .user_repo
            .find_by_id(actor.user_id)
            .await?
            .ok_or_else(|| AppError::not_found("USER_NOT_FOUND", "User not found"))?;

        let valid = verify_password(req.current_password, user.password_hash.clone()).await?;
        if !valid {
            return Err(AppError::bad_request(
                "INVALID_CURRENT_PASSWORD",
                "Current password is incorrect",
            ));
        }

        if let Some(sa_role) = self.role_repo.find_by_name(ROLE_SUPER_ADMIN).await?
            && user.roles.contains(&sa_role.id)
            && user.status == AccountStatus::Active
        {
            let active_count = self
                .user_repo
                .count_active_users_with_role(sa_role.id)
                .await?;
            if active_count <= 1 {
                return Err(AppError::conflict(
                    "LAST_SUPER_ADMIN_PROTECTED",
                    "Cannot delete the last active Super Administrator account",
                ));
            }
        }

        self.refresh_token_repo.revoke_all_for_user(user.id).await?;
        self.permission_cache.invalidate_user(user.id).await;
        self.user_repo.delete(user.id).await?;

        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "profile.account_deleted",
                    "profile",
                    format!("Deleted own account '{}'", user.email),
                )
                .with_resource(user.id.to_hex(), user.email.clone()),
            )
            .await;

        Ok(())
    }
}
