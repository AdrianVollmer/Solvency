//! Authentication middleware and handlers.
//!
//! This module provides password-based authentication using Argon2 hashed passwords.
//! Authentication can be disabled by setting `PASSWORD_HASH` to
//! `DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS` or by not setting it at all.

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use askama::Template;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use tower_cookies::{Cookie, Cookies};

use crate::config::AuthMode;
use crate::error::RenderHtml;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

/// Cookie name for the session token.
const SESSION_COOKIE: &str = "session";

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
    // Check if authentication is required
    let password_hash = match &state.config.auth_mode {
        AuthMode::Unauthenticated => return next.run(request).await,
        AuthMode::Password(hash) => hash,
    };

    // Check for valid session cookie
    if let Some(session_cookie) = cookies.get(SESSION_COOKIE) {
        // Verify the session token matches the expected format
        // We use a hash of the password hash + a fixed prefix as the session token
        let expected_token = generate_session_token(password_hash);
        if session_cookie.value() == expected_token {
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

/// Handle login form submission.
pub async fn login_submit(
    State(state): State<AppState>,
    cookies: Cookies,
    Form(form): Form<LoginFormData>,
) -> impl IntoResponse {
    let password_hash = match &state.config.auth_mode {
        AuthMode::Unauthenticated => return Redirect::to("/").into_response(),
        AuthMode::Password(hash) => hash,
    };

    // Verify the password
    if verify_password(&form.password, password_hash) {
        // Set session cookie
        let session_token = generate_session_token(password_hash);
        let cookie = Cookie::build((SESSION_COOKIE, session_token))
            .path("/")
            .http_only(true)
            .same_site(tower_cookies::cookie::SameSite::Strict)
            .build();
        cookies.add(cookie);

        return Redirect::to("/").into_response();
    }

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
pub async fn logout(cookies: Cookies) -> impl IntoResponse {
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

/// Generate a session token from the password hash.
/// This is a simple approach where the session is tied to the password hash,
/// meaning all sessions are invalidated when the password changes.
fn generate_session_token(password_hash: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    "solvency_session_v1".hash(&mut hasher);
    password_hash.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
