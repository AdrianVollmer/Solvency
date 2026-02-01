//! Authentication middleware and handlers.
//!
//! This module provides password-based authentication using Argon2 hashed passwords.
//! Authentication can be disabled by setting `PASSWORD_HASH` to
//! `DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS` or by not setting it at all.
//!
//! Session tokens are cryptographically random UUIDs, validated against a
//! server-side session store. Tokens are invalidated on logout or server restart.

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use askama::Template;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tower_cookies::{Cookie, Cookies};
use uuid::Uuid;

use crate::config::AuthMode;
use crate::error::RenderHtml;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

/// Cookie name for the session token.
const SESSION_COOKIE: &str = "session";

/// Maximum number of failed login attempts before rate-limiting kicks in.
const MAX_ATTEMPTS: u32 = 5;
/// Cooldown period after exceeding the attempt limit.
const LOCKOUT_SECS: u64 = 60;

/// Simple per-IP login rate limiter.
pub struct LoginRateLimiter {
    /// Maps IP string → (failed_count, first_failure_time).
    attempts: Mutex<HashMap<String, (u32, Instant)>>,
}

impl Default for LoginRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
        }
    }

    /// Returns `true` if the IP is currently locked out.
    fn is_locked_out(&self, ip: &str) -> bool {
        let mut map = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((count, first_time)) = map.get(ip) {
            if *count >= MAX_ATTEMPTS {
                if first_time.elapsed().as_secs() < LOCKOUT_SECS {
                    return true;
                }
                // Lockout expired, reset
                map.remove(ip);
            }
        }
        false
    }

    /// Record a failed login attempt for the given IP.
    fn record_failure(&self, ip: &str) {
        let mut map = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        let entry = map
            .entry(ip.to_string())
            .or_insert((0, Instant::now()));
        // Reset window if the previous window has expired
        if entry.1.elapsed().as_secs() >= LOCKOUT_SECS {
            *entry = (0, Instant::now());
        }
        entry.0 += 1;
    }

    /// Clear attempts for the given IP (call on successful login).
    fn reset(&self, ip: &str) {
        self.attempts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(ip);
    }
}

/// Template for the login page.
#[derive(Template)]
#[template(path = "pages/login.html")]
pub struct LoginTemplate {
    pub title: String,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub error: Option<String>,
}

/// Form data for login.
#[derive(Debug, Deserialize)]
pub struct LoginFormData {
    pub password: String,
}

/// Authentication middleware that redirects unauthenticated users to the login page.
pub async fn auth_middleware(
    State(state): State<AppState>,
    cookies: Cookies,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Skip auth entirely when no password is configured
    if matches!(state.config.auth_mode, AuthMode::Unauthenticated) {
        return next.run(request).await;
    }

    // Check for valid session cookie against server-side store
    if let Some(session_cookie) = cookies.get(SESSION_COOKIE) {
        let token = session_cookie.value().to_string();
        let is_valid = state
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&token);
        if is_valid {
            return next.run(request).await;
        }
    }

    // Not authenticated - redirect to login (for GET) or return 401 (for other methods)
    let path = request.uri().path();

    // Allow access to login page and static assets
    if path == "/login" || path.starts_with("/static/") || path == "/health" {
        return next.run(request).await;
    }

    // For HTMX requests or API calls, return 401
    let is_htmx = request.headers().contains_key("HX-Request");
    if is_htmx || path.starts_with("/api/") {
        return (StatusCode::UNAUTHORIZED, "Authentication required").into_response();
    }

    // Redirect to login page
    Redirect::to("/login").into_response()
}

/// Render the login page.
pub async fn login_page(State(state): State<AppState>) -> impl IntoResponse {
    // If authentication is not required, redirect to home
    if matches!(state.config.auth_mode, AuthMode::Unauthenticated) {
        return Redirect::to("/").into_response();
    }

    let template = LoginTemplate {
        title: "Login".into(),
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        error: None,
    };

    match template.render_html() {
        Ok(html) => html.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Extract a client identifier from request headers for rate-limiting.
/// Checks X-Forwarded-For and X-Real-Ip headers, falling back to "unknown".
fn client_ip(request: &Request<Body>) -> String {
    request
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            request
                .headers()
                .get("X-Real-Ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// Handle login form submission.
pub async fn login_submit(
    State(state): State<AppState>,
    cookies: Cookies,
    request: Request<Body>,
) -> impl IntoResponse {
    let password_hash = match &state.config.auth_mode {
        AuthMode::Unauthenticated => return Redirect::to("/").into_response(),
        AuthMode::Password(hash) => hash,
    };

    let ip = client_ip(&request);

    // Rate-limit check
    if state.login_rate_limiter.is_locked_out(&ip) {
        tracing::warn!(ip = %ip, "Login rate-limited");
        let template = LoginTemplate {
            title: "Login".into(),
            manifest: state.manifest.clone(),
            version: VERSION,
            xsrf_token: state.xsrf_token.value().to_string(),
            error: Some("Too many failed attempts. Please try again later.".into()),
        };
        return match template.render_html() {
            Ok(html) => html.into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        };
    }

    // Parse the form body
    let body_bytes = match axum::body::to_bytes(request.into_body(), 1024 * 16).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid request").into_response(),
    };
    let form: LoginFormData = match serde_urlencoded::from_bytes(&body_bytes) {
        Ok(f) => f,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid form data").into_response(),
    };

    // Verify the password
    if verify_password(&form.password, password_hash) {
        state.login_rate_limiter.reset(&ip);

        // Generate a cryptographically random session token
        let session_token = Uuid::new_v4().to_string();
        state
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session_token.clone());

        // Rotate the XSRF token so it is bound to this session
        state.xsrf_token.regenerate();

        let cookie = Cookie::build((SESSION_COOKIE, session_token))
            .path("/")
            .http_only(true)
            .secure(state.config.secure_cookies)
            .same_site(tower_cookies::cookie::SameSite::Strict)
            .build();
        cookies.add(cookie);

        return Redirect::to("/").into_response();
    }

    // Record failed attempt
    state.login_rate_limiter.record_failure(&ip);

    // Invalid password - show error
    let template = LoginTemplate {
        title: "Login".into(),
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        error: Some("Invalid password".into()),
    };

    match template.render_html() {
        Ok(html) => html.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Handle logout.
pub async fn logout(State(state): State<AppState>, cookies: Cookies) -> impl IntoResponse {
    // Remove the token from the server-side session store
    if let Some(session_cookie) = cookies.get(SESSION_COOKIE) {
        state
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(session_cookie.value());
    }

    // Remove the session cookie
    let cookie = Cookie::build((SESSION_COOKIE, ""))
        .path("/")
        .http_only(true)
        .build();
    cookies.remove(cookie);

    Redirect::to("/login")
}

/// Verify a password against an Argon2 hash.
fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(hash) else {
        tracing::error!("Invalid password hash format in PASSWORD_HASH");
        return false;
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

