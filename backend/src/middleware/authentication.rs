use std::{
    collections::HashSet,
    future::{Future, Ready, ready},
    pin::Pin,
    rc::Rc,
};

use actix_web::{
    Error, FromRequest, HttpMessage, HttpRequest,
    body::EitherBody,
    dev::{Payload, Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header,
    web,
};
use mongodb::bson::oid::ObjectId;

use crate::{
    app::AppState,
    common::{errors::AppError, validation::parse_object_id},
    repository::user::dao::user::AccountStatus,
};

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: ObjectId,
    pub email: String,
    pub role_ids: Vec<ObjectId>,
    pub role_names: HashSet<String>,
    pub permissions: HashSet<String>,
    pub is_super_admin: bool,
}

impl AuthenticatedUser {
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(permission)
    }

    pub fn has_any_permission(&self, permissions: &[&str]) -> bool {
        permissions.iter().any(|p| self.permissions.contains(*p))
    }

    pub fn has_all_permissions(&self, permissions: &[&str]) -> bool {
        permissions.iter().all(|p| self.permissions.contains(*p))
    }

    pub fn require_permission(&self, permission: &str) -> Result<(), AppError> {
        if self.has_permission(permission) {
            Ok(())
        } else {
            Err(AppError::forbidden(
                "INSUFFICIENT_PERMISSIONS",
                format!("Missing required permission: {permission}"),
            ))
        }
    }

    pub fn require_all_permissions(&self, permissions: &[&str]) -> Result<(), AppError> {
        if self.has_all_permissions(permissions) {
            Ok(())
        } else {
            Err(AppError::forbidden(
                "INSUFFICIENT_PERMISSIONS",
                "You do not have permission to perform this operation",
            ))
        }
    }
}

impl FromRequest for AuthenticatedUser {
    type Error = AppError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        if let Some(user) = req.extensions().get::<AuthenticatedUser>() {
            ready(Ok(user.clone()))
        } else {
            ready(Err(AppError::unauthorized(
                "UNAUTHENTICATED",
                "Authentication is required to access this resource",
            )))
        }
    }
}

#[derive(Default, Clone)]
pub struct JwtAuthMiddleware;

impl<S, B> Transform<S, ServiceRequest> for JwtAuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = JwtAuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(JwtAuthMiddlewareService {
            service: Rc::new(service),
        }))
    }
}

pub struct JwtAuthMiddlewareService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for JwtAuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);

        Box::pin(async move {
            match authenticate_request(&req).await {
                Ok(auth_user) => {
                    req.extensions_mut().insert(auth_user);
                    let res = service.call(req).await?;
                    Ok(res.map_into_left_body())
                }
                Err(app_err) => Ok(req.error_response(app_err).map_into_right_body()),
            }
        })
    }
}

async fn authenticate_request(req: &ServiceRequest) -> Result<AuthenticatedUser, AppError> {
    let state = req
        .app_data::<web::Data<AppState>>()
        .cloned()
        .ok_or_else(|| AppError::internal("Application state missing from request"))?;

    let bearer_token = extract_bearer_token(req.request())?;
    let claims = state.jwt_service.verify_access_token(&bearer_token)?;
    let user_id = parse_object_id(&claims.sub)
        .map_err(|_| AppError::unauthorized("INVALID_TOKEN", "Token subject is malformed"))?;

    let user = state.user_repo.find_by_id(user_id).await?.ok_or_else(|| {
        AppError::unauthorized("INVALID_SESSION", "Authenticated user no longer exists")
    })?;

    if user.status != AccountStatus::Active {
        return Err(AppError::forbidden(
            "ACCOUNT_INACTIVE",
            format!(
                "Account is currently {} and cannot access the API",
                user.status.as_str()
            ),
        ));
    }

    if user.token_version != claims.tv {
        return Err(AppError::unauthorized(
            "SESSION_REVOKED",
            "Authentication session has been revoked; please sign in again",
        ));
    }

    let resolved = state
        .permission_cache
        .resolve_user_access(
            user.id,
            &user.roles,
            &state.role_repo,
            &state.permission_repo,
        )
        .await?;

    Ok(AuthenticatedUser {
        user_id: user.id,
        email: user.email,
        role_ids: user.roles,
        role_names: resolved.role_names,
        permissions: resolved.permissions,
        is_super_admin: resolved.is_super_admin,
    })
}

fn extract_bearer_token(req: &HttpRequest) -> Result<String, AppError> {
    let header_value = req
        .headers()
        .get(header::AUTHORIZATION)
        .ok_or_else(|| {
            AppError::unauthorized(
                "MISSING_AUTH_HEADER",
                "Authorization header with Bearer token is required",
            )
        })?
        .to_str()
        .map_err(|_| {
            AppError::unauthorized(
                "INVALID_AUTH_HEADER",
                "Authorization header contains invalid characters",
            )
        })?;

    let mut parts = header_value.splitn(2, ' ');
    let scheme = parts.next().unwrap_or_default();
    let token = parts.next().unwrap_or_default().trim();

    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return Err(AppError::unauthorized(
            "INVALID_AUTH_SCHEME",
            "Authorization header must use Bearer <token> format",
        ));
    }

    Ok(token.to_string())
}
