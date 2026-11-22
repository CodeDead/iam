use std::{
    collections::{HashMap, HashSet},
    future::{Future, Ready, ready},
    pin::Pin,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use actix_web::{
    Error, HttpMessage,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use mongodb::bson::oid::ObjectId;
use tokio::sync::RwLock;

use crate::{
    common::errors::AppError,
    database::seed::ROLE_SUPER_ADMIN,
    middleware::authentication::AuthenticatedUser,
    repository::{
        permission::permission_repository::PermissionRepository,
        role::role_repository::RoleRepository,
    },
};

#[derive(Debug, Clone)]
pub struct ResolvedAccess {
    pub role_names: HashSet<String>,
    pub permissions: HashSet<String>,
    pub is_super_admin: bool,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    access: ResolvedAccess,
    expires_at: Instant,
}

const MAX_PERMISSION_CACHE_ENTRIES: usize = 10_000;

#[derive(Clone)]
pub struct PermissionCache {
    ttl: Duration,
    entries: Arc<RwLock<HashMap<ObjectId, CacheEntry>>>,
    generation: Arc<AtomicU64>,
}

impl PermissionCache {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            ttl: Duration::from_secs(ttl_seconds),
            entries: Arc::new(RwLock::new(HashMap::new())),
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn resolve_user_access(
        &self,
        user_id: ObjectId,
        role_ids: &[ObjectId],
        role_repo: &Arc<dyn RoleRepository>,
        permission_repo: &Arc<dyn PermissionRepository>,
    ) -> Result<ResolvedAccess, AppError> {
        let start_generation = self.generation.load(Ordering::Acquire);
        if !self.ttl.is_zero() {
            let guard = self.entries.read().await;
            if let Some(entry) = guard.get(&user_id)
                && entry.expires_at > Instant::now()
            {
                return Ok(entry.access.clone());
            }
        }

        let roles = role_repo.find_by_ids(role_ids).await?;
        let mut role_names = HashSet::with_capacity(roles.len());
        let mut perm_ids: HashSet<ObjectId> = HashSet::new();
        let mut is_super_admin = false;

        for role in roles {
            if role.name == ROLE_SUPER_ADMIN {
                is_super_admin = true;
            }
            role_names.insert(role.name);
            for pid in role.permissions {
                perm_ids.insert(pid);
            }
        }

        let perm_id_vec: Vec<ObjectId> = perm_ids.into_iter().collect();
        let perms = permission_repo.find_by_ids(&perm_id_vec).await?;
        let permissions: HashSet<String> = perms.into_iter().map(|p| p.name).collect();

        let access = ResolvedAccess {
            role_names,
            permissions,
            is_super_admin,
        };

        // Only cache if no invalidation occurred while resolving from DB.
        if !self.ttl.is_zero() && self.generation.load(Ordering::Acquire) == start_generation {
            let now = Instant::now();
            let mut guard = self.entries.write().await;
            if self.generation.load(Ordering::Acquire) == start_generation {
                if guard.len() >= MAX_PERMISSION_CACHE_ENTRIES {
                    guard.retain(|_, entry| entry.expires_at > now);
                    if guard.len() >= MAX_PERMISSION_CACHE_ENTRIES
                        && let Some(oldest_key) = guard
                            .iter()
                            .min_by_key(|(_, entry)| entry.expires_at)
                            .map(|(k, _)| *k)
                    {
                        guard.remove(&oldest_key);
                    }
                }
                guard.insert(
                    user_id,
                    CacheEntry {
                        access: access.clone(),
                        expires_at: now + self.ttl,
                    },
                );
            }
        }

        Ok(access)
    }

    pub async fn invalidate_user(&self, user_id: ObjectId) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        let mut guard = self.entries.write().await;
        guard.remove(&user_id);
    }

    pub async fn invalidate_all(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        let mut guard = self.entries.write().await;
        guard.clear();
    }
}

#[derive(Clone)]
pub struct RequirePermission {
    required: Rc<Vec<&'static str>>,
}

impl RequirePermission {
    pub fn new(permission: &'static str) -> Self {
        Self {
            required: Rc::new(vec![permission]),
        }
    }

    pub fn all(permissions: &[&'static str]) -> Self {
        Self {
            required: Rc::new(permissions.to_vec()),
        }
    }
}

pub fn require_permission(permission: &'static str) -> RequirePermission {
    RequirePermission::new(permission)
}

pub fn require_all_permissions(permissions: &[&'static str]) -> RequirePermission {
    RequirePermission::all(permissions)
}

impl<S, B> Transform<S, ServiceRequest> for RequirePermission
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = RequirePermissionMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequirePermissionMiddleware {
            service: Rc::new(service),
            required: Rc::clone(&self.required),
        }))
    }
}

pub struct RequirePermissionMiddleware<S> {
    service: Rc<S>,
    required: Rc<Vec<&'static str>>,
}

impl<S, B> Service<ServiceRequest> for RequirePermissionMiddleware<S>
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
        let required = Rc::clone(&self.required);

        Box::pin(async move {
            let auth_check = {
                let extensions = req.extensions();
                let user = extensions.get::<AuthenticatedUser>().ok_or_else(|| {
                    AppError::unauthorized(
                        "UNAUTHENTICATED",
                        "Authentication is required to access this resource",
                    )
                });
                match user {
                    Ok(u) => u.require_all_permissions(&required),
                    Err(e) => Err(e),
                }
            };

            match auth_check {
                Ok(()) => {
                    let res = service.call(req).await?;
                    Ok(res.map_into_left_body())
                }
                Err(app_err) => Ok(req.error_response(app_err).map_into_right_body()),
            }
        })
    }
}
