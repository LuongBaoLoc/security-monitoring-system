//! 4. MÔ HÌNH DỮ LIỆU — Struct ánh xạ các bảng trong Database (User, Camera, Device, Alert...).

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub full_name: String,
    pub role: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Camera {
    pub id: i64,
    pub name: String,
    pub ip_address: Option<String>,
    pub rtsp_url: String,
    pub location: Option<String>,
    pub status: String,
    pub last_seen: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Device {
    pub id: i64,
    pub device_code: String,
    pub device_name: String,
    pub device_type: String,
    pub status: String,
    pub last_seen: Option<String>,
    pub latest_pir: Option<bool>,
    pub latest_mq2: Option<f64>,
    pub relay: Option<bool>,
    pub buzzer: Option<bool>,
    pub lock_engaged: Option<bool>,
    #[serde(skip_serializing)]
    #[allow(dead_code)]
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorRecord {
    pub id: i64,
    pub device_id: String,
    pub sensor_type: String,
    pub sensor_value: Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetectionEvent {
    pub id: i64,
    pub camera_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera_name: Option<String>,
    pub object_type: String,
    pub confidence: f64,
    pub event_type: String,
    pub image_path: Option<String>,
    pub timestamp: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alert {
    pub id: i64,
    pub detection_id: Option<i64>,
    pub alert_type: String,
    pub severity: String,
    pub description: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handled_by: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handled_by_name: Option<String>,
    pub created_at: String,
    pub handled_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceLog {
    pub id: i64,
    pub device_id: i64,
    pub command_id: String,
    pub command: String,
    pub result: String,
    pub detail: Option<Value>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemLog {
    pub id: i64,
    pub user_id: Option<i64>,
    pub action: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeSettings {
    pub mq2_threshold: f64,
    pub alert_confidence_threshold: f64,
    pub alert_deduplication_seconds: i64,
    pub after_hours_alert: bool,
    pub offline_device_alert: bool,
    pub critical_notification: bool,
    pub camera_offline_notification: bool,
    pub daily_digest: bool,
    pub retention_days: i64,
    pub auto_save_snapshot: bool,
}
