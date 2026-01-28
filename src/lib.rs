pub mod auth;
pub mod config;
pub mod date_utils;
pub mod db;
pub mod error;
pub mod error_pages;
pub mod filters;
pub mod handlers;
pub mod models;
pub mod services;
pub mod sort_utils;
pub mod state;
pub mod xsrf;

/// Application version from Cargo.toml (single source of truth)
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
