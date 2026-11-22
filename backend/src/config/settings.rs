use std::env;

use config::{Config, File};
use serde::Deserialize;

use crate::common::errors::AppError;

#[derive(Debug, Clone, Deserialize)]
pub struct AppSettings {
    pub environment: String,
    pub require_email_verification: bool,
    pub permission_cache_ttl_seconds: u64,
    #[serde(default)]
    pub enable_swagger_ui: Option<bool>,
    #[serde(default = "default_audit_log_retention_days")]
    pub audit_log_retention_days: u64,
}

fn default_audit_log_retention_days() -> u64 {
    90
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
    pub frontend_url: String,
    pub allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MongoDbSettings {
    pub uri: String,
    pub database: String,
    pub max_pool_size: u32,
    pub min_pool_size: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwtSettings {
    pub issuer: String,
    pub audience: String,
    pub access_token_ttl: i64,
    pub refresh_token_ttl: i64,
    pub private_key_path: String,
    pub public_key_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CookieSettings {
    pub refresh_cookie_name: String,
    pub csrf_cookie_name: String,
    pub secure: bool,
    pub same_site: String,
    pub domain: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitSettings {
    pub enabled: bool,
    pub auth_requests_per_minute: u32,
    pub api_requests_per_minute: u32,
    #[serde(default)]
    pub trust_proxy_headers: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BootstrapSettings {
    pub super_admin_email: String,
    pub super_admin_password: String,
    pub super_admin_first_name: String,
    pub super_admin_last_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub app: AppSettings,
    pub server: ServerSettings,
    pub mongodb: MongoDbSettings,
    pub jwt: JwtSettings,
    pub cookie: CookieSettings,
    pub rate_limit: RateLimitSettings,
    pub bootstrap: BootstrapSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            app: AppSettings {
                environment: "development".to_string(),
                require_email_verification: false,
                permission_cache_ttl_seconds: 30,
                enable_swagger_ui: None,
                audit_log_retention_days: default_audit_log_retention_days(),
            },
            server: ServerSettings {
                host: "127.0.0.1".to_string(),
                port: 8080,
                frontend_url: "http://localhost:5173".to_string(),
                allowed_origins: vec!["http://localhost:5173".to_string()],
            },
            mongodb: MongoDbSettings {
                uri: "mongodb://localhost:27017".to_string(),
                database: "user_management".to_string(),
                max_pool_size: 50,
                min_pool_size: 5,
            },
            jwt: JwtSettings {
                issuer: "user-management-api".to_string(),
                audience: "user-management-client".to_string(),
                access_token_ttl: 900,
                refresh_token_ttl: 604_800,
                private_key_path: String::new(),
                public_key_path: String::new(),
            },
            cookie: CookieSettings {
                refresh_cookie_name: "refresh_token".to_string(),
                csrf_cookie_name: "csrf_token".to_string(),
                secure: false,
                same_site: "Lax".to_string(),
                domain: String::new(),
            },
            rate_limit: RateLimitSettings {
                enabled: true,
                auth_requests_per_minute: 30,
                api_requests_per_minute: 300,
                trust_proxy_headers: false,
            },
            bootstrap: BootstrapSettings {
                super_admin_email: "admin@example.com".to_string(),
                super_admin_password: String::new(),
                super_admin_first_name: "System".to_string(),
                super_admin_last_name: "Administrator".to_string(),
            },
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self, AppError> {
        let _ = dotenvy::dotenv();

        let builder = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name("config/local").required(false));

        let has_config_files = std::path::Path::new("config/default.toml").exists()
            || std::path::Path::new("config/local.toml").exists();

        let mut settings: Self = if has_config_files {
            let built = builder
                .build()
                .map_err(|e| AppError::Internal(format!("Failed to load configuration: {e}")))?;
            built.try_deserialize().map_err(|e| {
                AppError::Internal(format!("Failed to deserialize configuration: {e}"))
            })?
        } else {
            Self::default()
        };

        apply_env_overrides(&mut settings);
        settings.validate_security()?;
        Ok(settings)
    }

    pub fn is_production(&self) -> bool {
        self.app.environment.eq_ignore_ascii_case("production")
    }

    pub fn is_swagger_enabled(&self) -> bool {
        self.app.enable_swagger_ui.unwrap_or(!self.is_production())
    }

    pub fn validate_security(&self) -> Result<(), AppError> {
        if self.mongodb.database.trim().is_empty() {
            return Err(AppError::Internal(
                "MONGODB_DATABASE must not be empty".to_string(),
            ));
        }

        if self.jwt.access_token_ttl <= 0 || self.jwt.refresh_token_ttl <= 0 {
            return Err(AppError::Internal(
                "JWT token TTL values must be positive".to_string(),
            ));
        }

        if self.server.allowed_origins.iter().any(|o| o.trim() == "*") {
            return Err(AppError::Internal(
                "Wildcard CORS origin '*' is forbidden with credentialed requests".to_string(),
            ));
        }

        if self.is_production() {
            if self.jwt.private_key_path.trim().is_empty()
                || self.jwt.public_key_path.trim().is_empty()
            {
                return Err(AppError::Internal(
                    "JWT_PRIVATE_KEY_PATH and JWT_PUBLIC_KEY_PATH must be configured in production"
                        .to_string(),
                ));
            }
            if !self.cookie.secure {
                return Err(AppError::Internal(
                    "COOKIE_SECURE must be true in production".to_string(),
                ));
            }
        }

        Ok(())
    }
}

fn apply_env_overrides(settings: &mut Settings) {
    if let Ok(val) = env::var("APP_ENV") {
        settings.app.environment = val;
    }
    if let Ok(val) = env::var("REQUIRE_EMAIL_VERIFICATION")
        && let Ok(parsed) = val.parse::<bool>()
    {
        settings.app.require_email_verification = parsed;
    }
    if let Ok(val) = env::var("PERMISSION_CACHE_TTL_SECONDS")
        && let Ok(parsed) = val.parse::<u64>()
    {
        settings.app.permission_cache_ttl_seconds = parsed;
    }
    if let Ok(val) = env::var("ENABLE_SWAGGER_UI")
        && let Ok(parsed) = val.parse::<bool>()
    {
        settings.app.enable_swagger_ui = Some(parsed);
    }
    if let Ok(val) = env::var("AUDIT_LOG_RETENTION_DAYS")
        && let Ok(parsed) = val.parse::<u64>()
    {
        settings.app.audit_log_retention_days = parsed;
    }

    if let Ok(val) = env::var("SERVER_HOST") {
        settings.server.host = val;
    }
    if let Ok(val) = env::var("SERVER_PORT")
        && let Ok(parsed) = val.parse::<u16>()
    {
        settings.server.port = parsed;
    }
    if let Ok(val) = env::var("FRONTEND_URL") {
        settings.server.frontend_url = val.clone();
        if !settings.server.allowed_origins.contains(&val) {
            settings.server.allowed_origins.push(val);
        }
    }
    if let Ok(val) = env::var("ALLOWED_ORIGINS") {
        let origins: Vec<String> = val
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !origins.is_empty() {
            settings.server.allowed_origins = origins;
        }
    }

    if let Ok(val) = env::var("MONGODB_URI") {
        settings.mongodb.uri = val;
    }
    if let Ok(val) = env::var("MONGODB_DATABASE") {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            settings.mongodb.database = trimmed.to_string();
        }
    }

    if let Ok(val) = env::var("JWT_ISSUER") {
        settings.jwt.issuer = val;
    }
    if let Ok(val) = env::var("JWT_AUDIENCE") {
        settings.jwt.audience = val;
    }
    if let Ok(val) = env::var("JWT_ACCESS_TOKEN_TTL")
        && let Ok(parsed) = val.parse::<i64>()
    {
        settings.jwt.access_token_ttl = parsed;
    }
    if let Ok(val) = env::var("JWT_REFRESH_TOKEN_TTL")
        && let Ok(parsed) = val.parse::<i64>()
    {
        settings.jwt.refresh_token_ttl = parsed;
    }
    if let Ok(val) = env::var("JWT_PRIVATE_KEY_PATH") {
        settings.jwt.private_key_path = val;
    }
    if let Ok(val) = env::var("JWT_PUBLIC_KEY_PATH") {
        settings.jwt.public_key_path = val;
    }

    if let Ok(val) = env::var("COOKIE_SECURE")
        && let Ok(parsed) = val.parse::<bool>()
    {
        settings.cookie.secure = parsed;
    }
    if let Ok(val) = env::var("COOKIE_SAME_SITE") {
        settings.cookie.same_site = val;
    }
    if let Ok(val) = env::var("COOKIE_DOMAIN") {
        settings.cookie.domain = val;
    }

    if let Ok(val) = env::var("RATE_LIMIT_ENABLED")
        && let Ok(parsed) = val.parse::<bool>()
    {
        settings.rate_limit.enabled = parsed;
    }
    if let Ok(val) = env::var("RATE_LIMIT_AUTH_PER_MINUTE")
        && let Ok(parsed) = val.parse::<u32>()
    {
        settings.rate_limit.auth_requests_per_minute = parsed;
    }
    if let Ok(val) = env::var("RATE_LIMIT_API_PER_MINUTE")
        && let Ok(parsed) = val.parse::<u32>()
    {
        settings.rate_limit.api_requests_per_minute = parsed;
    }
    if let Ok(val) = env::var("TRUST_PROXY_HEADERS")
        && let Ok(parsed) = val.parse::<bool>()
    {
        settings.rate_limit.trust_proxy_headers = parsed;
    }

    if let Ok(val) = env::var("BOOTSTRAP_SUPER_ADMIN_EMAIL") {
        settings.bootstrap.super_admin_email = val;
    }
    if let Ok(val) = env::var("BOOTSTRAP_SUPER_ADMIN_PASSWORD") {
        settings.bootstrap.super_admin_password = val;
    }
    if let Ok(val) = env::var("BOOTSTRAP_SUPER_ADMIN_FIRST_NAME") {
        settings.bootstrap.super_admin_first_name = val;
    }
    if let Ok(val) = env::var("BOOTSTRAP_SUPER_ADMIN_LAST_NAME") {
        settings.bootstrap.super_admin_last_name = val;
    }
}
