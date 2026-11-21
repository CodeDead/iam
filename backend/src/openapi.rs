use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};

use crate::{
    common::{
        pagination::{PaginatedResponse, SortOrder},
        responses::{ApiResponse, EmptyData, ErrorBody, ErrorResponse},
    },
    repository::user::dao::user::{AccountStatus, UserProfileInfo},
    web::{
        controller::{
            auth as auth_ctrl, dashboard as dashboard_ctrl, health, permissions as perm_ctrl,
            profile as profile_ctrl, roles as role_ctrl, users as user_ctrl,
        },
        dto::{
            auth::{AuthTokenResponse, LoginRequest, MeResponse, RegisterRequest},
            dashboard::{DashboardStatsResponse, UserStats},
            health::HealthStatus,
            permissions::{CreatePermissionRequest, PermissionResponse, UpdatePermissionRequest},
            profile::{ChangePasswordRequest, UpdateProfileRequest},
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
    ),
    components(
        schemas(
            HealthStatus,
            ErrorBody,
            ErrorResponse,
            EmptyData,
            SortOrder,
            AccountStatus,
            UserProfileInfo,
            UserProfileUpdateDto,
            RegisterRequest,
            LoginRequest,
            AuthTokenResponse,
            MeResponse,
            DashboardStatsResponse,
            UserStats,
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
        )
    ),
    tags(
        (name = "Health", description = "Liveness and readiness probes"),
        (name = "Authentication", description = "Registration, login, refresh rotation, logout, and session identity"),
        (name = "Dashboard", description = "Permission-filtered administrative metrics"),
        (name = "Users", description = "User lifecycle, status, role assignment, and effective permissions"),
        (name = "Roles", description = "RBAC role lifecycle and permission assignment"),
        (name = "Permissions", description = "Granular resource.action permission management"),
        (name = "Profile", description = "Authenticated user self-service profile and password management")
    )
)]
pub struct ApiDoc;
