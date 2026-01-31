use askama::Template;
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Response};

use crate::config::AuthMode;
use crate::db::queries::settings;
use crate::filters::Icons;
use crate::models::Settings;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

/// Newtype for passing error messages through response extensions.
#[derive(Clone)]
pub struct ErrorMessage(pub String);

#[derive(Template)]
#[template(path = "pages/error.html")]
struct ErrorPageTemplate {
    title: String,
    settings: Settings,
    icons: Icons,
    manifest: JsManifest,
    version: &'static str,
    xsrf_token: String,
    status_code: u16,
    status_text: &'static str,
    message: String,
}

/// Middleware that replaces 4xx/5xx responses with a full error page.
///
/// Skips HTMX requests, API routes, and the health endpoint so they
/// keep their original (partial/JSON/plain) response bodies.
pub async fn error_page_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let is_htmx = request.headers().contains_key("hx-request");
    let path = request.uri().path().to_owned();
    let is_api = path.starts_with("/api/");
    let is_health = path == "/health";

    let method = request.method().clone();
    let response = next.run(request).await;

    let status = response.status();
    if status.is_client_error() || status.is_server_error() {
        let message = response
            .extensions()
            .get::<ErrorMessage>()
            .map(|e| e.0.as_str())
            .unwrap_or("");
        tracing::warn!(
            %status,
            %method,
            %path,
            message,
            "request failed"
        );
    }

    if is_htmx || is_api || is_health {
        return response;
    }

    if status.is_client_error() || status.is_server_error() {
        render_error_page(&state, status, &response)
    } else {
        response
    }
}

/// Fallback handler for unmatched routes.
pub async fn fallback_handler() -> Response {
    let mut response = StatusCode::NOT_FOUND.into_response();
    response.extensions_mut().insert(ErrorMessage(
        "The page you're looking for doesn't exist.".into(),
    ));
    response
}

fn render_error_page(state: &AppState, status: StatusCode, response: &Response) -> Response {
    let message = response
        .extensions()
        .get::<ErrorMessage>()
        .map(|e| e.0.clone())
        .unwrap_or_else(|| default_message(status));

    let (status_text, _) = status_info(status);

    let mut settings = state
        .db
        .get()
        .ok()
        .and_then(|conn| settings::get_settings(&conn).ok())
        .unwrap_or_default();
    settings.is_authenticated = matches!(state.config.auth_mode, AuthMode::Password(_));

    let template = ErrorPageTemplate {
        title: status_text.to_string(),
        settings,
        icons: Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        status_code: status.as_u16(),
        status_text,
        message,
    };

    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => {
            tracing::error!("Failed to render error page template: {}", e);
            (status, "Internal Server Error").into_response()
        }
    }
}

fn status_info(status: StatusCode) -> (&'static str, &'static str) {
    match status.as_u16() {
        400 => ("Bad Request", "The request could not be understood."),
        403 => ("Forbidden", "You don't have permission to access this."),
        404 => ("Not Found", "The page you're looking for doesn't exist."),
        405 => ("Method Not Allowed", "This action is not supported."),
        500 => ("Internal Server Error", "Something went wrong on our end."),
        _ => ("Error", ""),
    }
}

fn default_message(status: StatusCode) -> String {
    let msg = status_info(status).1;
    if msg.is_empty() {
        format!("An unexpected error occurred ({}).", status.as_u16())
    } else {
        msg.to_string()
    }
}
