//! Tự động kiểm tra trạng thái hệ thống và cài đặt (/api/system/health, /api/system/settings).

use axum::{extract::State, http::HeaderMap, Json};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use std::time::Duration as StdDuration;
use tokio::net::TcpStream;

use crate::{
    bao_mat::{admin_user, current_user, ApiError},
    co_so_du_lieu::AppState,
    dich_vu_nghiep_vu::record_system_log,
    khuon_mau_du_lieu::SettingsUpdate,
};

use super::*;

pub async fn get_system_settings(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let rt = state.runtime_settings.read().await.clone();
    Ok(Json(serde_json::to_value(rt).unwrap()))
}

pub async fn update_system_settings(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<SettingsUpdate>,
) -> Result<Json<Value>, ApiError> {
    let admin = admin_user(&headers, &state).await?;
    if !(50.0..=10_000.0).contains(&payload.mq2_threshold) {
        return Err(ApiError::unprocessable("mq2_threshold out of range"));
    }
    if !(0.1..=1.0).contains(&payload.alert_confidence_threshold) {
        return Err(ApiError::unprocessable(
            "alert_confidence_threshold out of range",
        ));
    }
    if !(0..=3600).contains(&payload.alert_deduplication_seconds) {
        return Err(ApiError::unprocessable(
            "alert_deduplication_seconds out of range",
        ));
    }

    {
        let mut rt = state.runtime_settings.write().await;
        rt.mq2_threshold = payload.mq2_threshold;
        rt.alert_confidence_threshold = payload.alert_confidence_threshold;
        rt.alert_deduplication_seconds = payload.alert_deduplication_seconds;
        rt.after_hours_alert = payload.after_hours_alert;
        rt.offline_device_alert = payload.offline_device_alert;
        rt.critical_notification = payload.critical_notification;
        rt.camera_offline_notification = payload.camera_offline_notification;
        rt.daily_digest = payload.daily_digest;
        rt.retention_days = payload.retention_days;
        rt.auto_save_snapshot = payload.auto_save_snapshot;
    }

    let _ = record_system_log(
        &state.db,
        "SETTINGS_UPDATE",
        &format!(
            "Admin cập nhật cài đặt hệ thống (MQ2={}ppm, AI={:.0}%, Dedup={}s)",
            payload.mq2_threshold,
            payload.alert_confidence_threshold * 100.0,
            payload.alert_deduplication_seconds
        ),
        Some(admin.id),
    )
    .await;

    let updated = state.runtime_settings.read().await.clone();
    Ok(Json(serde_json::to_value(updated).unwrap()))
}

pub async fn system_health(State(state): State<AppState>) -> Json<Value> {
    let db_status = if sqlx::query("SELECT 1").execute(&state.db).await.is_ok() {
        "ok"
    } else {
        "unavailable"
    };

    // 1. Probe MQTT Broker TCP port
    let mut mqtt_broker = "offline";
    if state.settings.mqtt_enabled {
        let addr = format!("{}:{}", state.settings.mqtt_host, state.settings.mqtt_port);
        if tokio::time::timeout(StdDuration::from_millis(350), TcpStream::connect(&addr))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
        {
            mqtt_broker = "online";
        }
    }

    // 2. Check active Cameras & ESP32 Devices in DB
    let mut cam_total = 0;
    let mut cam_online = 0;
    let mut dev_total = 0;
    let mut dev_online = 0;
    let mut recent_ai = 0;

    if db_status == "ok" {
        cam_total = count_query(&state.db, "SELECT COUNT(id) AS cnt FROM cameras", None)
            .await
            .unwrap_or(0);
        cam_online = count_query(
            &state.db,
            "SELECT COUNT(id) AS cnt FROM cameras WHERE status = 'ONLINE'",
            None,
        )
        .await
        .unwrap_or(0);
        dev_total = count_query(&state.db, "SELECT COUNT(id) AS cnt FROM devices", None)
            .await
            .unwrap_or(0);
        dev_online = count_query(
            &state.db,
            "SELECT COUNT(id) AS cnt FROM devices WHERE status = 'ONLINE'",
            None,
        )
        .await
        .unwrap_or(0);
        let cutoff = (Utc::now() - Duration::minutes(10)).to_rfc3339();
        recent_ai = count_query(
            &state.db,
            "SELECT COUNT(id) AS cnt FROM detection_events WHERE event_timestamp >= $1",
            Some(&cutoff),
        )
        .await
        .unwrap_or(0);
    }

    // 3. Probe AI Worker Service
    let mut ai_worker = if recent_ai > 0 { "online" } else { "offline" };
    if ai_worker == "offline" && cam_online > 0 {
        let client = reqwest::Client::builder()
            .timeout(StdDuration::from_millis(350))
            .build()
            .unwrap_or_default();
        for host in ["ai-service", "127.0.0.1"] {
            let url = format!("http://{host}:8001/health");
            if let Ok(resp) = client.get(&url).send().await {
                if let Ok(body) = resp.json::<Value>().await {
                    if body.get("model_configured").and_then(|v| v.as_bool()) == Some(true) {
                        ai_worker = "online";
                        break;
                    }
                }
            }
        }
    }

    // 4. Probe WireGuard VPN interface (wg0) or env flag
    let vpn_active_env = std::env::var("WIREGUARD_ACTIVE")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let vpn_status = if std::path::Path::new("/sys/class/net/wg0").exists() || vpn_active_env {
        "online"
    } else {
        "offline"
    };

    Json(json!({
        "api": "ok",
        "database": db_status,
        "mqtt": mqtt_broker,
        "mqtt_broker": mqtt_broker,
        "esp32": if dev_online > 0 { "online" } else { "offline" },
        "device_online": dev_online,
        "device_total": dev_total,
        "camera_rtsp": if cam_online > 0 { "online" } else { "offline" },
        "camera_online": cam_online,
        "camera_total": cam_total,
        "ai": ai_worker,
        "ai_worker": ai_worker,
        "vpn": vpn_status,
        "wireguard_vpn": vpn_status,
    }))
}
