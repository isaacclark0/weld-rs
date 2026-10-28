use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use url::Url;

#[derive(Clone)]
pub struct SecurityState {
    scheme: String,
    configured_host: String,
    configured_port: Option<u16>,
    allow_loopback_aliases: bool,
    buckets: Arc<Mutex<HashMap<(IpAddr, &'static str), Bucket>>>,
    last_cleanup: Arc<Mutex<Instant>>,
}

struct Bucket {
    started: Instant,
    count: u16,
}

impl SecurityState {
    pub fn new(app_url: &str) -> Self {
        let parsed = Url::parse(app_url).expect("APP_PUBLIC_URL must be an absolute URL");
        let configured_host = parsed
            .host_str()
            .expect("APP_PUBLIC_URL must include a host")
            .to_ascii_lowercase();
        let allow_loopback_aliases = is_loopback(&configured_host);
        Self {
            scheme: parsed.scheme().to_string(),
            configured_host,
            configured_port: parsed.port_or_known_default(),
            allow_loopback_aliases,
            buckets: Arc::new(Mutex::new(HashMap::new())),
            last_cleanup: Arc::new(Mutex::new(Instant::now())),
        }
    }
}

fn is_loopback(host: &str) -> bool {
    matches!(
        host.to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1"
    )
}

fn policy(path: &str) -> Option<(&'static str, u16, Duration)> {
    match path {
        "/login" => Some(("login", 10, Duration::from_secs(60))),
        "/signup" => Some(("signup", 5, Duration::from_secs(3600))),
        "/forgot" => Some(("forgot", 5, Duration::from_secs(3600))),
        "/reset" => Some(("reset", 10, Duration::from_secs(900))),
        "/verify" => Some(("verify", 20, Duration::from_secs(60))),
        "/invites" => Some(("invites", 5, Duration::from_secs(3600))),
        _ => None,
    }
}

fn allowed_origin(request: &Request, state: &SecurityState) -> bool {
    let source = request
        .headers()
        .get(header::ORIGIN)
        .or_else(|| request.headers().get(header::REFERER));
    let Some(source) = source.and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let Ok(source) = Url::parse(source) else {
        return false;
    };
    let Some(host) = request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
    else {
        return false;
    };
    let Ok(target) = Url::parse(&format!("{}://{host}", state.scheme)) else {
        return false;
    };
    let target_host = target.host_str().unwrap_or_default().to_ascii_lowercase();
    let target_is_configured = target_host == state.configured_host
        && target.port_or_known_default() == state.configured_port;
    let target_is_local_alias = state.allow_loopback_aliases && is_loopback(&target_host);
    if !target_is_configured && !target_is_local_alias {
        return false;
    }
    if source.origin() != target.origin() {
        return false;
    }
    !request
        .headers()
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("cross-site"))
}

fn take_rate_limit(
    state: &SecurityState,
    ip: IpAddr,
    key: &'static str,
    max: u16,
    window: Duration,
) -> bool {
    // Group IPv6 addresses by /64 so address rotation inside a subnet doesn't bypass limits.
    let ip = match ip {
        IpAddr::V6(ip) => {
            let mut segments = ip.segments();
            segments[4..].fill(0);
            IpAddr::V6(segments.into())
        }
        IpAddr::V4(ip) => IpAddr::V4(ip),
    };
    let now = Instant::now();
    if let Ok(mut last_cleanup) = state.last_cleanup.lock() {
        if now.duration_since(*last_cleanup) >= Duration::from_secs(60) {
            if let Ok(mut buckets) = state.buckets.lock() {
                buckets.retain(|_, bucket| {
                    now.duration_since(bucket.started) < Duration::from_secs(3600)
                });
            }
            *last_cleanup = now;
        }
    }
    let Ok(mut buckets) = state.buckets.lock() else {
        return false;
    };
    if buckets.len() >= 16_384 && !buckets.contains_key(&(ip, key)) {
        return false;
    }
    let bucket = buckets.entry((ip, key)).or_insert(Bucket {
        started: now,
        count: 0,
    });
    if now.duration_since(bucket.started) >= window {
        bucket.started = now;
        bucket.count = 0;
    }
    if bucket.count >= max {
        return false;
    }
    bucket.count += 1;
    true
}

pub async fn protect_requests(
    State(state): State<SecurityState>,
    request: Request,
    next: Next,
) -> Response {
    let token_authorized = matches!(request.uri().path(), "/verify" | "/reset");
    if !token_authorized
        && matches!(
            *request.method(),
            Method::POST | Method::PUT | Method::PATCH | Method::DELETE
        )
        && !allowed_origin(&request, &state)
    {
        return StatusCode::FORBIDDEN.into_response();
    }

    let Some((key, max, window)) = (*request.method() == Method::POST)
        .then(|| policy(request.uri().path()))
        .flatten()
    else {
        return next.run(request).await;
    };
    let Some(ConnectInfo(address)) = request.extensions().get::<ConnectInfo<SocketAddr>>() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if !take_rate_limit(&state, address.ip(), key, max, window) {
        let mut response = StatusCode::TOO_MANY_REQUESTS.into_response();
        let retry_after = HeaderValue::from_bytes(window.as_secs().to_string().as_bytes())
            .unwrap_or_else(|_| HeaderValue::from_static("60"));
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, retry_after);
        return response;
    }
    next.run(request).await
}
