use std::sync::Arc;

use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use serde_json::json;

use crate::{
    common::{
        errors::AppError,
        pagination::{PaginatedResponse, PaginationParams},
        validation::{parse_object_id, validate_dto},
    },
    middleware::{AuthenticatedUser, PermissionCache},
    repository::{
        permission::{
            dao::permission::Permission,
            permission_repository::{PermissionFilter, PermissionRepository},
        },
        role::role_repository::RoleRepository,
    },
    services::audit::{AuditEvent, AuditService},
    web::dto::{
        permissions::{
            CreatePermissionRequest, ListPermissionsQuery, PermissionResponse,
            UpdatePermissionRequest,
        },
        roles::RoleSummary,
    },
};

#[derive(Clone)]
pub struct PermissionService {
    permission_repo: Arc<dyn PermissionRepository>,
    role_repo: Arc<dyn RoleRepository>,
    permission_cache: PermissionCache,
    audit_service: AuditService,
}

impl PermissionService {
    pub fn new(
        permission_repo: Arc<dyn PermissionRepository>,
        role_repo: Arc<dyn RoleRepository>,
        permission_cache: PermissionCache,
        audit_service: AuditService,
    ) -> Self {
        Self {
            permission_repo,
            role_repo,
            permission_cache,
            audit_service,
        }
    }

    pub async fn list(
        &self,
        actor: &AuthenticatedUser,
        query: ListPermissionsQuery,
    ) -> Result<PaginatedResponse<PermissionResponse>, AppError> {
        actor.require_permission("permissions.read")?;

        let pagination = PaginationParams {
            page: query.page,
            page_size: query.page_size,
            sort_by: query.sort_by,
            sort_order: query.sort_order,
        };

        let filter = PermissionFilter {
            search: query.search,
            resource: query.resource,
            skip: pagination.skip(),
            limit: pagination.normalized_page_size(),
            sort_by: pagination
                .sort_by
                .clone()
                .unwrap_or_else(|| "name".to_string()),
            sort_order: Some(pagination.sort_order_or_default()),
        };

        let (perms, total) = self.permission_repo.find_all(&filter).await?;
        let items: Vec<PermissionResponse> =
            perms.into_iter().map(PermissionResponse::from).collect();

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
    ) -> Result<PermissionResponse, AppError> {
        actor.require_permission("permissions.read")?;
        let perm_id = parse_object_id(id)?;
        let perm = self.find_permission_or_not_found(perm_id).await?;
        Ok(PermissionResponse::from(perm))
    }

    pub async fn create(
        &self,
        actor: &AuthenticatedUser,
        req: CreatePermissionRequest,
    ) -> Result<PermissionResponse, AppError> {
        actor.require_permission("permissions.create")?;
        validate_dto(&req)?;

        let resource = req.resource.trim().to_lowercase();
        let action = req.action.trim().to_lowercase();
        let name = format!("{resource}.{action}");

        if self.permission_repo.find_by_name(&name).await?.is_some() {
            return Err(AppError::conflict(
                "PERMISSION_ALREADY_EXISTS",
                format!("Permission '{name}' already exists"),
            ));
        }

        let now = BsonDateTime::now();
        let permission = Permission {
            id: ObjectId::new(),
            name,
            description: req.description.trim().to_string(),
            resource,
            action,
            is_system: false,
            created_at: now,
            updated_at: now,
        };

        let created = self.permission_repo.create(&permission).await?;
        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "permission.created",
                    "permission",
                    format!("Created permission '{}'", created.name),
                )
                .with_resource(created.id.to_hex(), created.name.clone())
                .with_details(json!({
                    "resource": created.resource,
                    "action": created.action,
                    "description": created.description,
                })),
            )
            .await;
        Ok(PermissionResponse::from(created))
    }

    pub async fn update(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
        req: UpdatePermissionRequest,
    ) -> Result<PermissionResponse, AppError> {
        actor.require_permission("permissions.update")?;
        validate_dto(&req)?;

        let perm_id = parse_object_id(id)?;
        let mut perm = self.find_permission_or_not_found(perm_id).await?;

        if perm.is_system {
            return Err(AppError::forbidden(
                "SYSTEM_PERMISSION_PROTECTED",
                "Protected system permissions cannot be modified",
            ));
        }

        let previous_name = perm.name.clone();
        let new_resource = req
            .resource
            .map(|r| r.trim().to_lowercase())
            .unwrap_or_else(|| perm.resource.clone());
        let new_action = req
            .action
            .map(|a| a.trim().to_lowercase())
            .unwrap_or_else(|| perm.action.clone());
        let new_name = format!("{new_resource}.{new_action}");

        if new_name != perm.name {
            if self
                .permission_repo
                .find_by_name(&new_name)
                .await?
                .is_some()
            {
                return Err(AppError::conflict(
                    "PERMISSION_ALREADY_EXISTS",
                    format!("Permission '{new_name}' already exists"),
                ));
            }
            perm.resource = new_resource;
            perm.action = new_action;
            perm.name = new_name;
        }

        if let Some(description) = req.description {
            perm.description = description.trim().to_string();
        }

        perm.updated_at = BsonDateTime::now();
        let updated = self.permission_repo.update(&perm).await?;
        self.permission_cache.invalidate_all().await;
        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "permission.updated",
                    "permission",
                    format!("Updated permission '{}'", updated.name),
                )
                .with_resource(updated.id.to_hex(), updated.name.clone())
                .with_details(json!({
                    "previous_name": previous_name,
                    "name": updated.name,
                    "resource": updated.resource,
                    "action": updated.action,
                    "description": updated.description,
                })),
            )
            .await;
        Ok(PermissionResponse::from(updated))
    }

    pub async fn delete(&self, actor: &AuthenticatedUser, id: &str) -> Result<(), AppError> {
        actor.require_permission("permissions.delete")?;
        let perm_id = parse_object_id(id)?;
        let perm = self.find_permission_or_not_found(perm_id).await?;

        if perm.is_system {
            return Err(AppError::forbidden(
                "SYSTEM_PERMISSION_PROTECTED",
                "Protected system permissions cannot be deleted",
            ));
        }

        self.role_repo
            .remove_permission_from_all_roles(perm_id)
            .await?;
        self.permission_repo.delete(perm_id).await?;
        self.permission_cache.invalidate_all().await;
        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "permission.deleted",
                    "permission",
                    format!("Deleted permission '{}'", perm.name),
                )
                .with_resource(perm.id.to_hex(), perm.name.clone())
                .with_details(json!({
                    "resource": perm.resource,
                    "action": perm.action,
                })),
            )
            .await;
        Ok(())
    }

    pub async fn get_associated_roles(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
    ) -> Result<Vec<RoleSummary>, AppError> {
        actor.require_all_permissions(&["permissions.read", "roles.read"])?;
        let perm_id = parse_object_id(id)?;
        let _ = self.find_permission_or_not_found(perm_id).await?;
        let roles = self.role_repo.find_by_permission_id(perm_id).await?;
        Ok(roles.iter().map(RoleSummary::from).collect())
    }

    async fn find_permission_or_not_found(&self, id: ObjectId) -> Result<Permission, AppError> {
        self.permission_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::not_found("PERMISSION_NOT_FOUND", "Permission not found"))
    }
}
