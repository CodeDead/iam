use std::{collections::HashSet, sync::Arc};

use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use serde_json::json;

use crate::{
    common::{
        errors::AppError,
        pagination::{PaginatedResponse, PaginationParams},
        validation::{parse_object_id, validate_dto},
    },
    database::seed::ROLE_SUPER_ADMIN,
    middleware::{AuthenticatedUser, PermissionCache},
    repository::{
        permission::{dao::permission::Permission, permission_repository::PermissionRepository},
        role::{
            dao::role::Role,
            role_repository::{RoleFilter, RoleRepository},
        },
        user::user_repository::UserRepository,
    },
    services::audit::{AuditEvent, AuditService},
    web::dto::{
        permissions::PermissionResponse,
        roles::{
            CreateRoleRequest, ListRolesQuery, RoleResponse, RoleSummary,
            UpdateRolePermissionsRequest, UpdateRoleRequest,
        },
        users::UserResponse,
    },
};

const REQUIRED_SUPER_ADMIN_PERMISSIONS: &[&str] = &[
    "users.read",
    "users.update",
    "users.assign_roles",
    "roles.read",
    "roles.update",
    "permissions.read",
];

#[derive(Clone)]
pub struct RoleService {
    role_repo: Arc<dyn RoleRepository>,
    permission_repo: Arc<dyn PermissionRepository>,
    user_repo: Arc<dyn UserRepository>,
    permission_cache: PermissionCache,
    audit_service: AuditService,
}

impl RoleService {
    pub fn new(
        role_repo: Arc<dyn RoleRepository>,
        permission_repo: Arc<dyn PermissionRepository>,
        user_repo: Arc<dyn UserRepository>,
        permission_cache: PermissionCache,
        audit_service: AuditService,
    ) -> Self {
        Self {
            role_repo,
            permission_repo,
            user_repo,
            permission_cache,
            audit_service,
        }
    }

    pub async fn list(
        &self,
        actor: &AuthenticatedUser,
        query: ListRolesQuery,
    ) -> Result<PaginatedResponse<RoleResponse>, AppError> {
        actor.require_permission("roles.read")?;

        let pagination = PaginationParams {
            page: query.page,
            page_size: query.page_size,
            sort_by: query.sort_by,
            sort_order: query.sort_order,
        };

        let filter = RoleFilter {
            search: query.search,
            skip: pagination.skip(),
            limit: pagination.normalized_page_size(),
            sort_by: pagination
                .sort_by
                .clone()
                .unwrap_or_else(|| "name".to_string()),
            sort_order: Some(pagination.sort_order_or_default()),
        };

        let (roles, total) = self.role_repo.find_all(&filter).await?;
        let items: Vec<RoleResponse> = roles
            .into_iter()
            .map(|r| RoleResponse::from_role(r, None))
            .collect();

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
    ) -> Result<RoleResponse, AppError> {
        actor.require_permission("roles.read")?;
        let role_id = parse_object_id(id)?;
        let role = self.find_role_or_not_found(role_id).await?;
        let perms = self.load_role_permissions(&role).await?;
        Ok(RoleResponse::from_role(role, Some(perms)))
    }

    pub async fn create(
        &self,
        actor: &AuthenticatedUser,
        req: CreateRoleRequest,
    ) -> Result<RoleResponse, AppError> {
        actor.require_permission("roles.create")?;
        validate_dto(&req)?;

        let name = req.name.trim().to_string();
        if self.role_repo.find_by_name(&name).await?.is_some() {
            return Err(AppError::conflict(
                "ROLE_ALREADY_EXISTS",
                format!("A role named '{name}' already exists"),
            ));
        }

        let permissions = self
            .resolve_and_validate_permissions(&req.permission_ids)
            .await?;
        self.ensure_actor_holds_permissions(actor, &permissions)?;

        let now = BsonDateTime::now();
        let role = Role {
            id: ObjectId::new(),
            name,
            description: req.description.trim().to_string(),
            permissions: permissions.iter().map(|p| p.id).collect(),
            is_system: false,
            created_at: now,
            updated_at: now,
        };

        let created = self.role_repo.create(&role).await?;
        let mut perm_names: Vec<String> = permissions.iter().map(|p| p.name.clone()).collect();
        perm_names.sort();
        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "role.created",
                    "role",
                    format!("Created role '{}'", created.name),
                )
                .with_resource(created.id.to_hex(), created.name.clone())
                .with_details(json!({
                    "description": created.description,
                    "permissions": perm_names,
                })),
            )
            .await;

        let perm_dtos: Vec<PermissionResponse> = permissions
            .into_iter()
            .map(PermissionResponse::from)
            .collect();
        Ok(RoleResponse::from_role(created, Some(perm_dtos)))
    }

    pub async fn update(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
        req: UpdateRoleRequest,
    ) -> Result<RoleResponse, AppError> {
        actor.require_permission("roles.update")?;
        validate_dto(&req)?;

        let role_id = parse_object_id(id)?;
        let mut role = self.find_role_or_not_found(role_id).await?;

        if role.name == ROLE_SUPER_ADMIN && !actor.is_super_admin {
            return Err(AppError::forbidden(
                "PRIVILEGE_ESCALATION_DENIED",
                "Only a Super Administrator can modify the Super Administrator role",
            ));
        }

        let previous_name = role.name.clone();
        if let Some(new_name) = req.name {
            let trimmed = new_name.trim().to_string();
            if trimmed != role.name {
                if role.is_system {
                    return Err(AppError::forbidden(
                        "SYSTEM_ROLE_PROTECTED",
                        "Protected system roles cannot be renamed",
                    ));
                }
                if self.role_repo.find_by_name(&trimmed).await?.is_some() {
                    return Err(AppError::conflict(
                        "ROLE_ALREADY_EXISTS",
                        format!("A role named '{trimmed}' already exists"),
                    ));
                }
                role.name = trimmed;
            }
        }

        if let Some(description) = req.description {
            role.description = description.trim().to_string();
        }

        role.updated_at = BsonDateTime::now();
        let updated = self.role_repo.update(&role).await?;
        self.permission_cache.invalidate_all().await;
        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "role.updated",
                    "role",
                    format!("Updated role '{}'", updated.name),
                )
                .with_resource(updated.id.to_hex(), updated.name.clone())
                .with_details(json!({
                    "previous_name": previous_name,
                    "name": updated.name,
                    "description": updated.description,
                })),
            )
            .await;

        let perms = self.load_role_permissions(&updated).await?;
        Ok(RoleResponse::from_role(updated, Some(perms)))
    }

    pub async fn delete(&self, actor: &AuthenticatedUser, id: &str) -> Result<(), AppError> {
        actor.require_permission("roles.delete")?;
        let role_id = parse_object_id(id)?;
        let role = self.find_role_or_not_found(role_id).await?;

        if role.is_system {
            return Err(AppError::forbidden(
                "SYSTEM_ROLE_PROTECTED",
                "Protected system roles cannot be deleted",
            ));
        }

        self.user_repo.remove_role_from_all_users(role_id).await?;
        self.role_repo.delete(role_id).await?;
        self.permission_cache.invalidate_all().await;
        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "role.deleted",
                    "role",
                    format!("Deleted role '{}'", role.name),
                )
                .with_resource(role.id.to_hex(), role.name.clone()),
            )
            .await;
        Ok(())
    }

    pub async fn get_permissions(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
    ) -> Result<Vec<PermissionResponse>, AppError> {
        actor.require_permission("roles.read")?;
        let role_id = parse_object_id(id)?;
        let role = self.find_role_or_not_found(role_id).await?;
        self.load_role_permissions(&role).await
    }

    pub async fn update_permissions(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
        req: UpdateRolePermissionsRequest,
    ) -> Result<RoleResponse, AppError> {
        actor.require_permission("roles.update")?;
        validate_dto(&req)?;

        let role_id = parse_object_id(id)?;
        let mut role = self.find_role_or_not_found(role_id).await?;

        if role.name == ROLE_SUPER_ADMIN && !actor.is_super_admin {
            return Err(AppError::forbidden(
                "PRIVILEGE_ESCALATION_DENIED",
                "Only a Super Administrator can modify Super Administrator permissions",
            ));
        }

        let permissions = self
            .resolve_and_validate_permissions(&req.permission_ids)
            .await?;
        self.ensure_actor_holds_permissions(actor, &permissions)?;

        if role.name == ROLE_SUPER_ADMIN {
            let names: HashSet<&str> = permissions.iter().map(|p| p.name.as_str()).collect();
            for required in REQUIRED_SUPER_ADMIN_PERMISSIONS {
                if !names.contains(required) {
                    return Err(AppError::bad_request(
                        "SUPER_ADMIN_CORE_PERMISSIONS_REQUIRED",
                        format!(
                            "Super Administrator role must retain core permission '{required}'"
                        ),
                    ));
                }
            }
        }

        role.permissions = permissions.iter().map(|p| p.id).collect();
        role.updated_at = BsonDateTime::now();
        let updated = self.role_repo.update(&role).await?;
        self.permission_cache.invalidate_all().await;

        let mut perm_dtos: Vec<PermissionResponse> = permissions
            .into_iter()
            .map(PermissionResponse::from)
            .collect();
        perm_dtos.sort_by(|a, b| a.name.cmp(&b.name));
        let perm_names: Vec<String> = perm_dtos.iter().map(|p| p.name.clone()).collect();
        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "role.permissions_updated",
                    "role",
                    format!(
                        "Updated permissions for role '{}' ({} permissions)",
                        updated.name,
                        perm_names.len()
                    ),
                )
                .with_resource(updated.id.to_hex(), updated.name.clone())
                .with_details(json!({
                    "permissions": perm_names,
                })),
            )
            .await;

        Ok(RoleResponse::from_role(updated, Some(perm_dtos)))
    }

    pub async fn get_assigned_users(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
    ) -> Result<Vec<UserResponse>, AppError> {
        actor.require_all_permissions(&["roles.read", "users.read"])?;
        let role_id = parse_object_id(id)?;
        let role = self.find_role_or_not_found(role_id).await?;
        let users = self.user_repo.find_by_role_id(role.id).await?;

        let summary = RoleSummary::from(&role);
        Ok(users
            .into_iter()
            .map(|u| UserResponse::from_user(u, vec![summary.clone()], None))
            .collect())
    }

    async fn find_role_or_not_found(&self, id: ObjectId) -> Result<Role, AppError> {
        self.role_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::not_found("ROLE_NOT_FOUND", "Role not found"))
    }

    async fn load_role_permissions(
        &self,
        role: &Role,
    ) -> Result<Vec<PermissionResponse>, AppError> {
        let mut perms = self.permission_repo.find_by_ids(&role.permissions).await?;
        perms.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(perms.into_iter().map(PermissionResponse::from).collect())
    }

    async fn resolve_and_validate_permissions(
        &self,
        permission_ids: &[String],
    ) -> Result<Vec<Permission>, AppError> {
        let mut parsed_ids = Vec::with_capacity(permission_ids.len());
        let mut seen = HashSet::new();
        for raw_id in permission_ids {
            let oid = parse_object_id(raw_id)?;
            if seen.insert(oid) {
                parsed_ids.push(oid);
            }
        }

        if parsed_ids.is_empty() {
            return Ok(Vec::new());
        }

        let perms = self.permission_repo.find_by_ids(&parsed_ids).await?;
        if perms.len() != parsed_ids.len() {
            return Err(AppError::bad_request(
                "INVALID_PERMISSION_REFERENCE",
                "One or more specified permission IDs do not exist",
            ));
        }

        Ok(perms)
    }

    fn ensure_actor_holds_permissions(
        &self,
        actor: &AuthenticatedUser,
        permissions: &[Permission],
    ) -> Result<(), AppError> {
        if actor.is_super_admin {
            return Ok(());
        }

        for perm in permissions {
            if !actor.has_permission(&perm.name) {
                return Err(AppError::forbidden(
                    "PRIVILEGE_ESCALATION_DENIED",
                    format!(
                        "Cannot grant permission '{}' that you do not hold",
                        perm.name
                    ),
                ));
            }
        }

        Ok(())
    }
}
