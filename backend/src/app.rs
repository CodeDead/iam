use std::sync::Arc;

use actix_cors::Cors;
use actix_web::{http::header, web};

use crate::{
    common::errors::AppError,
    config::Settings,
    database::{mongodb::MongoDb, seed::seed_system_rbac},
    middleware::{PermissionCache, RateLimitMiddleware},
    repository::{
        audit::audit_log_repository::{AuditLogRepository, MongoAuditLogRepository},
        auth::refresh_token_repository::{MongoRefreshTokenRepository, RefreshTokenRepository},
        organisation::organisation_repository::{
            MongoOrganisationRepository, OrganisationRepository,
        },
        permission::permission_repository::{MongoPermissionRepository, PermissionRepository},
        role::role_repository::{MongoRoleRepository, RoleRepository},
        user::user_repository::{MongoUserRepository, UserRepository},
    },
    routes::configure_routes,
    security::{jwt::JwtService, keys::JwtKeyPair},
    services::{
        audit::AuditService, auth::AuthService, dashboard::DashboardService,
        organisations::OrganisationService, permissions::PermissionService,
        profile::ProfileService, roles::RoleService, users::UserService,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub mongodb: Option<MongoDb>,
    pub jwt_service: Arc<JwtService>,
    pub permission_cache: PermissionCache,
    pub auth_limiter: RateLimitMiddleware,
    pub api_limiter: RateLimitMiddleware,
    pub user_repo: Arc<dyn UserRepository>,
    pub role_repo: Arc<dyn RoleRepository>,
    pub permission_repo: Arc<dyn PermissionRepository>,
    pub refresh_token_repo: Arc<dyn RefreshTokenRepository>,
    pub organisation_repo: Arc<dyn OrganisationRepository>,
    pub audit_repo: Arc<dyn AuditLogRepository>,
    pub audit_service: AuditService,
    pub auth_service: AuthService,
    pub user_service: UserService,
    pub role_service: RoleService,
    pub permission_service: PermissionService,
    pub organisation_service: OrganisationService,
    pub profile_service: ProfileService,
    pub dashboard_service: DashboardService,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn from_repositories(
        settings: Arc<Settings>,
        mongodb: Option<MongoDb>,
        jwt_service: Arc<JwtService>,
        user_repo: Arc<dyn UserRepository>,
        role_repo: Arc<dyn RoleRepository>,
        permission_repo: Arc<dyn PermissionRepository>,
        refresh_token_repo: Arc<dyn RefreshTokenRepository>,
        organisation_repo: Arc<dyn OrganisationRepository>,
        audit_repo: Arc<dyn AuditLogRepository>,
    ) -> Self {
        let permission_cache = PermissionCache::new(settings.app.permission_cache_ttl_seconds);
        let auth_limiter = RateLimitMiddleware::per_minute(
            settings.rate_limit.enabled,
            settings.rate_limit.auth_requests_per_minute,
            settings.rate_limit.trust_proxy_headers,
        );
        let api_limiter = RateLimitMiddleware::per_minute(
            settings.rate_limit.enabled,
            settings.rate_limit.api_requests_per_minute,
            settings.rate_limit.trust_proxy_headers,
        );
        let audit_service = AuditService::new(Arc::clone(&audit_repo));

        let auth_service = AuthService::new(
            Arc::clone(&settings),
            Arc::clone(&jwt_service),
            Arc::clone(&user_repo),
            Arc::clone(&role_repo),
            Arc::clone(&permission_repo),
            Arc::clone(&refresh_token_repo),
            Arc::clone(&organisation_repo),
            permission_cache.clone(),
            audit_service.clone(),
        );

        let user_service = UserService::new(
            Arc::clone(&user_repo),
            Arc::clone(&role_repo),
            Arc::clone(&permission_repo),
            Arc::clone(&refresh_token_repo),
            Arc::clone(&organisation_repo),
            permission_cache.clone(),
            audit_service.clone(),
        );

        let role_service = RoleService::new(
            Arc::clone(&role_repo),
            Arc::clone(&permission_repo),
            Arc::clone(&user_repo),
            permission_cache.clone(),
            audit_service.clone(),
        );

        let permission_service = PermissionService::new(
            Arc::clone(&permission_repo),
            Arc::clone(&role_repo),
            permission_cache.clone(),
            audit_service.clone(),
        );

        let organisation_service = OrganisationService::new(
            Arc::clone(&organisation_repo),
            Arc::clone(&user_repo),
            audit_service.clone(),
        );

        let profile_service = ProfileService::new(
            Arc::clone(&user_repo),
            Arc::clone(&role_repo),
            Arc::clone(&permission_repo),
            Arc::clone(&refresh_token_repo),
            Arc::clone(&organisation_repo),
            permission_cache.clone(),
            audit_service.clone(),
        );

        let dashboard_service = DashboardService::new(
            Arc::clone(&user_repo),
            Arc::clone(&role_repo),
            Arc::clone(&permission_repo),
            Arc::clone(&refresh_token_repo),
            Arc::clone(&organisation_repo),
            Arc::clone(&audit_repo),
        );

        Self {
            settings,
            mongodb,
            jwt_service,
            permission_cache,
            auth_limiter,
            api_limiter,
            user_repo,
            role_repo,
            permission_repo,
            refresh_token_repo,
            organisation_repo,
            audit_repo,
            audit_service,
            auth_service,
            user_service,
            role_service,
            permission_service,
            organisation_service,
            profile_service,
            dashboard_service,
        }
    }

    pub async fn from_mongodb(settings: Settings) -> Result<Self, AppError> {
        let settings = Arc::new(settings);
        let mongo = MongoDb::connect(
            &settings.mongodb,
            settings.app.audit_log_retention_days,
            settings.jwt.revoked_token_retention_seconds,
        )
        .await?;
        let keys = JwtKeyPair::from_settings(&settings)?;
        let jwt_service = Arc::new(JwtService::new(keys, &settings.jwt));

        let user_repo: Arc<dyn UserRepository> =
            Arc::new(MongoUserRepository::new(&mongo.database));
        let role_repo: Arc<dyn RoleRepository> =
            Arc::new(MongoRoleRepository::new(&mongo.database));
        let permission_repo: Arc<dyn PermissionRepository> =
            Arc::new(MongoPermissionRepository::new(&mongo.database));
        let refresh_token_repo: Arc<dyn RefreshTokenRepository> =
            Arc::new(MongoRefreshTokenRepository::new(
                mongo.client.clone(),
                &mongo.database,
                mongo.supports_transactions,
            ));
        let organisation_repo: Arc<dyn OrganisationRepository> =
            Arc::new(MongoOrganisationRepository::new(&mongo.database));
        let audit_repo: Arc<dyn AuditLogRepository> =
            Arc::new(MongoAuditLogRepository::new(&mongo.database));

        seed_system_rbac(
            &permission_repo,
            &role_repo,
            &user_repo,
            &settings.bootstrap,
        )
        .await?;

        let backfilled_owners = organisation_repo.backfill_missing_owners().await?;
        if backfilled_owners > 0 {
            tracing::info!(
                count = backfilled_owners,
                "Backfilled owner role for legacy organisations"
            );
        }

        Ok(Self::from_repositories(
            settings,
            Some(mongo),
            jwt_service,
            user_repo,
            role_repo,
            permission_repo,
            refresh_token_repo,
            organisation_repo,
            audit_repo,
        ))
    }
}

pub fn build_cors(settings: &Settings) -> Cors {
    let mut cors = Cors::default()
        .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
        .allowed_headers(vec![
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::HeaderName::from_static("x-csrf-token"),
            header::HeaderName::from_static("x-request-id"),
        ])
        .expose_headers(vec![header::HeaderName::from_static("x-request-id")])
        .supports_credentials()
        .max_age(3600);

    for origin in &settings.server.allowed_origins {
        cors = cors.allowed_origin(origin);
    }

    cors
}

pub fn configure_app_services(cfg: &mut web::ServiceConfig, state: web::Data<AppState>) {
    let settings = Arc::clone(&state.settings);
    let auth_limiter = state.auth_limiter.with_config(
        settings.rate_limit.enabled,
        settings.rate_limit.auth_requests_per_minute,
        settings.rate_limit.trust_proxy_headers,
    );
    let api_limiter = state.api_limiter.with_config(
        settings.rate_limit.enabled,
        settings.rate_limit.api_requests_per_minute,
        settings.rate_limit.trust_proxy_headers,
    );

    cfg.app_data(state)
        .app_data(
            web::JsonConfig::default()
                .limit(1_048_576)
                .error_handler(|err, _req| {
                    let message = format!("Invalid JSON payload: {err}");
                    AppError::bad_request("INVALID_JSON_PAYLOAD", message).into()
                }),
        )
        .app_data(web::QueryConfig::default().error_handler(|err, _req| {
            let message = format!("Invalid query parameters: {err}");
            AppError::bad_request("INVALID_QUERY_PARAMETERS", message).into()
        }))
        .configure(|sc| configure_routes(sc, &settings, auth_limiter, api_limiter));
}
