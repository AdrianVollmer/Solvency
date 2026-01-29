#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use solvency::config::{AuthMode, Config};
use solvency::server;
use std::path::PathBuf;
use tauri::Manager;
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

    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();

            // In bundled builds, resources are under resource_dir().
            // In dev, fall back to the workspace root via CARGO_MANIFEST_DIR.
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
            };

            tracing::info!(
                db = %config.database_path.display(),
                static_dir = %config.static_path.display(),
                "Starting embedded Solvency server"
            );

            tauri::async_runtime::spawn(async move {
                let (_state, router) =
                    server::build_app(config).expect("Failed to build Solvency app");

                let (port, _handle) = server::serve(router, "127.0.0.1", 0)
                    .await
                    .expect("Failed to start server");

                tracing::info!("Solvency server listening on 127.0.0.1:{}", port);

                let url = format!("http://127.0.0.1:{port}");
                tauri::WebviewWindowBuilder::new(
                    &app_handle,
                    "main",
                    tauri::WebviewUrl::External(url.parse().unwrap()),
                )
                .title("Solvency")
                .inner_size(1280.0, 800.0)
                .min_inner_size(800.0, 600.0)
                .build()
                .expect("Failed to create window");
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error running Tauri application");
}
