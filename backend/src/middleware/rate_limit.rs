use std::{
    collections::{HashMap, VecDeque},
    future::{Future, Ready, ready},
    pin::Pin,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use actix_web::{
    Error,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use tokio::sync::Mutex;

use crate::common::errors::AppError;

#[derive(Clone)]
pub struct RateLimitMiddleware {
    enabled: bool,
    max_requests: u32,
    window: Duration,
    buckets: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

impl RateLimitMiddleware {
    pub fn per_minute(enabled: bool, max_requests: u32) -> Self {
        Self {
            enabled,
            max_requests,
            window: Duration::from_secs(60),
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimitMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitMiddlewareService {
            service: Rc::new(service),
            enabled: self.enabled,
            max_requests: self.max_requests,
            window: self.window,
            buckets: Arc::clone(&self.buckets),
        }))
    }
}

pub struct RateLimitMiddlewareService<S> {
    service: Rc<S>,
    enabled: bool,
    max_requests: u32,
    window: Duration,
    buckets: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddlewareService<S>
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
        if !self.enabled {
            return Box::pin(async move {
                let res = service.call(req).await?;
                Ok(res.map_into_left_body())
            });
        }

        let max_requests = self.max_requests as usize;
        let window = self.window;
        let buckets = Arc::clone(&self.buckets);
        let client_key = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();

        Box::pin(async move {
            let now = Instant::now();
            let exceeded = {
                let mut guard = buckets.lock().await;
                let timestamps = guard.entry(client_key).or_default();
                while let Some(&front) = timestamps.front() {
                    if now.duration_since(front) >= window {
                        timestamps.pop_front();
                    } else {
                        break;
                    }
                }
                if timestamps.len() >= max_requests {
                    true
                } else {
                    timestamps.push_back(now);
                    false
                }
            };

            if exceeded {
                let err =
                    AppError::too_many_requests("Too many requests; please wait before retrying");
                return Ok(req.error_response(err).map_into_right_body());
            }

            let res = service.call(req).await?;
            Ok(res.map_into_left_body())
        })
    }
}
