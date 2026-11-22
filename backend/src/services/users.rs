use std::{collections::HashSet, sync::Arc};

use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use serde_json::json;

use crate::{
    common::{
        errors::AppError,
        pagination::{PaginatedResponse, PaginationParams},
        validation::{normalize_email, parse_object_id, validate_dto},
    },
    database::seed::{ROLE_SUPER_ADMIN, ROLE_USER},
    middleware::{AuthenticatedUser, PermissionCache},
    repository::{
        auth::refresh_token_repository::RefreshTokenRepository,
        permission::permission_repository::PermissionRepository,
        role::{dao::role::Role, role_repository::RoleRepository},
        user::{
            dao::user::{AccountStatus, User, UserProfileInfo},
            user_repository::{UserFilter, UserRepository},
        },
    },
    security::password::hash_password,
    services::audit::{AuditEvent, AuditService},
    web::dto::{
        permissions::PermissionResponse,
        roles::RoleSummary,
        users::{
            CreateUserRequest, ListUsersQuery, UpdateUserRequest, UpdateUserRolesRequest,
            UpdateUserStatusRequest, UserResponse,
        },
    },
};

#[derive(Clone)]
pub struct UserService {
    user_repo: Arc<dyn UserRepository>,
    role_repo: Arc<dyn RoleRepository>,
    permission_repo: Arc<dyn PermissionRepository>,
    refresh_token_repo: Arc<dyn RefreshTokenRepository>,
    permission_cache: PermissionCache,
    audit_service: AuditService,
}

impl UserService {
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

    pub async fn list(
        &self,
        actor: &AuthenticatedUser,
        query: ListUsersQuery,
    ) -> Result<PaginatedResponse<UserResponse>, AppError> {
        actor.require_permission("users.read")?;

        let pagination = PaginationParams {
            page: query.page,
            page_size: query.page_size,
            sort_by: query.sort_by,
            sort_order: query.sort_order,
        };

        let filter = UserFilter {
            search: query.search,
            status: query.status,
            skip: pagination.skip(),
            limit: pagination.normalized_page_size(),
            sort_by: pagination
                .sort_by
                .clone()
                .unwrap_or_else(|| "created_at".to_string()),
            sort_order: Some(pagination.sort_order_or_default()),
        };

        let (users, total) = self.user_repo.find_all(&filter).await?;
        let role_ids: HashSet<ObjectId> =
            users.iter().flat_map(|u| u.roles.iter().copied()).collect();
        let role_id_vec: Vec<ObjectId> = role_ids.into_iter().collect();
        let all_roles = self.role_repo.find_by_ids(&role_id_vec).await?;

        let mut items = Vec::with_capacity(users.len());
        for user in users {
            let summaries: Vec<RoleSummary> = all_roles
                .iter()
                .filter(|r| user.roles.contains(&r.id))
                .map(RoleSummary::from)
                .collect();
            items.push(UserResponse::from_user(user, summaries, None));
        }

        Ok(PaginatedResponse::new(
            items,
            total,
            pagination.normalized_page(),
            pagination.normalized_page_size(),
        ))
    }

    pub async fn get_by_id(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
    ) -> Result<UserResponse, AppError> {
        actor.require_permission("users.read")?;
        let user_id = parse_object_id(id)?;
        let user = self.find_user_or_not_found(user_id).await?;
        self.build_user_response_with_permissions(user).await
    }

    pub async fn create(
        &self,
        actor: &AuthenticatedUser,
        req: CreateUserRequest,
    ) -> Result<UserResponse, AppError> {
        actor.require_permission("users.create")?;
        validate_dto(&req)?;
        if let Some(ref profile) = req.profile {
            validate_dto(profile)?;
        }

        let email = normalize_email(&req.email);
        if self.user_repo.find_by_email(&email).await?.is_some() {
            return Err(AppError::conflict(
                "EMAIL_ALREADY_EXISTS",
                "A user with this email address already exists",
            ));
        }

        let assigned_roles = if req.role_ids.is_empty() {
            let default_role = self
                .role_repo
                .find_by_name(ROLE_USER)
                .await?
                .ok_or_else(|| AppError::internal("Default 'User' role not found"))?;
            vec![default_role]
        } else {
            actor.require_permission("users.assign_roles")?;
            let roles = self.resolve_and_validate_roles(&req.role_ids).await?;
            self.ensure_actor_can_assign_roles(actor, &roles).await?;
            roles
        };

        let password_hash = hash_password(req.password).await?;
        let now = BsonDateTime::now();

        let user = User {
            id: ObjectId::new(),
            first_name: req.first_name.trim().to_string(),
            last_name: req.last_name.trim().to_string(),
            email,
            password_hash,
            status: req.status.unwrap_or(AccountStatus::Active),
            email_verified: req.email_verified.unwrap_or(false),
            roles: assigned_roles.iter().map(|r| r.id).collect(),
            token_version: 1,
            profile: req.profile.map(UserProfileInfo::from).unwrap_or_default(),
            created_at: now,
            updated_at: now,
            last_login_at: None,
        };

        let created = self.user_repo.create(&user).await?;
        let role_names: Vec<String> = assigned_roles.iter().map(|r| r.name.clone()).collect();
        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "user.created",
                    "user",
                    format!("Created user '{}'", created.email),
                )
                .with_resource(created.id.to_hex(), created.email.clone())
                .with_details(json!({
                    "status": created.status.as_str(),
                    "roles": role_names,
                })),
            )
            .await;
        self.build_user_response_with_permissions(created).await
    }

    pub async fn update(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
        req: UpdateUserRequest,
    ) -> Result<UserResponse, AppError> {
        actor.require_permission("users.update")?;
        validate_dto(&req)?;
        if let Some(ref profile) = req.profile {
            validate_dto(profile)?;
        }

        let user_id = parse_object_id(id)?;
        let mut user = self.find_user_or_not_found(user_id).await?;

        if !actor.is_super_admin && self.user_has_super_admin_role(&user).await? {
            return Err(AppError::forbidden(
                "PRIVILEGE_ESCALATION_DENIED",
                "Only a Super Administrator can modify a Super Administrator account",
            ));
        }

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
                        "A user with this email address already exists",
                    ));
                }
                user.email = normalized;
            }
        }
        if let Some(email_verified) = req.email_verified {
            user.email_verified = email_verified;
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
                    "user.updated",
                    "user",
                    format!("Updated user '{}'", updated.email),
                )
                .with_resource(updated.id.to_hex(), updated.email.clone())
                .with_details(json!({
                    "previous_email": previous_email,
                    "email": updated.email,
                    "first_name": updated.first_name,
                    "last_name": updated.last_name,
                    "email_verified": updated.email_verified,
                })),
            )
            .await;
        self.build_user_response_with_permissions(updated).await
    }

    pub async fn update_status(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
        req: UpdateUserStatusRequest,
    ) -> Result<UserResponse, AppError> {
        actor.require_permission("users.update")?;
        validate_dto(&req)?;

        let target_id = parse_object_id(id)?;
        if actor.user_id == target_id {
            return Err(AppError::forbidden(
                "SELF_STATUS_MODIFICATION_FORBIDDEN",
                "Users cannot modify their own account status",
            ));
        }

        let mut user = self.find_user_or_not_found(target_id).await?;
        let target_is_super_admin = self.user_has_super_admin_role(&user).await?;

        if target_is_super_admin && !actor.is_super_admin {
            return Err(AppError::forbidden(
                "PRIVILEGE_ESCALATION_DENIED",
                "Only a Super Administrator can change the status of a Super Administrator",
            ));
        }

        if target_is_super_admin
            && user.status == AccountStatus::Active
            && req.status != AccountStatus::Active
        {
            self.ensure_not_last_active_super_admin().await?;
        }

        let previous_status = user.status;
        user.status = req.status;
        user.updated_at = BsonDateTime::now();
        if req.status != AccountStatus::Active {
            user.token_version = user.token_version.saturating_add(1);
        }

        let updated = self.user_repo.update(&user).await?;
        if req.status != AccountStatus::Active {
            self.refresh_token_repo
                .revoke_all_for_user(updated.id)
                .await?;
            self.permission_cache.invalidate_user(updated.id).await;
        }

        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "user.status_updated",
                    "user",
                    format!(
                        "Changed status for user '{}' from {} to {}",
                        updated.email,
                        previous_status.as_str(),
                        updated.status.as_str()
                    ),
                )
                .with_resource(updated.id.to_hex(), updated.email.clone())
                .with_details(json!({
                    "previous_status": previous_status.as_str(),
                    "status": updated.status.as_str(),
                })),
            )
            .await;

        self.build_user_response_with_permissions(updated).await
    }

    pub async fn delete(&self, actor: &AuthenticatedUser, id: &str) -> Result<(), AppError> {
        actor.require_permission("users.delete")?;
        let target_id = parse_object_id(id)?;

        if actor.user_id == target_id {
            return Err(AppError::forbidden(
                "SELF_DELETE_FORBIDDEN",
                "Users cannot delete their own account through administration endpoints",
            ));
        }

        let user = self.find_user_or_not_found(target_id).await?;
        let target_is_super_admin = self.user_has_super_admin_role(&user).await?;

        if target_is_super_admin && !actor.is_super_admin {
            return Err(AppError::forbidden(
                "PRIVILEGE_ESCALATION_DENIED",
                "Only a Super Administrator can delete a Super Administrator",
            ));
        }

        if target_is_super_admin && user.status == AccountStatus::Active {
            self.ensure_not_last_active_super_admin().await?;
        }

        self.refresh_token_repo
            .revoke_all_for_user(target_id)
            .await?;
        self.permission_cache.invalidate_user(target_id).await;
        self.user_repo.delete(target_id).await?;
        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "user.deleted",
                    "user",
                    format!("Deleted user '{}'", user.email),
                )
                .with_resource(user.id.to_hex(), user.email.clone()),
            )
            .await;
        Ok(())
    }

    pub async fn get_roles(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
    ) -> Result<Vec<RoleSummary>, AppError> {
        actor.require_permission("users.read")?;
        let user_id = parse_object_id(id)?;
        let user = self.find_user_or_not_found(user_id).await?;
        let roles = self.role_repo.find_by_ids(&user.roles).await?;
        Ok(roles.iter().map(RoleSummary::from).collect())
    }

    pub async fn update_roles(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
        req: UpdateUserRolesRequest,
    ) -> Result<UserResponse, AppError> {
        actor.require_permission("users.assign_roles")?;
        validate_dto(&req)?;

        let target_id = parse_object_id(id)?;
        if actor.user_id == target_id && !actor.has_permission("users.manage_self_roles") {
            return Err(AppError::forbidden(
                "SELF_ROLE_MODIFICATION_FORBIDDEN",
                "You are not authorized to modify your own assigned roles",
            ));
        }

        let mut user = self.find_user_or_not_found(target_id).await?;
        let target_was_super_admin = self.user_has_super_admin_role(&user).await?;

        if target_was_super_admin && !actor.is_super_admin {
            return Err(AppError::forbidden(
                "PRIVILEGE_ESCALATION_DENIED",
                "Only a Super Administrator can modify roles of a Super Administrator",
            ));
        }

        let new_roles = self.resolve_and_validate_roles(&req.role_ids).await?;
        self.ensure_actor_can_assign_roles(actor, &new_roles)
            .await?;

        let new_has_super_admin = new_roles.iter().any(|r| r.name == ROLE_SUPER_ADMIN);
        if target_was_super_admin && !new_has_super_admin && user.status == AccountStatus::Active {
            self.ensure_not_last_active_super_admin().await?;
        }

        user.roles = new_roles.iter().map(|r| r.id).collect();
        user.token_version = user.token_version.saturating_add(1);
        user.updated_at = BsonDateTime::now();
        let updated = self.user_repo.update(&user).await?;

        // Immediately invalidate permission cache and revoke refresh tokens for security-sensitive changes.
        self.permission_cache.invalidate_user(updated.id).await;
        self.refresh_token_repo
            .revoke_all_for_user(updated.id)
            .await?;

        let mut role_names: Vec<String> = new_roles.iter().map(|r| r.name.clone()).collect();
        role_names.sort();
        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "user.roles_updated",
                    "user",
                    format!("Updated roles for user '{}'", updated.email),
                )
                .with_resource(updated.id.to_hex(), updated.email.clone())
                .with_details(json!({
                    "roles": role_names,
                })),
            )
            .await;

        self.build_user_response_with_permissions(updated).await
    }

    pub async fn get_effective_permissions(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
    ) -> Result<Vec<PermissionResponse>, AppError> {
        actor.require_permission("users.read")?;
        let user_id = parse_object_id(id)?;
        let user = self.find_user_or_not_found(user_id).await?;

        let roles = self.role_repo.find_by_ids(&user.roles).await?;
        let perm_ids: Vec<ObjectId> = roles
            .into_iter()
            .flat_map(|r| r.permissions)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let mut perms = self.permission_repo.find_by_ids(&perm_ids).await?;
        perms.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(perms.into_iter().map(PermissionResponse::from).collect())
    }

    async fn find_user_or_not_found(&self, id: ObjectId) -> Result<User, AppError> {
        self.user_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::not_found("USER_NOT_FOUND", "User not found"))
    }

    async fn resolve_and_validate_roles(&self, role_ids: &[String]) -> Result<Vec<Role>, AppError> {
        let mut parsed_ids = Vec::with_capacity(role_ids.len());
        let mut seen = HashSet::new();
        for raw_id in role_ids {
            let oid = parse_object_id(raw_id)?;
            if seen.insert(oid) {
                parsed_ids.push(oid);
            }
        }

        if parsed_ids.is_empty() {
            return Err(AppError::bad_request(
                "EMPTY_ROLES",
                "At least one valid role ID must be supplied",
            ));
        }

        let roles = self.role_repo.find_by_ids(&parsed_ids).await?;
        if roles.len() != parsed_ids.len() {
            return Err(AppError::bad_request(
                "INVALID_ROLE_REFERENCE",
                "One or more specified role IDs do not exist",
            ));
        }

        Ok(roles)
    }

    async fn ensure_actor_can_assign_roles(
        &self,
        actor: &AuthenticatedUser,
        roles: &[Role],
    ) -> Result<(), AppError> {
        if actor.is_super_admin {
            return Ok(());
        }

        for role in roles {
            if role.name == ROLE_SUPER_ADMIN {
                return Err(AppError::forbidden(
                    "PRIVILEGE_ESCALATION_DENIED",
                    "Only a Super Administrator can assign the Super Administrator role",
                ));
            }
        }

        let perm_ids: Vec<ObjectId> = roles
            .iter()
            .flat_map(|r| r.permissions.iter().copied())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let perms = self.permission_repo.find_by_ids(&perm_ids).await?;
        for perm in perms {
            if !actor.has_permission(&perm.name) {
                return Err(AppError::forbidden(
                    "PRIVILEGE_ESCALATION_DENIED",
                    format!(
                        "Cannot assign a role granting permission '{}' that you do not hold",
                        perm.name
                    ),
                ));
            }
        }

        Ok(())
    }

    async fn user_has_super_admin_role(&self, user: &User) -> Result<bool, AppError> {
        if let Some(sa_role) = self.role_repo.find_by_name(ROLE_SUPER_ADMIN).await? {
            return Ok(user.roles.contains(&sa_role.id));
        }
        Ok(false)
    }

    async fn ensure_not_last_active_super_admin(&self) -> Result<(), AppError> {
        if let Some(sa_role) = self.role_repo.find_by_name(ROLE_SUPER_ADMIN).await? {
            let active_count = self
                .user_repo
                .count_active_users_with_role(sa_role.id)
                .await?;
            if active_count <= 1 {
                return Err(AppError::conflict(
                    "LAST_SUPER_ADMIN_PROTECTED",
                    "Cannot disable, demote, or delete the last active Super Administrator",
                ));
            }
        }
        Ok(())
    }

    async fn build_user_response_with_permissions(
        &self,
        user: User,
    ) -> Result<UserResponse, AppError> {
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
}
