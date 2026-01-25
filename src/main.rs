use axum::middleware;
use axum::Router;
use moneymapper::config::Config;
use moneymapper::db::{create_pool, migrations};
use moneymapper::handlers;
use moneymapper::state::{AppState, JsManifest};
use moneymapper::xsrf::{xsrf_middleware, XsrfToken};
use std::sync::Arc;
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
                .unwrap_or_else(|_| "moneymapper=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    tracing::info!("Starting MoneyMapper on {}", config.address());

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
    };

    let app = Router::new()
        .merge(handlers::routes())
        .nest_service("/static", ServeDir::new(&config.static_path))
        .layer(middleware::from_fn(move |req, next| {
            let token = xsrf_token.clone();
            xsrf_middleware(token, req, next)
        }))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = TcpListener::bind(config.address())
        .await
        .expect("Failed to bind address");

    tracing::info!("Listening on http://{}", config.address());

    axum::serve(listener, app).await.expect("Server error");
}
