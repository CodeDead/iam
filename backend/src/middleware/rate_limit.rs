use std::{
    collections::{HashMap, VecDeque},
    future::{Future, Ready, ready},
    net::{IpAddr, SocketAddr},
    pin::Pin,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use actix_web::{
    Error, HttpRequest,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use tokio::sync::Mutex;

use crate::common::errors::AppError;

const CF_CONNECTING_IP_HEADER: &str = "cf-connecting-ip";
const MAX_TRACKED_KEYS: usize = 50_000;
const SWEEP_THRESHOLD: usize = 1_024;
const SWEEP_INTERVAL: Duration = Duration::from_secs(30);

struct RateLimitState {
    buckets: HashMap<String, VecDeque<Instant>>,
    last_sweep: Instant,
}

impl RateLimitState {
    fn new() -> Self {
        Self {
            buckets: HashMap::new(),
            last_sweep: Instant::now(),
        }
    }

    fn sweep_stale(&mut self, now: Instant, window: Duration) {
        self.buckets.retain(|_, timestamps| {
            while let Some(&front) = timestamps.front() {
                if now.duration_since(front) >= window {
                    timestamps.pop_front();
                } else {
                    break;
                }
            }
            !timestamps.is_empty()
        });
        self.last_sweep = now;
    }
}

#[derive(Clone)]
pub struct RateLimitMiddleware {
    enabled: bool,
    max_requests: u32,
    window: Duration,
    trust_proxy_headers: bool,
    state: Arc<Mutex<RateLimitState>>,
}

impl RateLimitMiddleware {
    pub fn per_minute(enabled: bool, max_requests: u32, trust_proxy_headers: bool) -> Self {
        Self {
            enabled,
            max_requests,
            window: Duration::from_secs(60),
            trust_proxy_headers,
            state: Arc::new(Mutex::new(RateLimitState::new())),
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
            trust_proxy_headers: self.trust_proxy_headers,
            state: Arc::clone(&self.state),
        }))
    }
}

pub struct RateLimitMiddlewareService<S> {
    service: Rc<S>,
    enabled: bool,
    max_requests: u32,
    window: Duration,
    trust_proxy_headers: bool,
    state: Arc<Mutex<RateLimitState>>,
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
        if !self.enabled || self.max_requests == 0 {
            return Box::pin(async move {
                let res = service.call(req).await?;
                Ok(res.map_into_left_body())
            });
        }

        let max_requests = self.max_requests as usize;
        let window = self.window;
        let state = Arc::clone(&self.state);
        let client_key = extract_client_ip(req.request(), self.trust_proxy_headers)
            .unwrap_or_else(|| "unknown".to_string());

        Box::pin(async move {
            let now = Instant::now();
            let exceeded = {
                let mut guard = state.lock().await;
                if guard.buckets.len() >= SWEEP_THRESHOLD
                    && now.duration_since(guard.last_sweep) >= SWEEP_INTERVAL
                {
                    guard.sweep_stale(now, window);
                }

                if !guard.buckets.contains_key(&client_key)
                    && guard.buckets.len() >= MAX_TRACKED_KEYS
                {
                    guard.sweep_stale(now, window);
                    if guard.buckets.len() >= MAX_TRACKED_KEYS {
                        true
                    } else {
                        let timestamps = guard.buckets.entry(client_key).or_default();
                        timestamps.push_back(now);
                        false
                    }
                } else {
                    let timestamps = guard.buckets.entry(client_key).or_default();
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

/// Extracts the caller's IP address for rate limiting and audit logging.
///
/// When `trust_proxy_headers` is `true`, prefers Cloudflare's `CF-Connecting-IP`
/// header (which Cloudflare Edge overwrites on every tunnelled request and Traefik
/// forwards untouched), then falls back to standard forwarded headers via Actix's
/// `realip_remote_addr()`, and finally the direct socket peer address.
///
/// When `trust_proxy_headers` is `false`, ignores all proxy headers and uses only
/// the direct socket peer address.
pub fn extract_client_ip(req: &HttpRequest, trust_proxy_headers: bool) -> Option<String> {
    if trust_proxy_headers {
        if let Some(cf_ip) = req
            .headers()
            .get(CF_CONNECTING_IP_HEADER)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_ip_str)
        {
            return Some(cf_ip.to_string());
        }

        if let Some(real_ip) = req.connection_info().realip_remote_addr() {
            if let Some(parsed) = parse_ip_str(real_ip) {
                return Some(parsed.to_string());
            }
            let trimmed = real_ip.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.chars().take(64).collect());
            }
        }
    }

    req.peer_addr().map(|addr| addr.ip().to_string())
}

fn parse_ip_str(raw: &str) -> Option<IpAddr> {
    let trimmed = raw.trim();
    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return Some(ip);
    }
    trimmed.parse::<SocketAddr>().ok().map(|sa| sa.ip())
}

#[cfg(test)]
mod tests {
    use actix_web::test::TestRequest;

    use super::*;

    #[test]
    fn prefers_cf_connecting_ip_when_proxy_headers_trusted() {
        let req = TestRequest::default()
            .insert_header(("CF-Connecting-IP", "203.0.113.42"))
            .insert_header(("X-Forwarded-For", "10.0.3.13"))
            .to_http_request();

        assert_eq!(
            extract_client_ip(&req, true).as_deref(),
            Some("203.0.113.42")
        );
    }

    #[test]
    fn ignores_invalid_cf_connecting_ip_and_falls_back_to_forwarded_for() {
        let req = TestRequest::default()
            .insert_header(("CF-Connecting-IP", "not-an-ip"))
            .insert_header(("X-Forwarded-For", "198.51.100.7"))
            .to_http_request();

        assert_eq!(
            extract_client_ip(&req, true).as_deref(),
            Some("198.51.100.7")
        );
    }

    #[test]
    fn ignores_proxy_headers_when_untrusted() {
        let req = TestRequest::default()
            .insert_header(("CF-Connecting-IP", "203.0.113.42"))
            .insert_header(("X-Forwarded-For", "198.51.100.7"))
            .to_http_request();

        assert_eq!(extract_client_ip(&req, false), None);
    }
}
