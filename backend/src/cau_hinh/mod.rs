//! 1. CẤU HÌNH HỆ THỐNG — Đọc biến môi trường (.env): port, database_url, MQTT, secret_key...

use std::env;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Settings {
    pub app_name: String,
    pub database_url: String,
    pub is_postgres: bool,
    pub secret_key: String,
    pub access_token_expire_minutes: i64,
    pub registration_enabled: bool,
    pub seed_demo_data: bool,
    pub ai_service_token: String,
    pub cors_origins: Vec<String>,
    pub alert_confidence_threshold: f64,
    pub alert_deduplication_seconds: i64,
    pub mqtt_host: String,
    pub mqtt_port: u16,
    pub mqtt_username: String,
    pub mqtt_password: String,
    pub mqtt_enabled: bool,
    pub bootstrap_admin_username: Option<String>,
    pub bootstrap_admin_password: Option<String>,
    pub host: String,
    pub port: u16,
}

impl Settings {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();

        let raw_db_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://security_monitor.db?mode=rwc".to_string());
        let database_url = normalize_database_url(&raw_db_url);
        let is_postgres =
            database_url.starts_with("postgres://") || database_url.starts_with("postgresql://");

        let cors_raw = env::var("CORS_ORIGINS").unwrap_or_else(|_| {
            "http://localhost:8080,http://localhost:5173,http://127.0.0.1:8080".to_string()
        });
        let cors_origins = cors_raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let bootstrap_admin_username = env::var("BOOTSTRAP_ADMIN_USERNAME")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let bootstrap_admin_password = env::var("BOOTSTRAP_ADMIN_PASSWORD")
            .ok()
            .filter(|s| !s.is_empty());

        Self {
            app_name: env::var("APP_NAME")
                .unwrap_or_else(|_| "Security Monitoring API".to_string()),
            database_url,
            is_postgres,
            secret_key: env::var("SECRET_KEY")
                .unwrap_or_else(|_| "change-this-development-secret-before-deploying".to_string()),
            access_token_expire_minutes: parse_env("ACCESS_TOKEN_EXPIRE_MINUTES", 60),
            registration_enabled: parse_bool_env("REGISTRATION_ENABLED", false),
            seed_demo_data: parse_bool_env("SEED_DEMO_DATA", false),
            ai_service_token: env::var("AI_SERVICE_TOKEN").unwrap_or_default(),
            cors_origins,
            alert_confidence_threshold: parse_env("ALERT_CONFIDENCE_THRESHOLD", 0.8),
            alert_deduplication_seconds: parse_env("ALERT_DEDUPLICATION_SECONDS", 60),
            mqtt_host: env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".to_string()),
            mqtt_port: parse_env("MQTT_PORT", 1883),
            mqtt_username: env::var("MQTT_USERNAME").unwrap_or_default(),
            mqtt_password: env::var("MQTT_PASSWORD").unwrap_or_default(),
            mqtt_enabled: parse_bool_env("MQTT_ENABLED", false),
            bootstrap_admin_username,
            bootstrap_admin_password,
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: parse_env("PORT", 8000),
        }
    }
}

fn normalize_database_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("postgresql+psycopg://") {
        return format!("postgresql://{rest}");
    }
    if let Some(rest) = trimmed.strip_prefix("postgresql+asyncpg://") {
        return format!("postgresql://{rest}");
    }
    if let Some(rest) = trimmed.strip_prefix("sqlite:///./") {
        let path = rest.split('?').next().unwrap_or(rest);
        return format!("sqlite://{path}?mode=rwc");
    }
    if let Some(rest) = trimmed.strip_prefix("sqlite:///") {
        if !rest.contains("?mode=") {
            return format!("sqlite://{rest}?mode=rwc");
        }
        return format!("sqlite://{rest}");
    }
    if trimmed.starts_with("sqlite://")
        && !trimmed.contains("?mode=")
        && !trimmed.contains(":memory:")
    {
        return format!("{trimmed}?mode=rwc");
    }
    trimmed.to_string()
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<T>().ok())
        .unwrap_or(default)
}

fn parse_bool_env(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(val) => matches!(
            val.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}
