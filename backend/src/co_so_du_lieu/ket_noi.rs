//! Quản lý Pool kết nối DB, AppState dùng chung và hàm đọc/ánh xạ dòng dữ liệu.

use serde_json::Value;
use sqlx::{any::AnyRow, AnyPool, Row};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::cau_hinh::Settings;
use crate::mo_hinh_du_lieu::{
    Alert, Camera, DetectionEvent, Device, DeviceLog, RuntimeSettings, SensorRecord, SystemLog,
    User,
};

#[derive(Clone)]
pub struct AppState {
    pub db: AnyPool,
    pub settings: Settings,
    pub runtime_settings: Arc<RwLock<RuntimeSettings>>,
    pub ws_tx: broadcast::Sender<Value>,
    pub mqtt_client: Arc<RwLock<Option<rumqttc::AsyncClient>>>,
}

pub fn row_i64(row: &AnyRow, col: &str) -> i64 {
    row.try_get::<i64, _>(col)
        .or_else(|_| row.try_get::<i32, _>(col).map(|v| v as i64))
        .unwrap_or(0)
}

pub fn row_opt_i64(row: &AnyRow, col: &str) -> Option<i64> {
    if let Ok(v) = row.try_get::<Option<i64>, _>(col) {
        return v;
    }
    if let Ok(v) = row.try_get::<Option<i32>, _>(col) {
        return v.map(|x| x as i64);
    }
    None
}

pub fn row_f64(row: &AnyRow, col: &str) -> f64 {
    row.try_get::<f64, _>(col)
        .or_else(|_| row.try_get::<f32, _>(col).map(|v| v as f64))
        .unwrap_or(0.0)
}

pub fn row_opt_f64(row: &AnyRow, col: &str) -> Option<f64> {
    if let Ok(v) = row.try_get::<Option<f64>, _>(col) {
        return v;
    }
    if let Ok(v) = row.try_get::<Option<f32>, _>(col) {
        return v.map(|x| x as f64);
    }
    None
}

pub fn row_bool(row: &AnyRow, col: &str) -> bool {
    row.try_get::<bool, _>(col)
        .or_else(|_| row.try_get::<i64, _>(col).map(|v| v != 0))
        .or_else(|_| row.try_get::<i32, _>(col).map(|v| v != 0))
        .unwrap_or(false)
}

pub fn row_opt_bool(row: &AnyRow, col: &str) -> Option<bool> {
    if let Ok(v) = row.try_get::<Option<bool>, _>(col) {
        return v;
    }
    if let Ok(v) = row.try_get::<Option<i64>, _>(col) {
        return v.map(|x| x != 0);
    }
    if let Ok(v) = row.try_get::<Option<i32>, _>(col) {
        return v.map(|x| x != 0);
    }
    None
}

pub fn row_str(row: &AnyRow, col: &str) -> String {
    row.try_get::<String, _>(col).unwrap_or_default()
}

pub fn row_opt_str(row: &AnyRow, col: &str) -> Option<String> {
    row.try_get::<Option<String>, _>(col).ok().flatten()
}

pub fn map_user(row: &AnyRow) -> User {
    User {
        id: row_i64(row, "id"),
        username: row_str(row, "username"),
        password_hash: row_str(row, "password_hash"),
        full_name: row_str(row, "full_name"),
        role: row_str(row, "role"),
        is_active: row_bool(row, "is_active"),
        created_at: row_str(row, "created_at"),
    }
}

pub fn map_camera(row: &AnyRow) -> Camera {
    Camera {
        id: row_i64(row, "id"),
        name: row_str(row, "name"),
        ip_address: row_opt_str(row, "ip_address"),
        rtsp_url: row_str(row, "rtsp_url"),
        location: row_opt_str(row, "location"),
        status: row_str(row, "status"),
        last_seen: row_opt_str(row, "last_seen"),
        created_at: row_str(row, "created_at"),
        updated_at: row_str(row, "updated_at"),
    }
}

pub fn map_device(row: &AnyRow) -> Device {
    Device {
        id: row_i64(row, "id"),
        device_code: row_str(row, "device_code"),
        device_name: row_str(row, "device_name"),
        device_type: row_str(row, "device_type"),
        status: row_str(row, "status"),
        last_seen: row_opt_str(row, "last_seen"),
        latest_pir: row_opt_bool(row, "latest_pir"),
        latest_mq2: row_opt_f64(row, "latest_mq2"),
        relay: row_opt_bool(row, "relay"),
        buzzer: row_opt_bool(row, "buzzer"),
        lock_engaged: row_opt_bool(row, "lock_engaged"),
        created_at: row_str(row, "created_at"),
    }
}

pub fn map_sensor(row: &AnyRow) -> SensorRecord {
    let raw_json = row_str(row, "sensor_value");
    let sensor_value = serde_json::from_str(&raw_json).unwrap_or(Value::Null);
    SensorRecord {
        id: row_i64(row, "id"),
        device_id: row_str(row, "device_code"),
        sensor_type: row_str(row, "sensor_type"),
        sensor_value,
        created_at: row_str(row, "created_at"),
    }
}

pub fn map_detection(row: &AnyRow) -> DetectionEvent {
    DetectionEvent {
        id: row_i64(row, "id"),
        camera_id: row_i64(row, "camera_id"),
        camera_name: row_opt_str(row, "camera_name"),
        object_type: row_str(row, "object_type"),
        confidence: row_f64(row, "confidence"),
        event_type: row_str(row, "event_type"),
        image_path: row_opt_str(row, "image_path"),
        timestamp: row_str(row, "event_timestamp"),
    }
}

pub fn map_alert(row: &AnyRow) -> Alert {
    Alert {
        id: row_i64(row, "id"),
        detection_id: row_opt_i64(row, "detection_id"),
        alert_type: row_str(row, "alert_type"),
        severity: row_str(row, "severity"),
        description: row_str(row, "description"),
        status: row_str(row, "status"),
        handled_by: row_opt_i64(row, "handled_by"),
        handled_by_name: row_opt_str(row, "handled_by_name"),
        created_at: row_str(row, "created_at"),
        handled_at: row_opt_str(row, "handled_at"),
    }
}

pub fn map_device_log(row: &AnyRow) -> DeviceLog {
    let detail = row_opt_str(row, "detail").and_then(|s| serde_json::from_str::<Value>(&s).ok());
    DeviceLog {
        id: row_i64(row, "id"),
        device_id: row_i64(row, "device_id"),
        command_id: row_str(row, "command_id"),
        command: row_str(row, "command"),
        result: row_str(row, "result"),
        detail,
        created_at: row_str(row, "created_at"),
        completed_at: row_opt_str(row, "completed_at"),
    }
}

pub fn map_system_log(row: &AnyRow) -> SystemLog {
    SystemLog {
        id: row_i64(row, "id"),
        user_id: row_opt_i64(row, "user_id"),
        action: row_str(row, "action"),
        description: row_str(row, "description"),
        created_at: row_str(row, "created_at"),
    }
}
