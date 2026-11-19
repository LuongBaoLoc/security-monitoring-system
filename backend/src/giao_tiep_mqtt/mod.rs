//! 8. GIAO TIẾP MQTT — Lắng nghe telemetry/status/ack từ ESP32 và phát lệnh điều khiển.

use chrono::{DateTime, Utc};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{error, info, warn};

use crate::{
    co_so_du_lieu::{map_device, map_device_log, AppState},
    ket_noi_websocket::broadcast,
    mo_hinh_du_lieu::now_iso,
};

pub async fn start_mqtt(state: AppState) {
    if !state.settings.mqtt_enabled {
        return;
    }

    let mut mqttoptions = MqttOptions::new(
        "security-monitor-backend",
        &state.settings.mqtt_host,
        state.settings.mqtt_port,
    );
    mqttoptions.set_keep_alive(Duration::from_secs(30));
    if !state.settings.mqtt_username.is_empty() {
        mqttoptions.set_credentials(&state.settings.mqtt_username, &state.settings.mqtt_password);
    }

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 64);
    {
        let mut guard = state.mqtt_client.write().await;
        *guard = Some(client.clone());
    }

    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    info!(
                        "Connected to MQTT broker {}:{}",
                        state.settings.mqtt_host, state.settings.mqtt_port
                    );
                    let _ = client
                        .subscribe("security/device/+/sensor", QoS::AtLeastOnce)
                        .await;
                    let _ = client
                        .subscribe("security/device/+/status", QoS::AtLeastOnce)
                        .await;
                    let _ = client
                        .subscribe("security/device/+/ack", QoS::AtLeastOnce)
                        .await;
                }
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    if let Err(err) =
                        handle_mqtt_message(&state, &publish.topic, &publish.payload).await
                    {
                        warn!(
                            "Rejected or failed MQTT message on {}: {}",
                            publish.topic, err
                        );
                    }
                }
                Ok(_) => {}
                Err(err) => {
                    error!("MQTT eventloop error: {}", err);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });
}

fn parse_timestamp(val: Option<&Value>) -> String {
    if let Some(Value::String(s)) = val {
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return dt.with_timezone(&Utc).to_rfc3339();
        }
    }
    now_iso()
}

async fn handle_mqtt_message(
    state: &AppState,
    topic: &str,
    payload_bytes: &[u8],
) -> Result<(), String> {
    let parts: Vec<&str> = topic.split('/').collect();
    if parts.len() != 4 || parts[0] != "security" || parts[1] != "device" {
        return Ok(());
    }
    let device_code = parts[2];
    let kind = parts[3];

    let payload: Value =
        serde_json::from_slice(payload_bytes).map_err(|e| format!("Invalid JSON payload: {e}"))?;

    if let Some(Value::String(payload_dev_id)) = payload.get("device_id") {
        if payload_dev_id != device_code {
            warn!("Ignoring MQTT payload with mismatched device_id");
            return Ok(());
        }
    }

    let dev_row = sqlx::query("SELECT * FROM devices WHERE device_code = $1")
        .bind(device_code)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    let Some(dev_row) = dev_row else {
        warn!(
            "Ignoring MQTT message from unregistered device {}",
            device_code
        );
        return Ok(());
    };
    let mut device = map_device(&dev_row);
    let now = parse_timestamp(payload.get("timestamp"));
    device.last_seen = Some(now.clone());
    device.status = "ONLINE".to_string();

    match kind {
        "status" => {
            if let Some(st) = payload.get("status").and_then(|v| v.as_str()) {
                let lower = st.to_ascii_lowercase();
                if lower == "offline" || lower == "online" {
                    device.status = lower.to_ascii_uppercase();
                }
            }
            sqlx::query("UPDATE devices SET status = $1, last_seen = $2 WHERE id = $3")
                .bind(&device.status)
                .bind(&now)
                .bind(device.id)
                .execute(&state.db)
                .await
                .map_err(|e| e.to_string())?;
        }
        "sensor" => {
            if let Some(p) = payload.get("pir") {
                let Some(b) = p.as_bool() else {
                    return Ok(());
                };
                device.latest_pir = Some(b);
            }
            if let Some(m) = payload.get("mq2") {
                let Some(f) = m.as_f64() else {
                    return Ok(());
                };
                if !(0.0..=100_000.0).contains(&f) {
                    return Ok(());
                }
                device.latest_mq2 = Some(f);
            }
            if let Some(r) = payload.get("relay") {
                let Some(b) = r.as_bool() else {
                    return Ok(());
                };
                device.relay = Some(b);
            }
            if let Some(bz) = payload.get("buzzer") {
                let Some(b) = bz.as_bool() else {
                    return Ok(());
                };
                device.buzzer = Some(b);
            }
            if let Some(lk) = payload.get("lock") {
                let Some(b) = lk.as_bool() else {
                    return Ok(());
                };
                device.lock_engaged = Some(b);
            }

            sqlx::query(
                "UPDATE devices SET status = $1, last_seen = $2, latest_pir = $3, latest_mq2 = $4, relay = $5, buzzer = $6, lock_engaged = $7 WHERE id = $8",
            )
            .bind(&device.status)
            .bind(&now)
            .bind(device.latest_pir)
            .bind(device.latest_mq2)
            .bind(device.relay)
            .bind(device.buzzer)
            .bind(device.lock_engaged)
            .bind(device.id)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;

            for key in ["pir", "mq2", "relay", "buzzer", "lock"] {
                if let Some(val) = payload.get(key) {
                    if !val.is_null() {
                        sqlx::query(
                            "INSERT INTO sensors (device_id, sensor_type, sensor_value, created_at) VALUES ($1, $2, $3, $4)",
                        )
                        .bind(device.id)
                        .bind(key)
                        .bind(json!({"value": val}).to_string())
                        .bind(&now)
                        .execute(&state.db)
                        .await
                        .map_err(|e| e.to_string())?;
                    }
                }
            }
        }
        "ack" => {
            let Some(command_id) = payload.get("command_id").and_then(|v| v.as_str()) else {
                return Ok(());
            };
            let log_row =
                sqlx::query("SELECT * FROM device_logs WHERE command_id = $1 AND device_id = $2")
                    .bind(command_id)
                    .bind(device.id)
                    .fetch_optional(&state.db)
                    .await
                    .map_err(|e| e.to_string())?;

            let Some(log_row) = log_row else {
                return Ok(());
            };
            let log = map_device_log(&log_row);
            if log.result != "PENDING" {
                return Ok(());
            }
            let status_str = payload
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let new_result = if status_str == "accepted" {
                "ACCEPTED"
            } else {
                "REJECTED"
            };

            sqlx::query(
                "UPDATE device_logs SET result = $1, completed_at = $2, detail = $3 WHERE id = $4",
            )
            .bind(new_result)
            .bind(&now)
            .bind(payload.to_string())
            .bind(log.id)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;

            sqlx::query("UPDATE devices SET status = 'ONLINE', last_seen = $1 WHERE id = $2")
                .bind(&now)
                .bind(device.id)
                .execute(&state.db)
                .await
                .map_err(|e| e.to_string())?;

            broadcast(
                state,
                "device.command.updated",
                json!({
                    "command_id": command_id,
                    "status": new_result.to_ascii_lowercase(),
                }),
            );
        }
        _ => return Ok(()),
    }

    broadcast(
        state,
        "device.status",
        json!({
            "device_code": device_code,
            "status": device.status,
            "last_seen": now,
        }),
    );

    Ok(())
}
