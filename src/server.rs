use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tower_cookies::CookieManagerLayer;
use axum::extract::DefaultBodyLimit;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::auth;
use crate::cache::{cache_invalidation_middleware, AppCache};
use crate::config::Config;
use crate::db::{create_pool, migrations};
use crate::error_pages::{error_page_middleware, fallback_handler};
use crate::handlers;
use crate::state::{AppState, JsManifest, MarketDataRefreshState};
use crate::xsrf::{xsrf_middleware, XsrfToken};

/// Build the application state and Axum router from a [`Config`].
///
/// Creates the database pool, runs migrations, loads the JS manifest, and
/// assembles the full middleware stack. Returns the shared state and a
/// ready-to-serve router.
pub fn build_app(config: Config) -> Result<(AppState, Router), Box<dyn std::error::Error>> {
    let db = create_pool(&config.database_path)?;

    {
        let conn = db.get()?;
        migrations::run_migrations(&conn, &config.migrations_path)?;
    }

    let manifest = JsManifest::load(&config.static_path);
    let xsrf_token = XsrfToken::generate();
    tracing::info!("Generated XSRF token for session");

    let state = AppState {
        db,
        config: Arc::new(config.clone()),
        manifest,
        xsrf_token: xsrf_token.clone(),
        market_data_refresh: Arc::new(Mutex::new(MarketDataRefreshState::default())),
        cache: Arc::new(AppCache::new()),
    };

    let app = Router::new()
        .merge(handlers::routes())
        .route("/login", get(auth::login_page))
        .route("/login", post(auth::login_submit))
        .route("/logout", post(auth::logout))
        .fallback(fallback_handler)
        .nest_service("/static", ServeDir::new(&config.static_path))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            cache_invalidation_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .layer(middleware::from_fn(move |req, next| {
            let token = xsrf_token.clone();
            xsrf_middleware(token, req, next)
        }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            error_page_middleware,
        ))
        .layer(DefaultBodyLimit::max(256 * 1024 * 1024))
        .layer(CookieManagerLayer::new())
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    Ok((state, app))
}

/// Bind the router to `host:port` and spawn the server as a tokio task.
///
/// Returns the actual port the server bound to (useful when `port` is 0 for
/// OS-assigned ports) and a [`JoinHandle`] for the server task.
pub async fn serve(
    app: Router,
    host: &str,
    port: u16,
) -> Result<(u16, JoinHandle<()>), Box<dyn std::error::Error>> {
    let addr = format!("{host}:{port}");
    let listener = TcpListener::bind(&addr).await?;
    let actual_port = listener.local_addr()?.port();

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("Server error");
    });

    Ok((actual_port, handle))
}
