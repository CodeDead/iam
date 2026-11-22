use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};

use crate::{
    common::{
        pagination::{PaginatedResponse, SortOrder},
        responses::{ApiResponse, EmptyData, ErrorBody, ErrorResponse},
    },
    repository::{
        organisation::dao::organisation::{OrganisationMemberRole, OrganisationMemberStatus},
        user::dao::user::{AccountStatus, UserProfileInfo},
    },
    web::{
        controller::{
            audit as audit_ctrl, auth as auth_ctrl, dashboard as dashboard_ctrl, health,
            organisations as org_ctrl, permissions as perm_ctrl, profile as profile_ctrl,
            roles as role_ctrl, users as user_ctrl,
        },
        dto::{
            audit::AuditLogResponse,
            auth::{AuthTokenResponse, LoginRequest, MeResponse, RegisterRequest},
            dashboard::{
                AuditStats, DailyActivityPoint, DashboardStatsResponse, OrganisationSizeItem,
                OrganisationStats, PermissionStats, RecentAuditEvent, ResourceCountItem,
                RoleDistributionItem, RoleStats, UserStats,
            },
            health::HealthStatus,
            organisations::{
                AddOrganisationMemberRequest, CreateOrganisationRequest,
                OrganisationMemberResponse, OrganisationResponse, UpdateOrganisationMemberRequest,
                UpdateOrganisationRequest, UserOrganisationSummary,
            },
            permissions::{CreatePermissionRequest, PermissionResponse, UpdatePermissionRequest},
            profile::{ChangePasswordRequest, DeleteAccountRequest, UpdateProfileRequest},
            roles::{
                CreateRoleRequest, RoleResponse, RoleSummary, UpdateRolePermissionsRequest,
                UpdateRoleRequest,
            },
            users::{
                CreateUserRequest, UpdateUserRequest, UpdateUserRolesRequest,
                UpdateUserStatusRequest, UserProfileUpdateDto, UserResponse,
            },
        },
    },
};

struct BearerSecurityAddon;

impl Modify for BearerSecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some("EdDSA signed short-lived access JWT"))
                        .build(),
                ),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "CodeDead IAM — Secure User, Role & Permission Management API",
        version = "1.0.0",
        description = "Production-oriented REST API for identity, RBAC roles, granular permissions, and secure JWT/refresh-token session management."
    ),
    modifiers(&BearerSecurityAddon),
    paths(
        health::liveness,
        health::readiness,
        auth_ctrl::register,
        auth_ctrl::login,
        auth_ctrl::refresh,
        auth_ctrl::logout,
        auth_ctrl::me,
        dashboard_ctrl::get_dashboard_stats,
        user_ctrl::list_users,
        user_ctrl::create_user,
        user_ctrl::get_user,
        user_ctrl::update_user,
        user_ctrl::update_user_status,
        user_ctrl::delete_user,
        user_ctrl::get_user_roles,
        user_ctrl::update_user_roles,
        user_ctrl::get_user_permissions,
        user_ctrl::get_user_organisations,
        role_ctrl::list_roles,
        role_ctrl::create_role,
        role_ctrl::get_role,
        role_ctrl::update_role,
        role_ctrl::delete_role,
        role_ctrl::get_role_permissions,
        role_ctrl::update_role_permissions,
        role_ctrl::get_role_users,
        perm_ctrl::list_permissions,
        perm_ctrl::create_permission,
        perm_ctrl::get_permission,
        perm_ctrl::update_permission,
        perm_ctrl::delete_permission,
        perm_ctrl::get_permission_roles,
        profile_ctrl::get_profile,
        profile_ctrl::update_profile,
        profile_ctrl::change_password,
        profile_ctrl::delete_account,
        audit_ctrl::list_audit_logs,
        audit_ctrl::get_audit_log,
        org_ctrl::list_my_organisations,
        org_ctrl::list_organisations,
        org_ctrl::create_organisation,
        org_ctrl::get_organisation,
        org_ctrl::update_organisation,
        org_ctrl::delete_organisation,
        org_ctrl::add_organisation_member,
        org_ctrl::update_organisation_member,
        org_ctrl::remove_organisation_member,
        org_ctrl::accept_organisation_invitation,
        org_ctrl::leave_organisation,
    ),
    components(
        schemas(
            HealthStatus,
            ErrorBody,
            ErrorResponse,
            EmptyData,
            SortOrder,
            AccountStatus,
            OrganisationMemberRole,
            OrganisationMemberStatus,
            UserProfileInfo,
            UserProfileUpdateDto,
            RegisterRequest,
            LoginRequest,
            AuthTokenResponse,
            MeResponse,
            DashboardStatsResponse,
            UserStats,
            RoleStats,
            RoleDistributionItem,
            PermissionStats,
            ResourceCountItem,
            OrganisationStats,
            OrganisationSizeItem,
            AuditStats,
            DailyActivityPoint,
            RecentAuditEvent,
            UserResponse,
            CreateUserRequest,
            UpdateUserRequest,
            UpdateUserStatusRequest,
            UpdateUserRolesRequest,
            RoleSummary,
            RoleResponse,
            CreateRoleRequest,
            UpdateRoleRequest,
            UpdateRolePermissionsRequest,
            PermissionResponse,
            CreatePermissionRequest,
            UpdatePermissionRequest,
            UpdateProfileRequest,
            ChangePasswordRequest,
            DeleteAccountRequest,
            AuditLogResponse,
            OrganisationResponse,
            OrganisationMemberResponse,
            UserOrganisationSummary,
            CreateOrganisationRequest,
            UpdateOrganisationRequest,
            AddOrganisationMemberRequest,
            UpdateOrganisationMemberRequest,
            ApiResponse<HealthStatus>,
            ApiResponse<EmptyData>,
            ApiResponse<AuthTokenResponse>,
            ApiResponse<MeResponse>,
            ApiResponse<DashboardStatsResponse>,
            ApiResponse<UserResponse>,
            ApiResponse<PaginatedResponse<UserResponse>>,
            ApiResponse<RoleResponse>,
            ApiResponse<PaginatedResponse<RoleResponse>>,
            ApiResponse<Vec<RoleSummary>>,
            ApiResponse<PermissionResponse>,
            ApiResponse<PaginatedResponse<PermissionResponse>>,
            ApiResponse<Vec<PermissionResponse>>,
            ApiResponse<Vec<UserResponse>>,
            ApiResponse<AuditLogResponse>,
            ApiResponse<PaginatedResponse<AuditLogResponse>>,
            ApiResponse<OrganisationResponse>,
            ApiResponse<PaginatedResponse<OrganisationResponse>>,
            ApiResponse<Vec<UserOrganisationSummary>>,
        )
    ),
    tags(
        (name = "Health", description = "Liveness and readiness probes"),
        (name = "Authentication", description = "Registration, login, refresh rotation, logout, and session identity"),
        (name = "Dashboard", description = "Permission-filtered administrative metrics"),
        (name = "Users", description = "User lifecycle, status, role assignment, and effective permissions"),
        (name = "Roles", description = "RBAC role lifecycle and permission assignment"),
        (name = "Permissions", description = "Granular resource.action permission management"),
        (name = "Organisations", description = "Organisation accounts and membership management"),
        (name = "Profile", description = "Authenticated user self-service profile and password management"),
        (name = "Audit Logs", description = "Immutable security and administration activity logs")
    )
)]
pub struct ApiDoc;
