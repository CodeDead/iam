use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use serde_json::json;

use crate::{
    common::{
        errors::AppError,
        pagination::{PaginatedResponse, PaginationParams, SortOrder},
        validation::{normalize_email, parse_object_id, validate_dto},
    },
    database::mongodb::bson_dt_to_chrono,
    middleware::AuthenticatedUser,
    repository::{
        organisation::{
            dao::organisation::{
                Organisation, OrganisationMember, OrganisationMemberRole, OrganisationMemberStatus,
            },
            organisation_repository::{OrganisationFilter, OrganisationRepository},
        },
        user::{
            dao::user::{AccountStatus, User},
            user_repository::UserRepository,
        },
    },
    services::audit::{AuditEvent, AuditService},
    web::dto::organisations::{
        AddOrganisationMemberRequest, CreateOrganisationRequest, ListOrganisationsQuery,
        OrganisationMemberResponse, OrganisationResponse, UpdateOrganisationMemberRequest,
        UpdateOrganisationRequest, UserOrganisationSummary,
    },
};

const MAX_SELF_SERVICE_OWNED_ORGANISATIONS: usize = 10;
const MAX_PENDING_INVITATIONS_PER_ORG: usize = 100;

#[derive(Clone)]
pub struct OrganisationService {
    organisation_repo: Arc<dyn OrganisationRepository>,
    user_repo: Arc<dyn UserRepository>,
    audit_service: AuditService,
}

impl OrganisationService {
    pub fn new(
        organisation_repo: Arc<dyn OrganisationRepository>,
        user_repo: Arc<dyn UserRepository>,
        audit_service: AuditService,
    ) -> Self {
        Self {
            organisation_repo,
            user_repo,
            audit_service,
        }
    }

    pub async fn list_my_organisations(
        &self,
        actor: &AuthenticatedUser,
    ) -> Result<Vec<UserOrganisationSummary>, AppError> {
        let orgs = self
            .organisation_repo
            .find_by_member_user_id(actor.user_id)
            .await?;

        let mut summaries = Vec::with_capacity(orgs.len());
        for org in orgs {
            if let Some(membership) = org.members.iter().find(|m| m.user_id == actor.user_id) {
                summaries.push(build_user_organisation_summary(&org, membership));
            }
        }
        Ok(summaries)
    }

    pub async fn list(
        &self,
        actor: &AuthenticatedUser,
        query: ListOrganisationsQuery,
    ) -> Result<PaginatedResponse<OrganisationResponse>, AppError> {
        actor.require_permission("organisations.read")?;
        let can_read_users = actor.has_permission("users.read");
        let pagination = PaginationParams {
            page: query.page,
            page_size: query.page_size,
            sort_by: query.sort_by.clone(),
            sort_order: query.sort_order,
        };

        let filter = OrganisationFilter {
            search: query.search,
            skip: pagination.skip(),
            limit: pagination.normalized_page_size(),
            sort_by: query.sort_by.unwrap_or_else(|| "created_at".to_string()),
            sort_order: query.sort_order.or(Some(SortOrder::Desc)),
        };

        let (orgs, total) = self.organisation_repo.find_all(&filter).await?;
        let user_map = self.load_member_users(&orgs).await?;
        let items = orgs
            .into_iter()
            .map(|org| build_organisation_response(org, &user_map, can_read_users))
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
    ) -> Result<OrganisationResponse, AppError> {
        let org_id = parse_object_id(id)?;
        let org = self
            .find_or_not_found_for_actor(actor, org_id, false)
            .await?;
        let can_read_global = actor.has_permission("organisations.read");
        let is_org_manager = active_member_role(&org, actor.user_id)
            .is_some_and(OrganisationMemberRole::can_manage_members);

        if !can_read_global && !is_org_manager {
            return Err(AppError::forbidden(
                "INSUFFICIENT_PERMISSIONS",
                "You do not have permission to view this organisation's member roster",
            ));
        }

        let user_map = self.load_member_users(std::slice::from_ref(&org)).await?;
        Ok(build_organisation_response(
            org,
            &user_map,
            actor.has_permission("users.read"),
        ))
    }

    pub async fn create(
        &self,
        actor: &AuthenticatedUser,
        req: CreateOrganisationRequest,
    ) -> Result<OrganisationResponse, AppError> {
        validate_dto(&req)?;

        let has_global_create = actor.has_permission("organisations.create");
        let has_global_manage = actor.has_permission("organisations.manage_members");

        if !has_global_manage
            && (req
                .initial_member_ids
                .as_ref()
                .is_some_and(|ids| !ids.is_empty())
                || req
                    .initial_owner_email
                    .as_deref()
                    .is_some_and(|email| !email.trim().is_empty())
                || req.include_creator == Some(false))
        {
            return Err(AppError::forbidden(
                "PRIVILEGE_ESCALATION_DENIED",
                "Only authorized administrators may configure creator inclusion, initial members, or another initial owner",
            ));
        }

        // Self-service creators are always added as active owner to prevent orphan organisations.
        let include_creator = if has_global_manage {
            req.include_creator.unwrap_or(true)
        } else {
            true
        };

        if !has_global_create && include_creator {
            let existing = self
                .organisation_repo
                .find_by_member_user_id(actor.user_id)
                .await?;
            let owned_count = existing
                .iter()
                .filter(|org| {
                    active_member_role(org, actor.user_id) == Some(OrganisationMemberRole::Owner)
                })
                .count();
            if owned_count >= MAX_SELF_SERVICE_OWNED_ORGANISATIONS {
                return Err(AppError::conflict(
                    "ORGANISATION_LIMIT_REACHED",
                    format!(
                        "You may own at most {MAX_SELF_SERVICE_OWNED_ORGANISATIONS} organisations"
                    ),
                ));
            }
        }

        let trimmed_name = req.name.trim().to_string();
        let name_chars = trimmed_name.chars().count();
        if !(2..=120).contains(&name_chars) {
            return Err(AppError::validation(
                "Organisation name must be 2 to 120 characters",
                HashMap::from([(
                    "name".to_string(),
                    vec!["Organisation name must be 2 to 120 characters".to_string()],
                )]),
            ));
        }

        if self
            .organisation_repo
            .find_by_name(&trimmed_name)
            .await?
            .is_some()
        {
            return Err(AppError::conflict(
                "ORGANISATION_ALREADY_EXISTS",
                "An organisation with this name already exists",
            ));
        }

        let now = BsonDateTime::now();
        let mut seen_ids: HashSet<ObjectId> = HashSet::new();
        let mut members: Vec<OrganisationMember> = Vec::new();
        let mut assigned_owner_email: Option<String> = None;

        if include_creator {
            seen_ids.insert(actor.user_id);
            members.push(OrganisationMember {
                user_id: actor.user_id,
                role: OrganisationMemberRole::Owner,
                status: OrganisationMemberStatus::Active,
                joined_at: now,
            });
        }

        if let Some(ref raw_owner_email) = req.initial_owner_email {
            let normalized = normalize_email(raw_owner_email);
            if !normalized.is_empty() {
                let owner_user = self
                    .user_repo
                    .find_by_email(&normalized)
                    .await?
                    .ok_or_else(|| {
                        AppError::not_found(
                            "USER_NOT_FOUND",
                            "No user account found with that initial owner email address",
                        )
                    })?;
                if owner_user.status != AccountStatus::Active {
                    return Err(AppError::validation(
                        "Initial organisation owner must have an active user account",
                        HashMap::from([(
                            "initial_owner_email".to_string(),
                            vec![
                                "Initial organisation owner must have an active user account"
                                    .to_string(),
                            ],
                        )]),
                    ));
                }
                assigned_owner_email = Some(owner_user.email.clone());
                if seen_ids.insert(owner_user.id) {
                    members.push(OrganisationMember {
                        user_id: owner_user.id,
                        role: OrganisationMemberRole::Owner,
                        status: OrganisationMemberStatus::Active,
                        joined_at: now,
                    });
                }
            }
        }

        if let Some(raw_ids) = req.initial_member_ids {
            let mut parsed_ids: Vec<(String, ObjectId)> = Vec::new();
            for raw_id in raw_ids {
                let uid = parse_object_id(&raw_id)?;
                if seen_ids.insert(uid) {
                    parsed_ids.push((raw_id, uid));
                }
            }

            if !parsed_ids.is_empty() {
                let id_list: Vec<ObjectId> = parsed_ids.iter().map(|(_, id)| *id).collect();
                let found_users = self.user_repo.find_by_ids(&id_list).await?;
                let found_map: HashMap<ObjectId, User> =
                    found_users.into_iter().map(|u| (u.id, u)).collect();
                for (raw_id, uid) in parsed_ids {
                    let Some(user) = found_map.get(&uid) else {
                        return Err(AppError::not_found(
                            "USER_NOT_FOUND",
                            format!("User '{raw_id}' was not found"),
                        ));
                    };
                    // If an admin omitted themselves, promote the first initial member to active owner
                    // so no organisation is ever created without an active owner.
                    let role = if members.is_empty() {
                        if user.status != AccountStatus::Active {
                            return Err(AppError::validation(
                                "Initial organisation owner must have an active user account",
                                HashMap::from([(
                                    "initial_member_ids".to_string(),
                                    vec![
                                        "Initial organisation owner must have an active user account"
                                            .to_string(),
                                    ],
                                )]),
                            ));
                        }
                        assigned_owner_email = Some(user.email.clone());
                        OrganisationMemberRole::Owner
                    } else {
                        OrganisationMemberRole::Member
                    };
                    members.push(OrganisationMember {
                        user_id: uid,
                        role,
                        status: OrganisationMemberStatus::Active,
                        joined_at: now,
                    });
                }
            }
        }

        if members.is_empty() {
            return Err(AppError::validation(
                "An organisation must have at least one active owner",
                HashMap::from([(
                    "include_creator".to_string(),
                    vec![
                        "Include the creator or provide at least one initial member to own the organisation"
                            .to_string(),
                    ],
                )]),
            ));
        }

        let org = Organisation {
            id: ObjectId::new(),
            name: trimmed_name,
            members,
            version: 0,
            created_at: now,
            updated_at: now,
        };

        let created = self.organisation_repo.create(&org).await?;

        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "organisation.created",
                    "organisation",
                    format!("Created organisation '{}'", created.name),
                )
                .with_resource(created.id.to_hex(), created.name.clone())
                .with_details(json!({
                    "name": created.name,
                    "member_count": created.members.len(),
                    "include_creator": include_creator,
                    "initial_owner_email": assigned_owner_email,
                })),
            )
            .await;

        let user_map = self
            .load_member_users(std::slice::from_ref(&created))
            .await?;
        Ok(build_organisation_response(
            created,
            &user_map,
            actor.has_permission("users.read"),
        ))
    }

    pub async fn update(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
        req: UpdateOrganisationRequest,
    ) -> Result<OrganisationResponse, AppError> {
        validate_dto(&req)?;

        let org_id = parse_object_id(id)?;
        let can_update_global = actor.has_permission("organisations.update");
        let mut org = self
            .find_or_not_found_for_actor(actor, org_id, can_update_global)
            .await?;
        let is_owner =
            active_member_role(&org, actor.user_id) == Some(OrganisationMemberRole::Owner);

        if !can_update_global && !is_owner {
            return Err(AppError::forbidden(
                "INSUFFICIENT_PERMISSIONS",
                "Only an organisation owner or authorized administrator can update organisation details",
            ));
        }

        let trimmed_name = req.name.trim().to_string();
        let name_chars = trimmed_name.chars().count();
        if !(2..=120).contains(&name_chars) {
            return Err(AppError::validation(
                "Organisation name must be 2 to 120 characters",
                HashMap::from([(
                    "name".to_string(),
                    vec!["Organisation name must be 2 to 120 characters".to_string()],
                )]),
            ));
        }

        if trimmed_name.to_lowercase() != org.name.to_lowercase()
            && self
                .organisation_repo
                .find_by_name(&trimmed_name)
                .await?
                .is_some()
        {
            return Err(AppError::conflict(
                "ORGANISATION_ALREADY_EXISTS",
                "Another organisation with this name already exists",
            ));
        }

        let previous_name = org.name.clone();
        org.name = trimmed_name;
        org.updated_at = BsonDateTime::now();
        let updated = self.organisation_repo.update(&org).await?;

        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "organisation.updated",
                    "organisation",
                    format!("Updated organisation '{}'", updated.name),
                )
                .with_resource(updated.id.to_hex(), updated.name.clone())
                .with_details(json!({
                    "previous_name": previous_name,
                    "name": updated.name,
                })),
            )
            .await;

        let user_map = self
            .load_member_users(std::slice::from_ref(&updated))
            .await?;
        Ok(build_organisation_response(
            updated,
            &user_map,
            actor.has_permission("users.read"),
        ))
    }

    pub async fn delete(&self, actor: &AuthenticatedUser, id: &str) -> Result<(), AppError> {
        let org_id = parse_object_id(id)?;
        let can_delete_global = actor.has_permission("organisations.delete");
        let org = self
            .find_or_not_found_for_actor(actor, org_id, can_delete_global)
            .await?;
        let is_owner =
            active_member_role(&org, actor.user_id) == Some(OrganisationMemberRole::Owner);

        if !can_delete_global && !is_owner {
            return Err(AppError::forbidden(
                "INSUFFICIENT_PERMISSIONS",
                "Only an organisation owner or authorized administrator can delete this organisation",
            ));
        }

        let deleted = self.organisation_repo.delete(org_id).await?;
        if !deleted {
            if can_delete_global || actor.has_permission("organisations.read") {
                return Err(AppError::not_found(
                    "ORGANISATION_NOT_FOUND",
                    "Organisation not found",
                ));
            }
            return Err(AppError::forbidden(
                "INSUFFICIENT_PERMISSIONS",
                "You do not have permission to access this organisation",
            ));
        }

        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "organisation.deleted",
                    "organisation",
                    format!("Deleted organisation '{}'", org.name),
                )
                .with_resource(org.id.to_hex(), org.name.clone())
                .with_details(json!({
                    "name": org.name,
                    "member_count": org.members.len(),
                }))
                .as_warning(),
            )
            .await;

        Ok(())
    }

    pub async fn add_member(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
        req: AddOrganisationMemberRequest,
    ) -> Result<OrganisationResponse, AppError> {
        validate_dto(&req)?;

        let org_id = parse_object_id(id)?;
        let has_global_manage = actor.has_permission("organisations.manage_members");
        let mut org = self
            .find_or_not_found_for_actor(actor, org_id, has_global_manage)
            .await?;

        let actor_org_role = active_member_role(&org, actor.user_id);
        let is_org_manager = actor_org_role.is_some_and(OrganisationMemberRole::can_manage_members);

        if !has_global_manage && !is_org_manager {
            return Err(AppError::forbidden(
                "INSUFFICIENT_PERMISSIONS",
                "You do not have permission to manage members of this organisation",
            ));
        }

        // Security: self-service organisation managers must invite by signup email only
        // (never by raw user_id probing) and cannot assign owner role or bypass user consent.
        if !has_global_manage && req.user_id.is_some() {
            return Err(AppError::forbidden(
                "EMAIL_REQUIRED_FOR_ORG_INVITE",
                "Organisation managers must add users using their signup email address",
            ));
        }

        let requested_role = req.role.unwrap_or(OrganisationMemberRole::Member);
        if !has_global_manage {
            let allowed_role = match actor_org_role {
                Some(OrganisationMemberRole::Owner) => {
                    matches!(
                        requested_role,
                        OrganisationMemberRole::Member | OrganisationMemberRole::Admin
                    )
                }
                Some(OrganisationMemberRole::Admin) => {
                    requested_role == OrganisationMemberRole::Member
                }
                _ => false,
            };
            if !allowed_role {
                return Err(AppError::forbidden(
                    "PRIVILEGE_ESCALATION_DENIED",
                    "You are not authorized to assign that organisation role",
                ));
            }
        }

        if req.user_id.is_some() && req.email.is_some() {
            return Err(AppError::validation(
                "Provide either user_id or email, not both",
                HashMap::from([(
                    "email".to_string(),
                    vec!["Provide either user_id or email, not both".to_string()],
                )]),
            ));
        }

        let target_user = if let Some(ref raw_uid) = req.user_id {
            let uid = parse_object_id(raw_uid)?;
            self.user_repo.find_by_id(uid).await?.ok_or_else(|| {
                AppError::not_found("USER_NOT_FOUND", "Specified user does not exist")
            })?
        } else if let Some(ref email) = req.email {
            let normalized = normalize_email(email);
            let found = self
                .user_repo
                .find_by_email(&normalized)
                .await?
                .ok_or_else(|| {
                    AppError::not_found(
                        "USER_NOT_FOUND",
                        "No user account found with that email address",
                    )
                })?;
            // Self-service managers cannot invite inactive/disabled/locked accounts (returning the
            // same 404 prevents leaking whether an account is disabled or locked).
            if !has_global_manage && found.status != AccountStatus::Active {
                return Err(AppError::not_found(
                    "USER_NOT_FOUND",
                    "No user account found with that email address",
                ));
            }
            found
        } else {
            return Err(AppError::validation(
                "Either user_id or email is required to add an organisation member",
                HashMap::from([(
                    "email".to_string(),
                    vec!["Signup email address is required".to_string()],
                )]),
            ));
        };

        if org.members.iter().any(|m| m.user_id == target_user.id) {
            return Err(AppError::conflict(
                "MEMBER_ALREADY_EXISTS",
                "User is already a member of this organisation",
            ));
        }

        let now = BsonDateTime::now();
        // Self-service org managers always send an invitation (`invited`) so user details remain
        // private until the user explicitly accepts membership. Platform admins may specify status.
        let status = if has_global_manage {
            req.status.unwrap_or(OrganisationMemberStatus::Active)
        } else {
            let pending_count = org
                .members
                .iter()
                .filter(|m| m.status == OrganisationMemberStatus::Invited)
                .count();
            if pending_count >= MAX_PENDING_INVITATIONS_PER_ORG {
                return Err(AppError::conflict(
                    "PENDING_INVITATION_LIMIT_REACHED",
                    format!(
                        "An organisation may have at most {MAX_PENDING_INVITATIONS_PER_ORG} pending invitations"
                    ),
                ));
            }
            OrganisationMemberStatus::Invited
        };

        org.members.push(OrganisationMember {
            user_id: target_user.id,
            role: requested_role,
            status,
            joined_at: now,
        });
        org.updated_at = now;
        let updated = self.organisation_repo.update(&org).await?;

        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "organisation.member_added",
                    "organisation",
                    format!(
                        "Added '{}' to organisation '{}' ({}, {})",
                        target_user.email,
                        updated.name,
                        requested_role.as_str(),
                        status.as_str()
                    ),
                )
                .with_resource(updated.id.to_hex(), updated.name.clone())
                .with_details(json!({
                    "member_user_id": target_user.id.to_hex(),
                    "member_email": target_user.email,
                    "role": requested_role.as_str(),
                    "status": status.as_str(),
                })),
            )
            .await;

        let user_map = self
            .load_member_users(std::slice::from_ref(&updated))
            .await?;
        Ok(build_organisation_response(
            updated,
            &user_map,
            actor.has_permission("users.read"),
        ))
    }

    pub async fn update_member(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
        member_user_id: &str,
        req: UpdateOrganisationMemberRequest,
    ) -> Result<OrganisationResponse, AppError> {
        validate_dto(&req)?;
        if req.status.is_none() && req.role.is_none() {
            return Err(AppError::validation(
                "At least one of status or role must be provided",
                HashMap::from([(
                    "status".to_string(),
                    vec!["Provide a new status or role".to_string()],
                )]),
            ));
        }

        let org_id = parse_object_id(id)?;
        let target_uid = parse_object_id(member_user_id)?;
        let has_global_manage = actor.has_permission("organisations.manage_members");
        let mut org = self
            .find_or_not_found_for_actor(actor, org_id, has_global_manage)
            .await?;

        let actor_org_role = active_member_role(&org, actor.user_id);
        let is_org_manager = actor_org_role.is_some_and(OrganisationMemberRole::can_manage_members);
        if !has_global_manage && !is_org_manager {
            return Err(AppError::forbidden(
                "INSUFFICIENT_PERMISSIONS",
                "You do not have permission to manage members of this organisation",
            ));
        }

        let active_owner_count = count_active_owners(&org);
        let member = org
            .members
            .iter_mut()
            .find(|m| m.user_id == target_uid)
            .ok_or_else(|| {
                AppError::not_found(
                    "MEMBER_NOT_FOUND",
                    "User is not a member of this organisation",
                )
            })?;

        let previous_status = member.status;
        let previous_role = member.role;

        if !has_global_manage {
            // Org admins cannot modify owners or other admins, and cannot change roles.
            if actor_org_role == Some(OrganisationMemberRole::Admin) {
                if previous_role != OrganisationMemberRole::Member {
                    return Err(AppError::forbidden(
                        "PRIVILEGE_ESCALATION_DENIED",
                        "Only an organisation owner can modify owners or administrators",
                    ));
                }
                if req.role.is_some() {
                    return Err(AppError::forbidden(
                        "PRIVILEGE_ESCALATION_DENIED",
                        "Only an organisation owner can change member roles",
                    ));
                }
            }
            // Self-service managers cannot mutate status on a pending invitation (blocks both
            // direct Invited -> Active and two-step Invited -> Suspended -> Active consent bypasses)
            // and cannot downgrade an existing member back to Invited.
            if previous_status == OrganisationMemberStatus::Invited && req.status.is_some() {
                return Err(AppError::forbidden(
                    "INVITATION_ACCEPTANCE_REQUIRED",
                    "Pending invitations cannot have their status changed before the user accepts; cancel the invitation instead",
                ));
            }
            if req.status == Some(OrganisationMemberStatus::Invited) {
                return Err(AppError::forbidden(
                    "INVALID_STATUS_TRANSITION",
                    "Existing members cannot be reverted to invited status",
                ));
            }
            // Prevent self-service org owners from promoting pending/suspended members to owner.
            let resulting_status = req.status.unwrap_or(previous_status);
            if req.role == Some(OrganisationMemberRole::Owner)
                && resulting_status != OrganisationMemberStatus::Active
            {
                return Err(AppError::forbidden(
                    "INVALID_ROLE_TRANSITION",
                    "Only active members can be promoted to organisation owner",
                ));
            }
        }

        let next_status = req.status.unwrap_or(previous_status);
        let next_role = req.role.unwrap_or(previous_role);

        let was_active_owner = previous_role == OrganisationMemberRole::Owner
            && previous_status == OrganisationMemberStatus::Active;
        let still_active_owner = next_role == OrganisationMemberRole::Owner
            && next_status == OrganisationMemberStatus::Active;

        if was_active_owner && !still_active_owner && active_owner_count <= 1 {
            return Err(AppError::conflict(
                "LAST_ORG_OWNER_PROTECTED",
                "Cannot demote or deactivate the last active owner of an organisation",
            ));
        }

        member.status = next_status;
        member.role = next_role;
        org.updated_at = BsonDateTime::now();
        let updated = self.organisation_repo.update(&org).await?;
        let user_map = self
            .load_member_users(std::slice::from_ref(&updated))
            .await?;
        let target_email = user_map.get(&target_uid).map(|u| u.email.clone());
        let target_label = target_email.clone().unwrap_or_else(|| target_uid.to_hex());

        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "organisation.member_updated",
                    "organisation",
                    format!(
                        "Updated member '{}' in organisation '{}' ({}, {})",
                        target_label,
                        updated.name,
                        next_role.as_str(),
                        next_status.as_str()
                    ),
                )
                .with_resource(updated.id.to_hex(), updated.name.clone())
                .with_details(json!({
                    "member_user_id": target_uid.to_hex(),
                    "member_email": target_email,
                    "previous_role": previous_role.as_str(),
                    "role": next_role.as_str(),
                    "previous_status": previous_status.as_str(),
                    "status": next_status.as_str(),
                })),
            )
            .await;

        Ok(build_organisation_response(
            updated,
            &user_map,
            actor.has_permission("users.read"),
        ))
    }

    pub async fn remove_member(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
        member_user_id: &str,
    ) -> Result<OrganisationResponse, AppError> {
        let org_id = parse_object_id(id)?;
        let target_uid = parse_object_id(member_user_id)?;
        let has_global_manage = actor.has_permission("organisations.manage_members");
        let mut org = self
            .find_or_not_found_for_actor(actor, org_id, has_global_manage)
            .await?;

        let actor_org_role = active_member_role(&org, actor.user_id);
        let is_org_manager = actor_org_role.is_some_and(OrganisationMemberRole::can_manage_members);
        if !has_global_manage && !is_org_manager {
            return Err(AppError::forbidden(
                "INSUFFICIENT_PERMISSIONS",
                "You do not have permission to remove members from this organisation",
            ));
        }

        let target_member = org
            .members
            .iter()
            .find(|m| m.user_id == target_uid)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "MEMBER_NOT_FOUND",
                    "User is not a member of this organisation",
                )
            })?;

        if !has_global_manage
            && actor_org_role == Some(OrganisationMemberRole::Admin)
            && target_member.role != OrganisationMemberRole::Member
        {
            return Err(AppError::forbidden(
                "PRIVILEGE_ESCALATION_DENIED",
                "Only an organisation owner can remove owners or administrators",
            ));
        }

        let active_owner_count = count_active_owners(&org);
        if target_member.role == OrganisationMemberRole::Owner
            && target_member.status == OrganisationMemberStatus::Active
            && active_owner_count <= 1
        {
            return Err(AppError::conflict(
                "LAST_ORG_OWNER_PROTECTED",
                "Assign another active owner before removing the last active owner, or delete the organisation instead",
            ));
        }

        let was_invited = target_member.status == OrganisationMemberStatus::Invited;
        let target_email = self
            .user_repo
            .find_by_id(target_uid)
            .await?
            .map(|u| u.email);
        let target_label = target_email.clone().unwrap_or_else(|| target_uid.to_hex());
        org.members.retain(|m| m.user_id != target_uid);
        org.updated_at = BsonDateTime::now();
        let updated = self.organisation_repo.update(&org).await?;

        let (audit_action, audit_summary) = if was_invited {
            (
                "organisation.invitation_cancelled",
                format!(
                    "Cancelled pending invitation for '{}' in organisation '{}'",
                    target_label, updated.name
                ),
            )
        } else {
            (
                "organisation.member_removed",
                format!(
                    "Removed member '{}' from organisation '{}'",
                    target_label, updated.name
                ),
            )
        };

        let mut audit_event =
            AuditEvent::by_actor(actor, audit_action, "organisation", audit_summary)
                .with_resource(updated.id.to_hex(), updated.name.clone())
                .with_details(json!({
                    "member_user_id": target_uid.to_hex(),
                    "member_email": target_email,
                    "previous_status": target_member.status.as_str(),
                }));
        if !was_invited {
            audit_event = audit_event.as_warning();
        }
        self.audit_service.record(audit_event).await;

        let user_map = self
            .load_member_users(std::slice::from_ref(&updated))
            .await?;
        Ok(build_organisation_response(
            updated,
            &user_map,
            actor.has_permission("users.read"),
        ))
    }

    pub async fn accept_invitation(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
    ) -> Result<UserOrganisationSummary, AppError> {
        let org_id = parse_object_id(id)?;
        let mut org = self
            .find_or_not_found_for_actor(actor, org_id, false)
            .await?;

        let member = org
            .members
            .iter_mut()
            .find(|m| m.user_id == actor.user_id)
            .ok_or_else(|| {
                AppError::forbidden(
                    "INSUFFICIENT_PERMISSIONS",
                    "You do not have an invitation to this organisation",
                )
            })?;

        if member.status != OrganisationMemberStatus::Invited {
            return Err(AppError::conflict(
                "INVITATION_NOT_PENDING",
                "Your membership in this organisation is not pending acceptance",
            ));
        }

        let now = BsonDateTime::now();
        member.status = OrganisationMemberStatus::Active;
        member.joined_at = now;
        let role = member.role;
        org.updated_at = now;
        let updated = self.organisation_repo.update(&org).await?;

        self.audit_service
            .record(
                AuditEvent::by_actor(
                    actor,
                    "organisation.member_accepted",
                    "organisation",
                    format!("Accepted invitation to organisation '{}'", updated.name),
                )
                .with_resource(updated.id.to_hex(), updated.name.clone())
                .with_details(json!({
                    "role": role.as_str(),
                })),
            )
            .await;

        let accepted_membership = OrganisationMember {
            user_id: actor.user_id,
            role,
            status: OrganisationMemberStatus::Active,
            joined_at: now,
        };
        Ok(build_user_organisation_summary(
            &updated,
            &accepted_membership,
        ))
    }

    pub async fn leave_organisation(
        &self,
        actor: &AuthenticatedUser,
        id: &str,
    ) -> Result<(), AppError> {
        let org_id = parse_object_id(id)?;
        let mut org = self
            .find_or_not_found_for_actor(actor, org_id, false)
            .await?;

        let membership = org
            .members
            .iter()
            .find(|m| m.user_id == actor.user_id)
            .cloned()
            .ok_or_else(|| {
                AppError::forbidden(
                    "INSUFFICIENT_PERMISSIONS",
                    "You are not a member of this organisation",
                )
            })?;

        let other_active_members = org
            .members
            .iter()
            .any(|m| m.user_id != actor.user_id && m.status == OrganisationMemberStatus::Active);

        if membership.role == OrganisationMemberRole::Owner
            && membership.status == OrganisationMemberStatus::Active
            && count_active_owners(&org) <= 1
        {
            if other_active_members {
                return Err(AppError::conflict(
                    "LAST_ORG_OWNER_PROTECTED",
                    "Assign another active owner before leaving this organisation",
                ));
            }
            return Err(AppError::conflict(
                "LAST_ORG_OWNER_PROTECTED",
                "You are the sole active member of this organisation; delete the organisation instead of leaving",
            ));
        }

        let was_invited = membership.status == OrganisationMemberStatus::Invited;
        org.members.retain(|m| m.user_id != actor.user_id);
        org.updated_at = BsonDateTime::now();
        self.organisation_repo.update(&org).await?;

        let (audit_action, audit_summary) = if was_invited {
            (
                "organisation.invitation_declined",
                format!("Declined invitation to organisation '{}'", org.name),
            )
        } else {
            (
                "organisation.member_left",
                format!("Left organisation '{}'", org.name),
            )
        };

        self.audit_service
            .record(
                AuditEvent::by_actor(actor, audit_action, "organisation", audit_summary)
                    .with_resource(org.id.to_hex(), org.name.clone())
                    .with_details(json!({
                        "previous_status": membership.status.as_str(),
                    })),
            )
            .await;

        Ok(())
    }

    async fn find_or_not_found_for_actor(
        &self,
        actor: &AuthenticatedUser,
        id: ObjectId,
        has_global_access: bool,
    ) -> Result<Organisation, AppError> {
        let Some(org) = self.organisation_repo.find_by_id(id).await? else {
            // Avoid leaking existence of unknown organisations to callers without global access.
            if has_global_access || actor.has_permission("organisations.read") {
                return Err(AppError::not_found(
                    "ORGANISATION_NOT_FOUND",
                    "Organisation not found",
                ));
            }
            return Err(AppError::forbidden(
                "INSUFFICIENT_PERMISSIONS",
                "You do not have permission to access this organisation",
            ));
        };
        Ok(org)
    }

    async fn load_member_users(
        &self,
        orgs: &[Organisation],
    ) -> Result<HashMap<ObjectId, User>, AppError> {
        let mut unique_ids: HashSet<ObjectId> = HashSet::new();
        for org in orgs {
            for member in &org.members {
                unique_ids.insert(member.user_id);
            }
        }
        if unique_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let id_vec: Vec<ObjectId> = unique_ids.into_iter().collect();
        let found = self.user_repo.find_by_ids(&id_vec).await?;
        let mut users = HashMap::with_capacity(found.len());
        for user in found {
            users.insert(user.id, user);
        }
        Ok(users)
    }
}

/// Phase 1 (pre-deletion): Verifies that deleting `target_user_id` will not leave any shared
/// organisation without an active owner.
pub async fn validate_user_memberships_for_account_delete(
    organisation_repo: &Arc<dyn OrganisationRepository>,
    target_user_id: ObjectId,
) -> Result<(), AppError> {
    let orgs = organisation_repo
        .find_by_member_user_id(target_user_id)
        .await?;

    let mut blocked_orgs: Vec<String> = Vec::new();
    for org in &orgs {
        let user_is_active_owner = org.members.iter().any(|m| {
            m.user_id == target_user_id
                && m.role == OrganisationMemberRole::Owner
                && m.status == OrganisationMemberStatus::Active
        });
        let has_other_active_owner = org.members.iter().any(|m| {
            m.user_id != target_user_id
                && m.role == OrganisationMemberRole::Owner
                && m.status == OrganisationMemberStatus::Active
        });
        let has_other_active_member = org
            .members
            .iter()
            .any(|m| m.user_id != target_user_id && m.status == OrganisationMemberStatus::Active);

        if user_is_active_owner && !has_other_active_owner && has_other_active_member {
            blocked_orgs.push(org.name.clone());
        }
    }

    if !blocked_orgs.is_empty() {
        blocked_orgs.sort();
        return Err(AppError::conflict(
            "LAST_ORG_OWNER_PROTECTED",
            format!(
                "Cannot delete account while last active owner of organisation(s): {}. Promote another active owner or delete the organisation first.",
                blocked_orgs.join(", ")
            ),
        ));
    }

    Ok(())
}

/// Phase 2 (post-deletion): Auto-deletes solo organisations where `target_user_id` was the
/// active owner with no other active members, then pulls the deleted user from all remaining orgs.
pub async fn finalize_user_memberships_on_account_delete(
    organisation_repo: &Arc<dyn OrganisationRepository>,
    audit_service: &AuditService,
    actor: &AuthenticatedUser,
    target_user_id: ObjectId,
) -> Result<(), AppError> {
    let orgs = organisation_repo
        .find_by_member_user_id(target_user_id)
        .await?;

    for org in &orgs {
        let user_is_active_owner = org.members.iter().any(|m| {
            m.user_id == target_user_id
                && m.role == OrganisationMemberRole::Owner
                && m.status == OrganisationMemberStatus::Active
        });
        let has_other_active_member = org
            .members
            .iter()
            .any(|m| m.user_id != target_user_id && m.status == OrganisationMemberStatus::Active);

        if user_is_active_owner && !has_other_active_member {
            organisation_repo.delete(org.id).await?;
            audit_service
                .record(
                    AuditEvent::by_actor(
                        actor,
                        "organisation.deleted",
                        "organisation",
                        format!(
                            "Auto-deleted solo organisation '{}' on owner account deletion",
                            org.name
                        ),
                    )
                    .with_resource(org.id.to_hex(), org.name.clone())
                    .with_details(json!({
                        "name": org.name,
                        "reason": "sole_active_owner_account_deleted",
                        "removed_user_id": target_user_id.to_hex(),
                    }))
                    .as_warning(),
                )
                .await;
        }
    }

    organisation_repo
        .remove_member_from_all_organisations(target_user_id)
        .await?;

    Ok(())
}

fn active_member_role(org: &Organisation, user_id: ObjectId) -> Option<OrganisationMemberRole> {
    org.members
        .iter()
        .find(|m| m.user_id == user_id && m.status == OrganisationMemberStatus::Active)
        .map(|m| m.role)
}

fn count_active_owners(org: &Organisation) -> usize {
    org.members
        .iter()
        .filter(|m| {
            m.role == OrganisationMemberRole::Owner && m.status == OrganisationMemberStatus::Active
        })
        .count()
}

pub(crate) fn build_user_organisation_summary(
    org: &Organisation,
    membership: &OrganisationMember,
) -> UserOrganisationSummary {
    let active_member_count = org
        .members
        .iter()
        .filter(|m| m.status == OrganisationMemberStatus::Active)
        .count() as u64;
    let active_owner_count = count_active_owners(org) as u64;

    UserOrganisationSummary {
        id: org.id.to_hex(),
        name: org.name.clone(),
        role: membership.role,
        status: membership.status,
        member_count: org.members.len() as u64,
        active_member_count,
        active_owner_count,
        joined_at: bson_dt_to_chrono(membership.joined_at),
        created_at: bson_dt_to_chrono(org.created_at),
    }
}

fn build_organisation_response(
    org: Organisation,
    user_map: &HashMap<ObjectId, User>,
    can_read_users: bool,
) -> OrganisationResponse {
    let member_count = org.members.len() as u64;
    let active_member_count = org
        .members
        .iter()
        .filter(|m| m.status == OrganisationMemberStatus::Active)
        .count() as u64;

    let members = org
        .members
        .into_iter()
        .map(|member| {
            let user = user_map.get(&member.user_id);
            // Data minimization: without global `users.read`, personal display names remain
            // redacted for pending invitations until the user accepts, while accepted (active or
            // suspended) members retain their display names in the roster.
            let expose_name = can_read_users || member.status != OrganisationMemberStatus::Invited;
            OrganisationMemberResponse {
                user_id: member.user_id.to_hex(),
                email: user.map(|u| u.email.clone()),
                first_name: if expose_name {
                    user.map(|u| u.first_name.clone())
                } else {
                    None
                },
                last_name: if expose_name {
                    user.map(|u| u.last_name.clone())
                } else {
                    None
                },
                role: member.role,
                status: member.status,
                joined_at: bson_dt_to_chrono(member.joined_at),
            }
        })
        .collect();

    OrganisationResponse {
        id: org.id.to_hex(),
        name: org.name,
        members,
        member_count,
        active_member_count,
        created_at: bson_dt_to_chrono(org.created_at),
        updated_at: bson_dt_to_chrono(org.updated_at),
    }
}
