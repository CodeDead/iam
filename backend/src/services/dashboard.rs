use std::sync::Arc;

use crate::{
    common::errors::AppError,
    middleware::AuthenticatedUser,
    repository::{
        permission::permission_repository::PermissionRepository,
        role::role_repository::RoleRepository,
        user::{dao::user::AccountStatus, user_repository::UserRepository},
    },
    web::dto::dashboard::{DashboardStatsResponse, UserStats},
};

#[derive(Clone)]
pub struct DashboardService {
    user_repo: Arc<dyn UserRepository>,
    role_repo: Arc<dyn RoleRepository>,
    permission_repo: Arc<dyn PermissionRepository>,
}

impl DashboardService {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        role_repo: Arc<dyn RoleRepository>,
        permission_repo: Arc<dyn PermissionRepository>,
    ) -> Self {
        Self {
            user_repo,
            role_repo,
            permission_repo,
        }
    }

    pub async fn get_stats(
        &self,
        actor: &AuthenticatedUser,
    ) -> Result<DashboardStatsResponse, AppError> {
        let users = if actor.has_permission("users.read") {
            let total = self.user_repo.count_all().await?;
            let active = self
                .user_repo
                .count_by_status(AccountStatus::Active)
                .await?;
            let disabled = self
                .user_repo
                .count_by_status(AccountStatus::Disabled)
                .await?;
            let locked = self
                .user_repo
                .count_by_status(AccountStatus::Locked)
                .await?;
            let pending_verification = self
                .user_repo
                .count_by_status(AccountStatus::PendingVerification)
                .await?;
            Some(UserStats {
                total,
                active,
                disabled,
                locked,
                pending_verification,
            })
        } else {
            None
        };

        let total_roles = if actor.has_permission("roles.read") {
            Some(self.role_repo.count_all().await?)
        } else {
            None
        };

        let total_permissions = if actor.has_permission("permissions.read") {
            Some(self.permission_repo.count_all().await?)
        } else {
            None
        };

        Ok(DashboardStatsResponse {
            users,
            total_roles,
            total_permissions,
        })
    }
}
