use std::sync::Arc;

use actix_cors::Cors;
use actix_web::{http::header, web};

use crate::{
    common::errors::AppError,
    config::Settings,
    database::{mongodb::MongoDb, seed::seed_system_rbac},
    middleware::PermissionCache,
    repository::{
        auth::refresh_token_repository::{MongoRefreshTokenRepository, RefreshTokenRepository},
        permission::permission_repository::{MongoPermissionRepository, PermissionRepository},
        role::role_repository::{MongoRoleRepository, RoleRepository},
        user::user_repository::{MongoUserRepository, UserRepository},
    },
    routes::configure_routes,
    security::{jwt::JwtService, keys::JwtKeyPair},
    services::{
        auth::AuthService, dashboard::DashboardService, permissions::PermissionService,
        profile::ProfileService, roles::RoleService, users::UserService,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub mongodb: Option<MongoDb>,
    pub jwt_service: Arc<JwtService>,
    pub permission_cache: PermissionCache,
    pub user_repo: Arc<dyn UserRepository>,
    pub role_repo: Arc<dyn RoleRepository>,
    pub permission_repo: Arc<dyn PermissionRepository>,
    pub refresh_token_repo: Arc<dyn RefreshTokenRepository>,
    pub auth_service: AuthService,
    pub user_service: UserService,
    pub role_service: RoleService,
    pub permission_service: PermissionService,
    pub profile_service: ProfileService,
    pub dashboard_service: DashboardService,
}

impl AppState {
    pub fn from_repositories(
        settings: Arc<Settings>,
        mongodb: Option<MongoDb>,
        jwt_service: Arc<JwtService>,
        user_repo: Arc<dyn UserRepository>,
        role_repo: Arc<dyn RoleRepository>,
        permission_repo: Arc<dyn PermissionRepository>,
        refresh_token_repo: Arc<dyn RefreshTokenRepository>,
    ) -> Self {
        let permission_cache = PermissionCache::new(settings.app.permission_cache_ttl_seconds);

        let auth_service = AuthService::new(
            Arc::clone(&settings),
            Arc::clone(&jwt_service),
            Arc::clone(&user_repo),
            Arc::clone(&role_repo),
            Arc::clone(&permission_repo),
            Arc::clone(&refresh_token_repo),
            permission_cache.clone(),
        );

        let user_service = UserService::new(
            Arc::clone(&user_repo),
            Arc::clone(&role_repo),
            Arc::clone(&permission_repo),
            Arc::clone(&refresh_token_repo),
            permission_cache.clone(),
        );

        let role_service = RoleService::new(
            Arc::clone(&role_repo),
            Arc::clone(&permission_repo),
            Arc::clone(&user_repo),
            permission_cache.clone(),
        );

        let permission_service = PermissionService::new(
            Arc::clone(&permission_repo),
            Arc::clone(&role_repo),
            permission_cache.clone(),
        );

        let profile_service = ProfileService::new(
            Arc::clone(&user_repo),
            Arc::clone(&role_repo),
            Arc::clone(&permission_repo),
            Arc::clone(&refresh_token_repo),
            permission_cache.clone(),
        );

        let dashboard_service = DashboardService::new(
            Arc::clone(&user_repo),
            Arc::clone(&role_repo),
            Arc::clone(&permission_repo),
        );

        Self {
            settings,
            mongodb,
            jwt_service,
            permission_cache,
            user_repo,
            role_repo,
            permission_repo,
            refresh_token_repo,
            auth_service,
            user_service,
            role_service,
            permission_service,
            profile_service,
            dashboard_service,
        }
    }

    pub async fn from_mongodb(settings: Settings) -> Result<Self, AppError> {
        let settings = Arc::new(settings);
        let mongo = MongoDb::connect(&settings.mongodb).await?;
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

        seed_system_rbac(
            &permission_repo,
            &role_repo,
            &user_repo,
            &settings.bootstrap,
        )
        .await?;

        Ok(Self::from_repositories(
            settings,
            Some(mongo),
            jwt_service,
            user_repo,
            role_repo,
            permission_repo,
            refresh_token_repo,
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
    let rate_limit = state.settings.rate_limit.clone();

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
        .configure(|sc| configure_routes(sc, &rate_limit));
}
