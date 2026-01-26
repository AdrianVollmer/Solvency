use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_path: PathBuf,
    pub migrations_path: PathBuf,
    pub static_path: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(7070),
            database_path: env::var("DATABASE_URL")
                .map(|v| {
                    PathBuf::from(
                        v.strip_prefix("sqlite://")
                            .or_else(|| v.strip_prefix("sqlite:"))
                            .unwrap_or(&v),
                    )
                })
                .unwrap_or_else(|_| PathBuf::from("data/moneymapper.db")),
            migrations_path: env::var("MIGRATIONS_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("migrations")),
            static_path: env::var("STATIC_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("static")),
        }
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
