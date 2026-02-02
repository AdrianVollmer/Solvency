#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use http_body_util::BodyExt;
use solvency::config::{AuthMode, Config};
use solvency::server;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tauri::Manager;
use tower::ServiceExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "solvency=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let router: Arc<OnceLock<axum::Router>> = Arc::new(OnceLock::new());

    let protocol_router = Arc::clone(&router);
    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol("solvency", move |_ctx, request, responder| {
            let router = Arc::clone(&protocol_router);
            tauri::async_runtime::spawn(async move {
                let router = router.get().expect("Router not initialized").clone();

                let (parts, body) = request.into_parts();
                let body = axum::body::Body::from(body);
                let request = http::Request::from_parts(parts, body);

                let response = router.oneshot(request).await.expect("Infallible");

                let (parts, body) = response.into_parts();
                let bytes = body
                    .collect()
                    .await
                    .expect("Failed to collect response body")
                    .to_bytes();

                let response = http::Response::from_parts(parts, bytes.to_vec());
                responder.respond(response);
            });
        })
        .setup(move |app| {
            let resource_dir = app.path().resource_dir().ok();
            let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("CARGO_MANIFEST_DIR has no parent")
                .to_path_buf();

            let static_path = resource_dir
                .as_ref()
                .map(|d| d.join("static"))
                .filter(|p| p.exists())
                .unwrap_or_else(|| workspace_root.join("static"));

            let migrations_path = resource_dir
                .as_ref()
                .map(|d| d.join("migrations"))
                .filter(|p| p.exists())
                .unwrap_or_else(|| workspace_root.join("migrations"));

            let data_dir = dirs::data_dir()
                .expect("Failed to resolve data directory")
                .join("solvency");
            std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");
            let database_path = data_dir.join("solvency.db");

            let config = Config {
                host: "127.0.0.1".into(),
                port: 0,
                database_path,
                migrations_path,
                static_path,
                auth_mode: AuthMode::Unauthenticated,
                secure_cookies: false,
            };

            tracing::info!(
                db = %config.database_path.display(),
                static_dir = %config.static_path.display(),
                "Starting embedded Solvency server"
            );

            let (_state, app_router) =
                server::build_app(config).expect("Failed to build Solvency app");
            router.set(app_router).expect("Router already initialized");

            let window = tauri::WebviewWindowBuilder::new(
                app.handle(),
                "main",
                tauri::WebviewUrl::CustomProtocol("solvency://localhost".parse().unwrap()),
            )
            .title("Solvency")
            .inner_size(1280.0, 800.0)
            .min_inner_size(800.0, 600.0)
            .build()
            .expect("Failed to create window");

            let _ = window;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error running Tauri application");
}
