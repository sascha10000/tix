use std::env;

pub struct Config {
    pub db_name: String,
    pub bind_address: String,
    pub db_pool_size: u32,
    pub session_duration_hours: i64,
    pub admin_default_username: String,
    pub admin_default_email: String,
    pub admin_default_password: String,
    /// Externally reachable base URL (no trailing slash), used as OAuth issuer
    /// and to build the MCP resource URL. Must be set when behind a reverse proxy.
    pub public_base_url: String,
    pub oauth_access_token_minutes: i64,
    pub oauth_refresh_token_days: i64,
}

impl Config {
    pub fn from_env() -> Self {
        let bind_address = env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".into());
        Self {
            db_name: env::var("DB_NAME").unwrap_or_else(|_| "ticketsystem.db".into()),
            public_base_url: env::var("PUBLIC_BASE_URL")
                .map(|s| s.trim_end_matches('/').to_string())
                .unwrap_or_else(|_| format!("http://{bind_address}")),
            oauth_access_token_minutes: env::var("OAUTH_ACCESS_TOKEN_MINUTES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(60),
            oauth_refresh_token_days: env::var("OAUTH_REFRESH_TOKEN_DAYS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            bind_address,
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
