use axum::middleware;
use axum::Router;
use solvency::config::Config;
use solvency::db::{create_pool, migrations};
use solvency::error_pages::{error_page_middleware, fallback_handler};
use solvency::handlers;
use solvency::state::{AppState, JsManifest, MarketDataRefreshState};
use solvency::xsrf::{xsrf_middleware, XsrfToken};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "solvency=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    tracing::info!("Starting Solvency on {}", config.address());

    let db = create_pool(&config.database_path).expect("Failed to create database pool");

    {
        let conn = db.get().expect("Failed to get database connection");
        migrations::run_migrations(&conn, &config.migrations_path)
            .expect("Failed to run migrations");
    }

    let manifest = JsManifest::load();
    let xsrf_token = XsrfToken::generate();
    tracing::info!("Generated XSRF token for session");

    let state = AppState {
        db,
        config: Arc::new(config.clone()),
        manifest,
        xsrf_token: xsrf_token.clone(),
        market_data_refresh: Arc::new(Mutex::new(MarketDataRefreshState::default())),
    };

    let app = Router::new()
        .merge(handlers::routes())
        .fallback(fallback_handler)
        .nest_service("/static", ServeDir::new(&config.static_path))
        .layer(middleware::from_fn(move |req, next| {
            let token = xsrf_token.clone();
            xsrf_middleware(token, req, next)
        }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            error_page_middleware,
        ))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = TcpListener::bind(config.address())
        .await
        .expect("Failed to bind address");

    tracing::info!("Listening on http://{}", config.address());

    axum::serve(listener, app).await.expect("Server error");
}
