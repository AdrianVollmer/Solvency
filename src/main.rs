use solvency::config::{AuthMode, Config};
use solvency::server;
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

    match &config.auth_mode {
        AuthMode::Unauthenticated => {
            tracing::warn!("Running without authentication - all users can access the app");
        }
        AuthMode::Password(_) => {
            tracing::info!("Password authentication enabled");
        }
    }

    let host = config.host.clone();
    let port = config.port;

    let (_state, app) = server::build_app(config).expect("Failed to build app");

    let (actual_port, handle) = server::serve(app, &host, port)
        .await
        .expect("Failed to start server");

    tracing::info!("Listening on http://{}:{}", host, actual_port);

    handle.await.expect("Server task panicked");
}
