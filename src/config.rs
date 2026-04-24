use std::env;

pub struct Config {
    pub db_name: String,
    pub bind_address: String,
    pub db_pool_size: u32,
    pub session_duration_hours: i64,
    pub admin_default_username: String,
    pub admin_default_email: String,
    pub admin_default_password: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            db_name: env::var("DB_NAME").unwrap_or_else(|_| "ticketsystem.db".into()),
            bind_address: env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".into()),
            db_pool_size: env::var("DB_POOL_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4),
            session_duration_hours: env::var("SESSION_DURATION_HOURS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(24),
            admin_default_username: env::var("ADMIN_DEFAULT_USERNAME")
                .unwrap_or_else(|_| "admin".into()),
            admin_default_email: env::var("ADMIN_DEFAULT_EMAIL")
                .unwrap_or_else(|_| "admin@localhost".into()),
            admin_default_password: env::var("ADMIN_DEFAULT_PASSWORD")
                .unwrap_or_else(|_| "admin".into()),
        }
    }
}
