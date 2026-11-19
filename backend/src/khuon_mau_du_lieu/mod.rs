//! 5. KHUÔN MẪU DỮ LIỆU — JSON Request/Response (Login, UserCreate, CameraCreate, CommandCreate...).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Login {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct Register {
    pub username: String,
    pub password: String,
    pub full_name: String,
}

#[derive(Debug, Deserialize)]
pub struct UserCreate {
    pub username: String,
    pub password: String,
    pub full_name: String,
    #[serde(default = "default_operator_role")]
    pub role: String,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct UserUpdate {
    pub full_name: Option<String>,
    pub role: Option<String>,
    pub is_active: Option<bool>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CameraCreate {
    pub name: String,
    pub ip_address: Option<String>,
    pub rtsp_url: String,
    pub location: Option<String>,
    #[serde(default = "default_online_status")]
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct CameraUpdate {
    pub name: String,
    pub ip_address: Option<String>,
    pub rtsp_url: Option<String>,
    pub location: Option<String>,
    #[serde(default = "default_online_status")]
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct DeviceCreate {
    pub device_code: String,
    pub device_name: String,
    #[serde(default = "default_esp32_type")]
    pub device_type: String,
    #[serde(default = "default_online_status")]
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct DetectionCreate {
    pub camera_id: i64,
    pub object_type: String,
    pub confidence: f64,
    pub event_type: String,
    pub image_path: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
}

impl DetectionCreate {
    pub fn validate(&self) -> Result<(), String> {
        if self.object_type.trim().is_empty() || self.object_type.len() > 80 {
            return Err("object_type must be 1..80 chars".to_string());
        }
        if !(0.0..=1.0).contains(&self.confidence) {
            return Err("confidence must be between 0 and 1".to_string());
        }
        if self.event_type.trim().is_empty() || self.event_type.len() > 80 {
            return Err("event_type must be 1..80 chars".to_string());
        }
        if let Some(ref path) = self.image_path {
            let normalized = path.replace('\\', "/");
            if normalized.starts_with('/')
                || normalized.split('/').any(|p| p == "..")
                || normalized.contains(':')
            {
                return Err("image_path must be a relative path".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct SensorCreate {
    pub device_id: String,
    pub pir: Option<bool>,
    pub mq2: Option<f64>,
    pub relay: Option<bool>,
    pub buzzer: Option<bool>,
    pub lock: Option<bool>,
    pub timestamp: Option<DateTime<Utc>>,
}

impl SensorCreate {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(mq2) = self.mq2 {
            if !(0.0..=100_000.0).contains(&mq2) {
                return Err("mq2 must be between 0 and 100000".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct CommandCreate {
    pub action: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SettingsUpdate {
    #[serde(default = "default_mq2_threshold")]
    pub mq2_threshold: f64,
    #[serde(default = "default_conf_threshold")]
    pub alert_confidence_threshold: f64,
    #[serde(default = "default_dedup_seconds")]
    pub alert_deduplication_seconds: i64,
    #[serde(default = "default_true")]
    pub after_hours_alert: bool,
    #[serde(default = "default_true")]
    pub offline_device_alert: bool,
    #[serde(default = "default_true")]
    pub critical_notification: bool,
    #[serde(default = "default_true")]
    pub camera_offline_notification: bool,
    #[serde(default)]
    pub daily_digest: bool,
    #[serde(default = "default_retention_days")]
    pub retention_days: i64,
    #[serde(default = "default_true")]
    pub auto_save_snapshot: bool,
}

#[derive(Debug, Serialize)]
pub struct Envelope<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

pub fn envelope<T>(items: Vec<T>, total: i64, limit: i64, offset: i64) -> Envelope<T> {
    Envelope {
        items,
        total,
        limit,
        offset,
    }
}

pub fn normalize_role(value: &str) -> Result<String, String> {
    let role = value.trim().to_ascii_uppercase();
    if role != "ADMIN" && role != "OPERATOR" {
        return Err("role must be ADMIN or OPERATOR".to_string());
    }
    Ok(role)
}

fn default_operator_role() -> String {
    "OPERATOR".to_string()
}

fn default_online_status() -> String {
    "ONLINE".to_string()
}

fn default_esp32_type() -> String {
    "ESP32".to_string()
}

fn default_true() -> bool {
    true
}

fn default_mq2_threshold() -> f64 {
    600.0
}

fn default_conf_threshold() -> f64 {
    0.8
}

fn default_dedup_seconds() -> i64 {
    60
}

fn default_retention_days() -> i64 {
    30
}
