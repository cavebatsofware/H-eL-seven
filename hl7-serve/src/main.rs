// H-eL-seven - a schema-aware HL7 v2 to JSON translator
// Copyright (C) 2026 CavebatSoftware LLC - Grant DeFayette
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, version 3.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! hl7-serve - ultralite static file server for the hl7-ui web bundle.
//!
//! Serves the static bundle (index.html, wasm, JS, fonts, CSS) and gates the
//! definition JSON snapshots behind a Cloudflare Turnstile challenge: clients
//! POST a widget token to /api/verify, which is checked with Cloudflare's
//! siteverify and, on success, sets a signed clearance cookie. Requests to the
//! bundled defs (/assets/defs...) require that cookie. All traffic passes
//! through the basic-axum-rate-limit layers (per-IP), with the client IP taken
//! from Cloudflare's CF-Connecting-IP header.
//!
//! Configuration (environment):
//!   HL7_STATIC_DIR   directory of the built web bundle   (default /srv/hl7-ui)
//!   HL7_BIND         listen address                      (default 0.0.0.0:8080)
//!   TURNSTILE_SECRET Turnstile secret key. Unset => defs gate disabled.
//!   TURNSTILE_SITEKEY Turnstile public site key, handed to the widget.
//!   HL7_COOKIE_KEY   >=64 bytes signing key. Unset => ephemeral (resets on
//!                    restart, invalidating existing clearances).
//!   HL7_CLEARANCE_HOURS clearance cookie lifetime         (default 12)
//!   HL7_RATE_RPM     requests per minute per IP           (default 300)
//!   HL7_RATE_BLOCK_SECS block duration after tripping     (default 900)
//!   HL7_IP_STRATEGY  "cloudflare" | "socket"              (default cloudflare)
//!
//! Secrets support the `_FILE` convention for Docker/Compose secrets: set
//! e.g. TURNSTILE_SECRET_FILE=/run/secrets/turnstile_secret and the value is
//! read from that file (trimmed). Applies to TURNSTILE_SECRET and
//! HL7_COOKIE_KEY. The `_FILE` form takes precedence over the plain variable.

use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{from_fn, from_fn_with_state, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, Key, SameSite, SignedCookieJar};
use basic_axum_rate_limit::{
    rate_limit_middleware, security_context_middleware_with_config, IpExtractionStrategy,
    NoOpOnBlocked, RateLimitConfig, RateLimiter, SecurityContextConfig,
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, time::Duration};
use tower_http::services::ServeDir;

const CLEARANCE_COOKIE: &str = "hl7_clearance";

#[derive(Clone)]
struct AppState {
    /// Turnstile secret; `None` disables the defs gate entirely.
    turnstile_secret: Option<String>,
    /// Public site key handed to the browser widget ("" => no widget).
    turnstile_sitekey: String,
    cookie_key: Key,
    clearance: Duration,
    http: reqwest::Client,
}

// Lets SignedCookieJar pull the signing key out of the router state.
impl axum::extract::FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Key {
        state.cookie_key.clone()
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Read a secret from `${KEY}_FILE` (a file path, for Docker/Compose secrets)
/// if set, otherwise from `$KEY`. Returns `None` when neither is set or the
/// value is empty. A configured `_FILE` that cannot be read is fatal.
fn secret_from_env(key: &str) -> Option<String> {
    if let Ok(path) = std::env::var(format!("{key}_FILE")) {
        let value = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {key}_FILE ({path}): {e}"));
        return Some(value.trim().to_string()).filter(|s| !s.is_empty());
    }
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let static_dir = env_or("HL7_STATIC_DIR", "/srv/hl7-ui");
    let bind: SocketAddr = env_or("HL7_BIND", "0.0.0.0:8080")
        .parse()
        .expect("HL7_BIND must be a socket address");
    let turnstile_secret = secret_from_env("TURNSTILE_SECRET");
    let turnstile_sitekey = env_or("TURNSTILE_SITEKEY", "");

    let cookie_key = match secret_from_env("HL7_COOKIE_KEY") {
        Some(v) => Key::try_from(v.as_bytes()).expect("HL7_COOKIE_KEY must be at least 64 bytes"),
        None => {
            tracing::warn!(
                "HL7_COOKIE_KEY unset; generating an ephemeral signing key. \
                 Turnstile clearances will reset on restart."
            );
            Key::generate()
        }
    };

    let clearance = Duration::from_secs(
        env_or("HL7_CLEARANCE_HOURS", "12")
            .parse::<u64>()
            .expect("HL7_CLEARANCE_HOURS must be an integer")
            * 3600,
    );

    let ip_strategy = match env_or("HL7_IP_STRATEGY", "cloudflare").as_str() {
        "socket" => IpExtractionStrategy::SocketAddr,
        _ => IpExtractionStrategy::cloudflare(),
    };
    let rpm = env_or("HL7_RATE_RPM", "300")
        .parse::<u32>()
        .expect("HL7_RATE_RPM must be an integer");
    let block = env_or("HL7_RATE_BLOCK_SECS", "900")
        .parse::<u64>()
        .expect("HL7_RATE_BLOCK_SECS must be an integer");

    if turnstile_secret.is_none() {
        tracing::warn!("TURNSTILE_SECRET unset; defs gate disabled (all assets public).");
    }

    let state = AppState {
        turnstile_secret,
        turnstile_sitekey,
        cookie_key,
        clearance,
        http: reqwest::Client::new(),
    };

    let rate_limiter = RateLimiter::new(
        RateLimitConfig::new(rpm, Duration::from_secs(block)),
        NoOpOnBlocked,
    );
    let sec_config = SecurityContextConfig::new().with_ip_extraction(ip_strategy);

    // Routes + static fallback, then (inner -> outer): defs gate, rate limit,
    // security context. In axum the LAST .layer() runs FIRST, so the security
    // context (which the rate limiter reads) is outermost, as required.
    let app = Router::new()
        .route("/api/config", get(config))
        .route("/api/verify", post(verify))
        .fallback_service(ServeDir::new(&static_dir))
        .with_state(state.clone())
        .layer(from_fn(cache_control))
        .layer(from_fn_with_state(state, defs_gate))
        .layer(from_fn_with_state(rate_limiter, rate_limit_middleware))
        .layer(from_fn_with_state(
            sec_config,
            security_context_middleware_with_config,
        ));

    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .unwrap_or_else(|e| panic!("bind {bind}: {e}"));
    tracing::info!("hl7-serve listening on {bind}, serving {static_dir}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("server error");
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}

/// dx content-addresses (hashes) asset filenames, so anything under /assets or
/// /wasm can be cached forever; the unhashed HTML shell must not be, so new
/// deploys are picked up immediately.
async fn cache_control(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    let value = if path == "/" || path.ends_with(".html") {
        "no-cache"
    } else if path.starts_with("/assets/") || path.starts_with("/wasm/") {
        "public, max-age=31536000, immutable"
    } else {
        return response;
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    response
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigResponse {
    /// Public Turnstile site key ("" => widget disabled).
    turnstile_sitekey: String,
    /// Whether the defs gate is enforced.
    gate_enabled: bool,
}

async fn config(State(state): State<AppState>) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        turnstile_sitekey: state.turnstile_sitekey.clone(),
        gate_enabled: state.turnstile_secret.is_some(),
    })
}

#[derive(Deserialize)]
struct VerifyRequest {
    token: String,
}

#[derive(Deserialize)]
struct SiteverifyResponse {
    success: bool,
}

/// Validate a Turnstile token; on success set the signed clearance cookie.
async fn verify(
    State(state): State<AppState>,
    jar: SignedCookieJar,
    headers: HeaderMap,
    Json(body): Json<VerifyRequest>,
) -> Response {
    let Some(secret) = state.turnstile_secret.clone() else {
        // Gate disabled: nothing to verify, treat as cleared.
        return (jar.add(clearance_cookie(state.clearance)), StatusCode::OK).into_response();
    };

    let remote_ip = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let mut form = vec![("secret", secret), ("response", body.token)];
    if let Some(ip) = remote_ip {
        form.push(("remoteip", ip));
    }

    let ok = match state
        .http
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&form)
        .send()
        .await
    {
        Ok(resp) => resp
            .json::<SiteverifyResponse>()
            .await
            .map(|v| v.success)
            .unwrap_or(false),
        Err(e) => {
            tracing::warn!("turnstile siteverify request failed: {e}");
            false
        }
    };

    if ok {
        (jar.add(clearance_cookie(state.clearance)), StatusCode::OK).into_response()
    } else {
        (StatusCode::FORBIDDEN, "turnstile verification failed").into_response()
    }
}

fn clearance_cookie(ttl: Duration) -> Cookie<'static> {
    Cookie::build((CLEARANCE_COOKIE, "1"))
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(ttl.as_secs() as i64))
        .build()
}

/// Require a valid clearance cookie for the bundled defs JSON. Everything else
/// (the shell, wasm, fonts, CSS, and the API) passes through.
async fn defs_gate(
    State(state): State<AppState>,
    jar: SignedCookieJar,
    request: Request,
    next: Next,
) -> Response {
    let gated =
        state.turnstile_secret.is_some() && request.uri().path().starts_with("/assets/defs");
    if gated && jar.get(CLEARANCE_COOKIE).is_none() {
        return (
            StatusCode::FORBIDDEN,
            "complete the verification challenge to load HL7 definitions",
        )
            .into_response();
    }
    next.run(request).await
}
