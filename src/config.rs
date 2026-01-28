use std::env;
use std::path::PathBuf;

/// Authentication mode for the application.
#[derive(Debug, Clone)]
pub enum AuthMode {
    /// No authentication required - all users can access the app.
    Unauthenticated,
    /// Password authentication with an Argon2 hash.
    Password(String),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_path: PathBuf,
    pub migrations_path: PathBuf,
    pub static_path: PathBuf,
    pub auth_mode: AuthMode,
}

/// The magic value that disables authentication.
pub const UNAUTHENTICATED_MAGIC: &str = "DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS";

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let auth_mode = match env::var("SOLVENCY_PASSWORD_HASH") {
            Ok(hash) if hash == UNAUTHENTICATED_MAGIC => AuthMode::Unauthenticated,
            Ok(hash) if hash.starts_with("$argon2id$") => AuthMode::Password(hash),
            Ok(hash) if hash.is_empty() => {
                panic!(
                    "SOLVENCY_PASSWORD_HASH is empty. Set a valid Argon2 hash or '{}' to \
                     explicitly allow unauthenticated access.",
                    UNAUTHENTICATED_MAGIC
                );
            }
            Ok(hash) => {
                panic!(
                    "Invalid SOLVENCY_PASSWORD_HASH: must start with '$argon2id$' or be set \
                     to '{}'. Got: {}...",
                    UNAUTHENTICATED_MAGIC,
                    &hash[..hash.len().min(20)]
                );
            }
            Err(_) => {
                panic!(
                    "SOLVENCY_PASSWORD_HASH environment variable is not set. Set a valid \
                     Argon2 hash or '{}' to explicitly allow unauthenticated access.",
                    UNAUTHENTICATED_MAGIC
                );
            }
        };

        Self {
            host: env::var("SOLVENCY_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("SOLVENCY_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(7070),
            database_path: env::var("SOLVENCY_DATABASE_URL")
                .map(|v| {
                    PathBuf::from(
                        v.strip_prefix("sqlite://")
                            .or_else(|| v.strip_prefix("sqlite:"))
                            .unwrap_or(&v),
                    )
                })
                .unwrap_or_else(|_| PathBuf::from("data/solvency.db")),
            migrations_path: env::var("SOLVENCY_MIGRATIONS_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("migrations")),
            static_path: env::var("SOLVENCY_STATIC_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("static")),
            auth_mode,
        }
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
