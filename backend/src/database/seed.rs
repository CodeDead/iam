use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use tracing::info;

use crate::{
    common::{errors::AppError, validation::normalize_email},
    config::settings::BootstrapSettings,
    repository::{
        permission::{dao::permission::Permission, permission_repository::PermissionRepository},
        role::{dao::role::Role, role_repository::RoleRepository},
        user::{
            dao::user::{AccountStatus, User, UserProfileInfo},
            user_repository::UserRepository,
        },
    },
    security::password::hash_password,
};

pub const ROLE_SUPER_ADMIN: &str = "Super Administrator";
pub const ROLE_ADMIN: &str = "Administrator";
pub const ROLE_MODERATOR: &str = "Moderator";
pub const ROLE_USER: &str = "User";

pub const SYSTEM_PERMISSIONS: &[(&str, &str, &str)] = &[
    (
        "users",
        "read",
        "View user accounts, roles, and effective permissions",
    ),
    ("users", "create", "Create new user accounts"),
    ("users", "update", "Update user accounts and account status"),
    ("users", "delete", "Delete user accounts"),
    (
        "users",
        "assign_roles",
        "Assign or remove roles for user accounts",
    ),
    (
        "users",
        "manage_self_roles",
        "Modify one's own assigned roles (restricted capability)",
    ),
    ("roles", "read", "View roles and their assigned permissions"),
    ("roles", "create", "Create custom roles"),
    (
        "roles",
        "update",
        "Update roles and manage role permissions",
    ),
    ("roles", "delete", "Delete non-system roles"),
    ("permissions", "read", "View available permissions"),
    ("permissions", "create", "Create custom permissions"),
    ("permissions", "update", "Update non-system permissions"),
    ("permissions", "delete", "Delete non-system permissions"),
    ("profile", "read", "View own user profile"),
    ("profile", "update", "Update own user profile and password"),
    ("profile", "delete", "Delete own user account"),
    (
        "audit_logs",
        "read",
        "View system audit logs and activity history",
    ),
];

pub async fn seed_system_rbac(
    permission_repo: &Arc<dyn PermissionRepository>,
    role_repo: &Arc<dyn RoleRepository>,
    user_repo: &Arc<dyn UserRepository>,
    bootstrap: &BootstrapSettings,
) -> Result<(), AppError> {
    let now = BsonDateTime::now();
    let mut perm_map: HashMap<String, ObjectId> = HashMap::new();
    let mut newly_created_perms: HashSet<ObjectId> = HashSet::new();

    for &(resource, action, description) in SYSTEM_PERMISSIONS {
        let name = format!("{resource}.{action}");
        if let Some(existing) = permission_repo.find_by_name(&name).await? {
            perm_map.insert(name, existing.id);
        } else {
            let perm = Permission {
                id: ObjectId::new(),
                name: name.clone(),
                description: description.to_string(),
                resource: resource.to_string(),
                action: action.to_string(),
                is_system: true,
                created_at: now,
                updated_at: now,
            };
            let created = permission_repo.create(&perm).await?;
            newly_created_perms.insert(created.id);
            perm_map.insert(name, created.id);
        }
    }

    let all_perm_ids: Vec<ObjectId> = perm_map.values().copied().collect();
    let collect_perms = |names: &[&str]| -> Vec<ObjectId> {
        names
            .iter()
            .filter_map(|n| perm_map.get(*n).copied())
            .collect()
    };

    let admin_perms = collect_perms(&[
        "users.read",
        "users.create",
        "users.update",
        "users.delete",
        "users.assign_roles",
        "roles.read",
        "roles.create",
        "roles.update",
        "roles.delete",
        "permissions.read",
        "audit_logs.read",
        "profile.read",
        "profile.update",
        "profile.delete",
    ]);

    let moderator_perms = collect_perms(&[
        "users.read",
        "users.update",
        "roles.read",
        "permissions.read",
        "profile.read",
        "profile.update",
        "profile.delete",
    ]);

    let user_perms = collect_perms(&["profile.read", "profile.update", "profile.delete"]);

    let role_defs = [
        (
            ROLE_SUPER_ADMIN,
            "Full unrestricted administrative access across the entire platform",
            all_perm_ids,
        ),
        (
            ROLE_ADMIN,
            "Administrative management of users and roles",
            admin_perms,
        ),
        (
            ROLE_MODERATOR,
            "Read access to users, roles, and permissions with limited account moderation",
            moderator_perms,
        ),
        (
            ROLE_USER,
            "Standard authenticated user with self-service profile access",
            user_perms,
        ),
    ];

    let mut super_admin_role_id: Option<ObjectId> = None;

    for (name, description, permissions) in role_defs {
        let role_id = if let Some(mut existing) = role_repo.find_by_name(name).await? {
            if !newly_created_perms.is_empty() {
                let mut added = false;
                for perm_id in &permissions {
                    if newly_created_perms.contains(perm_id)
                        && !existing.permissions.contains(perm_id)
                    {
                        existing.permissions.push(*perm_id);
                        added = true;
                    }
                }
                if added {
                    existing.updated_at = now;
                    role_repo.update(&existing).await?;
                }
            }
            existing.id
        } else {
            let role = Role {
                id: ObjectId::new(),
                name: name.to_string(),
                description: description.to_string(),
                permissions,
                is_system: true,
                created_at: now,
                updated_at: now,
            };
            let created = role_repo.create(&role).await?;
            created.id
        };

        if name == ROLE_SUPER_ADMIN {
            super_admin_role_id = Some(role_id);
        }
    }

    // Optional bootstrap Super Administrator creation if password is provided and no active super admin exists.
    if let Some(sa_role_id) = super_admin_role_id
        && !bootstrap.super_admin_password.trim().is_empty()
        && !bootstrap.super_admin_email.trim().is_empty()
    {
        let existing_super_admins = user_repo.count_active_users_with_role(sa_role_id).await?;
        if existing_super_admins == 0 {
            let email = normalize_email(&bootstrap.super_admin_email);
            if user_repo.find_by_email(&email).await?.is_none() {
                let password_hash = hash_password(bootstrap.super_admin_password.clone()).await?;
                let admin_user = User {
                    id: ObjectId::new(),
                    first_name: bootstrap.super_admin_first_name.clone(),
                    last_name: bootstrap.super_admin_last_name.clone(),
                    email: email.clone(),
                    password_hash,
                    status: AccountStatus::Active,
                    email_verified: true,
                    roles: vec![sa_role_id],
                    token_version: 1,
                    profile: UserProfileInfo::default(),
                    created_at: now,
                    updated_at: now,
                    last_login_at: None,
                };
                user_repo.create(&admin_user).await?;
                info!(email = %email, "Bootstrapped initial Super Administrator account");
            }
        }
    }

    Ok(())
}
