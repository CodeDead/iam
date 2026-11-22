use std::sync::Arc;

use chrono::{Duration, Utc};
use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use serde_json::json;
use uuid::Uuid;

use crate::{
    common::{
        errors::AppError,
        validation::{normalize_email, validate_dto},
    },
    config::Settings,
    database::{mongodb::chrono_to_bson_dt, seed::ROLE_USER},
    middleware::{AuthenticatedUser, PermissionCache},
    repository::{
        auth::{
            dao::refresh_token::RefreshToken, refresh_token_repository::RefreshTokenRepository,
        },
        permission::permission_repository::PermissionRepository,
        role::role_repository::RoleRepository,
        user::{
            dao::user::{AccountStatus, User, UserProfileInfo},
            user_repository::UserRepository,
        },
    },
    security::{
        csrf::{generate_opaque_token, sha256_hex},
        jwt::JwtService,
        password::{hash_password, verify_dummy_password, verify_password},
    },
    services::audit::{AuditEvent, AuditService},
    web::dto::{
        auth::{AuthTokenResponse, LoginRequest, MeResponse, RegisterRequest},
        roles::RoleSummary,
        users::UserResponse,
    },
};

pub struct AuthSession {
    pub response: AuthTokenResponse,
    pub raw_refresh_token: String,
    pub csrf_token: String,
}

#[derive(Clone)]
pub struct AuthService {
    settings: Arc<Settings>,
    jwt_service: Arc<JwtService>,
    user_repo: Arc<dyn UserRepository>,
    role_repo: Arc<dyn RoleRepository>,
    permission_repo: Arc<dyn PermissionRepository>,
    refresh_token_repo: Arc<dyn RefreshTokenRepository>,
    permission_cache: PermissionCache,
    audit_service: AuditService,
}

impl AuthService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        settings: Arc<Settings>,
        jwt_service: Arc<JwtService>,
        user_repo: Arc<dyn UserRepository>,
        role_repo: Arc<dyn RoleRepository>,
        permission_repo: Arc<dyn PermissionRepository>,
        refresh_token_repo: Arc<dyn RefreshTokenRepository>,
        permission_cache: PermissionCache,
        audit_service: AuditService,
    ) -> Self {
        Self {
            settings,
            jwt_service,
            user_repo,
            role_repo,
            permission_repo,
            refresh_token_repo,
            permission_cache,
            audit_service,
        }
    }

    pub async fn register(
        &self,
        req: RegisterRequest,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<AuthSession, AppError> {
        validate_dto(&req)?;

        let email = normalize_email(&req.email);
        if self.user_repo.find_by_email(&email).await?.is_some() {
            return Err(AppError::conflict(
                "EMAIL_ALREADY_EXISTS",
                "An account with this email address already exists",
            ));
        }

        let default_role = self
            .role_repo
            .find_by_name(ROLE_USER)
            .await?
            .ok_or_else(|| AppError::internal("Default 'User' role is missing from database"))?;

        let password_hash = hash_password(req.password).await?;
        let now = BsonDateTime::now();

        let status = if self.settings.app.require_email_verification {
            AccountStatus::PendingVerification
        } else {
            AccountStatus::Active
        };

        let user = User {
            id: ObjectId::new(),
            first_name: req.first_name.trim().to_string(),
            last_name: req.last_name.trim().to_string(),
            email,
            password_hash,
            status,
            email_verified: false,
            roles: vec![default_role.id],
            token_version: 1,
            profile: UserProfileInfo::default(),
            created_at: now,
            updated_at: now,
            last_login_at: Some(now),
        };

        let created = self.user_repo.create(&user).await?;
        let family_id = Uuid::new_v4().to_string();
        let created_id = created.id;
        let created_email = created.email.clone();
        let session = self
            .issue_session(created, family_id, user_agent.clone(), ip_address.clone())
            .await?;

        self.audit_service
            .record(
                AuditEvent::system(
                    "auth.registered",
                    "auth",
                    format!("Registered new account '{created_email}'"),
                )
                .with_actor(Some(created_id), Some(created_email.clone()))
                .with_resource(created_id.to_hex(), created_email)
                .with_details(json!({
                    "ip_address": ip_address,
                    "user_agent": user_agent,
                })),
            )
            .await;

        Ok(session)
    }

    pub async fn login(
        &self,
        req: LoginRequest,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<AuthSession, AppError> {
        validate_dto(&req)?;

        let email = normalize_email(&req.email);
        let Some(mut user) = self.user_repo.find_by_email(&email).await? else {
            // Equalize Argon2id latency for unknown emails to prevent timing-based user enumeration.
            verify_dummy_password(req.password).await;
            self.audit_service
                .record(
                    AuditEvent::system(
                        "auth.login_failed",
                        "auth",
                        format!("Failed login attempt for '{email}'"),
                    )
                    .with_actor(None, Some(email.clone()))
                    .with_details(json!({
                        "reason": "invalid_credentials",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                    }))
                    .as_warning(),
                )
                .await;
            return Err(AppError::unauthorized(
                "INVALID_CREDENTIALS",
                "Invalid email or password",
            ));
        };

        let password_valid = verify_password(req.password, user.password_hash.clone()).await?;
        if !password_valid {
            self.audit_service
                .record(
                    AuditEvent::system(
                        "auth.login_failed",
                        "auth",
                        format!("Failed login attempt for '{}'", user.email),
                    )
                    .with_actor(Some(user.id), Some(user.email.clone()))
                    .with_resource(user.id.to_hex(), user.email.clone())
                    .with_details(json!({
                        "reason": "invalid_credentials",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                    }))
                    .as_warning(),
                )
                .await;
            return Err(AppError::unauthorized(
                "INVALID_CREDENTIALS",
                "Invalid email or password",
            ));
        }

        ensure_account_can_authenticate(&user)?;

        let now = BsonDateTime::now();
        user.last_login_at = Some(now);
        user.updated_at = now;
        let updated = self.user_repo.update(&user).await?;

        let family_id = Uuid::new_v4().to_string();
        let user_id = updated.id;
        let user_email = updated.email.clone();
        let session = self
            .issue_session(updated, family_id, user_agent.clone(), ip_address.clone())
            .await?;

        self.audit_service
            .record(
                AuditEvent::system(
                    "auth.login",
                    "auth",
                    format!("User '{user_email}' signed in"),
                )
                .with_actor(Some(user_id), Some(user_email.clone()))
                .with_resource(user_id.to_hex(), user_email)
                .with_details(json!({
                    "ip_address": ip_address,
                    "user_agent": user_agent,
                })),
            )
            .await;

        Ok(session)
    }

    pub async fn refresh(
        &self,
        raw_refresh_token: &str,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<AuthSession, AppError> {
        if raw_refresh_token.trim().is_empty() {
            return Err(AppError::unauthorized(
                "MISSING_REFRESH_TOKEN",
                "Refresh token is required",
            ));
        }

        let token_hash = sha256_hex(raw_refresh_token);
        let stored = self
            .refresh_token_repo
            .find_by_hash(&token_hash)
            .await?
            .ok_or_else(|| {
                AppError::unauthorized("INVALID_REFRESH_TOKEN", "Refresh token is invalid")
            })?;

        // Reuse detection: a revoked refresh token presented again indicates theft/replay.
        if stored.is_revoked() {
            self.refresh_token_repo
                .revoke_family(&stored.family_id)
                .await?;
            let user_email = self
                .user_repo
                .find_by_id(stored.user_id)
                .await
                .ok()
                .flatten()
                .map(|u| u.email);
            self.audit_service
                .record(
                    AuditEvent::system(
                        "auth.refresh_token_reused",
                        "auth",
                        "Revoked refresh token reuse detected; token family revoked",
                    )
                    .with_actor(Some(stored.user_id), user_email.clone())
                    .with_resource(
                        stored.user_id.to_hex(),
                        user_email.unwrap_or_else(|| stored.user_id.to_hex()),
                    )
                    .with_details(json!({
                        "family_id": stored.family_id,
                        "ip_address": ip_address,
                    }))
                    .as_warning(),
                )
                .await;
            return Err(AppError::unauthorized(
                "REFRESH_TOKEN_REUSED",
                "Refresh token has already been used or revoked; session terminated",
            ));
        }

        let now = BsonDateTime::now();
        if stored.is_expired(now) {
            self.refresh_token_repo.revoke_by_hash(&token_hash).await?;
            return Err(AppError::unauthorized(
                "REFRESH_TOKEN_EXPIRED",
                "Refresh token has expired; please sign in again",
            ));
        }

        let user = self
            .user_repo
            .find_by_id(stored.user_id)
            .await?
            .ok_or_else(|| {
                AppError::unauthorized("INVALID_SESSION", "Account associated with token not found")
            })?;

        if let Err(err) = ensure_account_can_authenticate(&user) {
            self.refresh_token_repo.revoke_all_for_user(user.id).await?;
            return Err(err);
        }

        let new_raw_token = generate_opaque_token(32);
        let new_token_hash = sha256_hex(&new_raw_token);
        let refresh_expires = Utc::now() + Duration::seconds(self.settings.jwt.refresh_token_ttl);

        let replacement = RefreshToken {
            id: ObjectId::new(),
            user_id: user.id,
            token_hash: new_token_hash,
            family_id: stored.family_id.clone(),
            expires_at: chrono_to_bson_dt(refresh_expires),
            revoked_at: None,
            replaced_by_hash: None,
            user_agent,
            ip_address: ip_address.clone(),
            created_at: now,
        };

        let rotated = self
            .refresh_token_repo
            .rotate(&token_hash, &replacement)
            .await?;

        if !rotated {
            self.refresh_token_repo
                .revoke_family(&stored.family_id)
                .await?;
            self.audit_service
                .record(
                    AuditEvent::system(
                        "auth.refresh_token_reused",
                        "auth",
                        "Concurrent refresh token reuse detected; token family revoked",
                    )
                    .with_actor(Some(user.id), Some(user.email.clone()))
                    .with_resource(user.id.to_hex(), user.email)
                    .with_details(json!({
                        "family_id": stored.family_id,
                        "ip_address": ip_address,
                    }))
                    .as_warning(),
                )
                .await;
            return Err(AppError::unauthorized(
                "REFRESH_TOKEN_REUSED",
                "Concurrent refresh token reuse detected; session terminated",
            ));
        }

        self.build_auth_session(user, new_raw_token).await
    }

    pub async fn logout(&self, raw_refresh_token: Option<&str>) -> Result<(), AppError> {
        if let Some(raw) = raw_refresh_token
            && !raw.trim().is_empty()
        {
            let token_hash = sha256_hex(raw);
            if let Some(stored) = self.refresh_token_repo.find_by_hash(&token_hash).await? {
                self.refresh_token_repo
                    .revoke_family(&stored.family_id)
                    .await?;
                let user_email = self
                    .user_repo
                    .find_by_id(stored.user_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|u| u.email);
                let label = user_email
                    .clone()
                    .unwrap_or_else(|| stored.user_id.to_hex());
                self.audit_service
                    .record(
                        AuditEvent::system(
                            "auth.logout",
                            "auth",
                            format!("User '{label}' signed out"),
                        )
                        .with_actor(Some(stored.user_id), user_email)
                        .with_resource(stored.user_id.to_hex(), label),
                    )
                    .await;
            }
        }
        Ok(())
    }

    pub async fn me(&self, actor: &AuthenticatedUser) -> Result<MeResponse, AppError> {
        let user = self
            .user_repo
            .find_by_id(actor.user_id)
            .await?
            .ok_or_else(|| AppError::not_found("USER_NOT_FOUND", "Authenticated user not found"))?;

        let (role_summaries, permissions) = self.load_roles_and_permissions(&user).await?;
        let user_response =
            UserResponse::from_user(user, role_summaries, Some(permissions.clone()));

        Ok(MeResponse {
            user: user_response,
            permissions,
        })
    }

    async fn issue_session(
        &self,
        user: User,
        family_id: String,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<AuthSession, AppError> {
        let raw_refresh_token = generate_opaque_token(32);
        let token_hash = sha256_hex(&raw_refresh_token);
        let refresh_expires = Utc::now() + Duration::seconds(self.settings.jwt.refresh_token_ttl);

        let record = RefreshToken {
            id: ObjectId::new(),
            user_id: user.id,
            token_hash,
            family_id,
            expires_at: chrono_to_bson_dt(refresh_expires),
            revoked_at: None,
            replaced_by_hash: None,
            user_agent,
            ip_address,
            created_at: BsonDateTime::now(),
        };

        self.refresh_token_repo.create(&record).await?;
        self.build_auth_session(user, raw_refresh_token).await
    }

    async fn build_auth_session(
        &self,
        user: User,
        raw_refresh_token: String,
    ) -> Result<AuthSession, AppError> {
        let (access_token, expires_at) = self
            .jwt_service
            .generate_access_token(user.id, user.token_version)?;
        let csrf_token = generate_opaque_token(24);
        let (role_summaries, permissions) = self.load_roles_and_permissions(&user).await?;
        let user_response =
            UserResponse::from_user(user, role_summaries, Some(permissions.clone()));

        let response = AuthTokenResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: self.jwt_service.access_token_ttl_seconds(),
            expires_at,
            csrf_token: csrf_token.clone(),
            user: user_response,
            permissions,
        };

        Ok(AuthSession {
            response,
            raw_refresh_token,
            csrf_token,
        })
    }

    async fn load_roles_and_permissions(
        &self,
        user: &User,
    ) -> Result<(Vec<RoleSummary>, Vec<String>), AppError> {
        let roles = self.role_repo.find_by_ids(&user.roles).await?;
        let role_summaries: Vec<RoleSummary> = roles.iter().map(RoleSummary::from).collect();
        let resolved = self
            .permission_cache
            .resolve_user_access(user.id, &user.roles, &self.role_repo, &self.permission_repo)
            .await?;

        let mut permissions: Vec<String> = resolved.permissions.into_iter().collect();
        permissions.sort();
        Ok((role_summaries, permissions))
    }
}

fn ensure_account_can_authenticate(user: &User) -> Result<(), AppError> {
    match user.status {
        AccountStatus::Active => Ok(()),
        AccountStatus::Disabled => Err(AppError::forbidden(
            "ACCOUNT_DISABLED",
            "This account has been disabled by an administrator",
        )),
        AccountStatus::Locked => Err(AppError::forbidden(
            "ACCOUNT_LOCKED",
            "This account is currently locked",
        )),
        AccountStatus::PendingVerification => Err(AppError::forbidden(
            "EMAIL_NOT_VERIFIED",
            "Email verification is required before signing in",
        )),
    }
}
