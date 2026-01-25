//! XSRF (Cross-Site Request Forgery) protection middleware and utilities.
//!
//! This module provides XSRF token generation, validation, and middleware
//! for protecting state-changing requests (POST, PUT, DELETE, PATCH).

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use uuid::Uuid;

/// The header name for XSRF tokens in AJAX/HTMX requests.
pub const XSRF_HEADER: &str = "X-XSRF-Token";

/// The form field name for XSRF tokens in form submissions.
pub const XSRF_FORM_FIELD: &str = "_xsrf_token";

/// XSRF token storage that can be shared across the application.
#[derive(Clone)]
pub struct XsrfToken(Arc<String>);

impl XsrfToken {
    /// Generate a new random XSRF token.
    pub fn generate() -> Self {
        Self(Arc::new(Uuid::new_v4().to_string()))
    }

    /// Get the token value as a string.
    pub fn value(&self) -> &str {
        &self.0
    }
}

/// Middleware that validates XSRF tokens on state-changing requests.
pub async fn xsrf_middleware(
    xsrf_token: XsrfToken,
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();

    // Only check mutating methods
    if !matches!(
        method,
        Method::POST | Method::PUT | Method::DELETE | Method::PATCH
    ) {
        // For non-mutating requests, just continue
        return next.run(request).await;
    }

    // Check for token in header first (for HTMX/AJAX requests)
    let header_token = request
        .headers()
        .get(XSRF_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    if let Some(token) = header_token {
        if token == xsrf_token.value() {
            return next.run(request).await;
        }
        return xsrf_error_response();
    }

    // For form submissions, we need to read the body to check the token
    // Only do this for form-urlencoded content type
    let is_form = request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("application/x-www-form-urlencoded"))
        .unwrap_or(false);

    // Check for multipart forms too
    let is_multipart = request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("multipart/form-data"))
        .unwrap_or(false);

    if is_form {
        // Read and parse the form body
        let (parts, body) = request.into_parts();
        let bytes = match axum::body::to_bytes(body, usize::MAX).await {
            Ok(b) => b,
            Err(_) => return xsrf_error_response(),
        };

        // Parse form data to find XSRF token
        let body_str = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(_) => return xsrf_error_response(),
        };

        let form_token = form_decode::parse(body_str)
            .find(|(key, _)| key == XSRF_FORM_FIELD)
            .map(|(_, value)| value.to_string());

        if let Some(token) = form_token {
            if token == xsrf_token.value() {
                // Reconstruct the request with the body
                let body = Body::from(bytes);
                return next.run(Request::from_parts(parts, body)).await;
            }
        }

        return xsrf_error_response();
    }

    if is_multipart {
        // For multipart forms, we need to check the header since parsing multipart is complex
        // Since we already checked headers above and didn't find one, reject
        return xsrf_error_response();
    }

    // For other content types (like JSON), require the header
    xsrf_error_response()
}

fn xsrf_error_response() -> Response {
    (StatusCode::FORBIDDEN, "Invalid or missing XSRF token").into_response()
}

/// Simple form decoding helper module
mod form_decode {
    use std::borrow::Cow;

    /// Parse URL-encoded form data
    pub fn parse(input: &str) -> impl Iterator<Item = (Cow<'_, str>, Cow<'_, str>)> {
        input.split('&').filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or("");
            Some((decode(key), decode(value)))
        })
    }

    fn decode(input: &str) -> Cow<'_, str> {
        if input.contains('%') || input.contains('+') {
            let mut result = String::with_capacity(input.len());
            let mut chars = input.bytes().peekable();

            while let Some(c) = chars.next() {
                match c {
                    b'+' => result.push(' '),
                    b'%' => {
                        let h1 = chars.next().and_then(|c| (c as char).to_digit(16));
                        let h2 = chars.next().and_then(|c| (c as char).to_digit(16));
                        if let (Some(h1), Some(h2)) = (h1, h2) {
                            result.push(((h1 << 4) | h2) as u8 as char);
                        } else {
                            result.push('%');
                        }
                    }
                    _ => result.push(c as char),
                }
            }

            Cow::Owned(result)
        } else {
            Cow::Borrowed(input)
        }
    }
}
