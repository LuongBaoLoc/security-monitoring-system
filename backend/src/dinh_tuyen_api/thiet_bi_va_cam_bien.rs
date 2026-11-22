//! Thiết bị IoT ESP32, lệnh điều khiển và dữ liệu cảm biến (/api/devices/*, /api/sensors/*).

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use rumqttc::QoS;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    bao_mat::{admin_user, current_user, ApiError},
    co_so_du_lieu::{map_device, map_sensor, AppState},
    dich_vu_nghiep_vu::record_system_log,
    ket_noi_websocket::broadcast,
    khuon_mau_du_lieu::{envelope, CommandCreate, DeviceCreate, SensorCreate},
    mo_hinh_du_lieu::now_iso,
};

use super::*;

pub async fn list_devices(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let rows = sqlx::query("SELECT * FROM devices ORDER BY device_code ASC")
        .fetch_all(&state.db)
        .await?;
    let items: Vec<_> = rows.iter().map(map_device).collect();
    let total = items.len() as i64;
    Ok(Json(
        serde_json::to_value(envelope(items, total, total, 0)).unwrap(),
    ))
}

pub async fn create_device(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(data): Json<DeviceCreate>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let user = admin_user(&headers, &state).await?;
    let code = data.device_code.trim().to_string();
    let existing = sqlx::query("SELECT id FROM devices WHERE device_code = $1")
        .bind(&code)
        .fetch_optional(&state.db)
        .await?;
    if existing.is_some() {
        return Err(ApiError::conflict("Mã thiết bị đã tồn tại"));
    }

    let dev_type = if data.device_type.trim().is_empty() {
        "ESP32".to_string()
    } else {
        data.device_type.trim().to_string()
    };
    let status = if data.status.trim().is_empty() {
        "ONLINE".to_string()
    } else {
        data.status.trim().to_ascii_uppercase()
    };
    let now = now_iso();
    let row = sqlx::query(
        "INSERT INTO devices (device_code, device_name, device_type, status, last_seen, latest_pir, latest_mq2, relay, buzzer, lock_engaged, created_at)
         VALUES ($1, $2, $3, $4, $5, FALSE, 350.0, FALSE, FALSE, TRUE, $6) RETURNING *",
    )
    .bind(&code)
    .bind(data.device_name.trim())
    .bind(&dev_type)
    .bind(&status)
    .bind(&now)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;
    let device = map_device(&row);

    let _ = record_system_log(
        &state.db,
        "DEVICE_CREATE",
        &format!(
            "Thêm thiết bị IoT {} ({})",
            device.device_code, device.device_name
        ),
        Some(user.id),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(device).unwrap()),
    ))
}

pub async fn delete_device(
    headers: HeaderMap,
    Path(device_id): Path<String>,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    let user = admin_user(&headers, &state).await?;
    let row = sqlx::query("SELECT * FROM devices WHERE device_code = $1")
        .bind(&device_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Device not found"));
    };
    let device = map_device(&row);

    sqlx::query("DELETE FROM sensors WHERE device_id = $1")
        .bind(device.id)
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM device_logs WHERE device_id = $1")
        .bind(device.id)
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM devices WHERE id = $1")
        .bind(device.id)
        .execute(&state.db)
        .await?;

    let _ = record_system_log(
        &state.db,
        "DEVICE_DELETE",
        &format!("Xóa thiết bị IoT {}", device.device_code),
        Some(user.id),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_device(
    headers: HeaderMap,
    Path(device_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let row = sqlx::query("SELECT * FROM devices WHERE device_code = $1")
        .bind(&device_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Device not found"));
    };
    Ok(Json(serde_json::to_value(map_device(&row)).unwrap()))
}

pub async fn command_device(
    headers: HeaderMap,
    Path(device_id): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<CommandCreate>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let user = current_user(&headers, &state).await?;
    let allowed = [
        "relay_on",
        "relay_off",
        "buzzer_on",
        "buzzer_off",
        "unlock",
        "lock",
    ];
    if !allowed.contains(&body.action.as_str()) {
        return Err(ApiError::unprocessable("Unsupported command"));
    }

    let row = sqlx::query("SELECT * FROM devices WHERE device_code = $1")
        .bind(&device_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Device not found"));
    };
    let mut device = map_device(&row);
    let command_id = Uuid::new_v4().to_string();
    let now = now_iso();

    match body.action.as_str() {
        "relay_on" => device.relay = Some(true),
        "relay_off" => device.relay = Some(false),
        "buzzer_on" => device.buzzer = Some(true),
        "buzzer_off" => device.buzzer = Some(false),
        "lock" => device.lock_engaged = Some(true),
        "unlock" => device.lock_engaged = Some(false),
        _ => {}
    }
    device.last_seen = Some(now.clone());
    device.status = "ONLINE".to_string();

    sqlx::query(
        "UPDATE devices SET relay = $1, buzzer = $2, lock_engaged = $3, last_seen = $4, status = 'ONLINE' WHERE id = $5",
    )
    .bind(device.relay)
    .bind(device.buzzer)
    .bind(device.lock_engaged)
    .bind(&now)
    .bind(device.id)
    .execute(&state.db)
    .await?;

    let mut mqtt_sent = false;
    if state.settings.mqtt_enabled {
        let guard = state.mqtt_client.read().await;
        if let Some(ref client) = *guard {
            let topic = format!("security/device/{device_id}/command");
            let payload = json!({
                "command_id": command_id,
                "action": body.action,
                "requested_at": now,
                "requested_by": user.username,
            })
            .to_string();
            if client
                .publish(topic, QoS::AtLeastOnce, false, payload)
                .await
                .is_ok()
            {
                mqtt_sent = true;
            }
        }
    }

    let (result_status, completed_at, detail) = if mqtt_sent || !state.settings.mqtt_enabled {
        (
            "ACCEPTED",
            Some(now.clone()),
            json!({"note": if mqtt_sent { "Published via MQTT" } else { "Local state updated" }})
                .to_string(),
        )
    } else {
        (
            "PENDING",
            None,
            json!({"note": "State updated; MQTT broker unreachable"}).to_string(),
        )
    };

    sqlx::query(
        "INSERT INTO device_logs (device_id, command_id, command, result, detail, created_at, completed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(device.id)
    .bind(&command_id)
    .bind(&body.action)
    .bind(result_status)
    .bind(detail)
    .bind(&now)
    .bind(completed_at)
    .execute(&state.db)
    .await?;

    let _ = record_system_log(
        &state.db,
        "DEVICE_COMMAND",
        &format!(
            "Người dùng {} gửi lệnh {} tới {}",
            user.username, body.action, device_id
        ),
        Some(user.id),
    )
    .await;

    broadcast(
        &state,
        "device.status",
        json!({
            "device_code": device.device_code,
            "status": device.status,
            "last_seen": now,
        }),
    );

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "command_id": command_id,
            "status": if mqtt_sent || !state.settings.mqtt_enabled { "accepted" } else { "pending" },
            "device_code": device.device_code,
            "action": body.action,
            "relay": device.relay,
            "buzzer": device.buzzer,
            "lock_engaged": device.lock_engaged,
            "message": format!("Đã thực thi lệnh {} cho thiết bị {}", body.action, device.device_code),
        })),
    ))
}

pub async fn sensor_data(
    State(state): State<AppState>,
    Json(data): Json<SensorCreate>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    data.validate().map_err(ApiError::unprocessable)?;
    let row = sqlx::query("SELECT * FROM devices WHERE device_code = $1")
        .bind(&data.device_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Device not registered"));
    };
    let mut device = map_device(&row);
    let ts = data
        .timestamp
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(now_iso);

    device.status = "ONLINE".to_string();
    device.last_seen = Some(ts.clone());
    if let Some(pir) = data.pir {
        device.latest_pir = Some(pir);
    }
    if let Some(mq2) = data.mq2 {
        device.latest_mq2 = Some(mq2);
    }
    if let Some(relay) = data.relay {
        device.relay = Some(relay);
    }
    if let Some(buzzer) = data.buzzer {
        device.buzzer = Some(buzzer);
    }
    if let Some(lock) = data.lock {
        device.lock_engaged = Some(lock);
    }

    sqlx::query(
        "UPDATE devices SET status = 'ONLINE', last_seen = $1, latest_pir = $2, latest_mq2 = $3, relay = $4, buzzer = $5, lock_engaged = $6 WHERE id = $7",
    )
    .bind(&ts)
    .bind(device.latest_pir)
    .bind(device.latest_mq2)
    .bind(device.relay)
    .bind(device.buzzer)
    .bind(device.lock_engaged)
    .bind(device.id)
    .execute(&state.db)
    .await?;

    let mut sensor_entries: Vec<(&str, Value)> = Vec::new();
    if let Some(v) = data.pir {
        sensor_entries.push(("pir", json!({"value": v})));
    }
    if let Some(v) = data.mq2 {
        sensor_entries.push(("mq2", json!({"value": v})));
    }
    if let Some(v) = data.relay {
        sensor_entries.push(("relay", json!({"value": v})));
    }
    if let Some(v) = data.buzzer {
        sensor_entries.push(("buzzer", json!({"value": v})));
    }
    if let Some(v) = data.lock {
        sensor_entries.push(("lock", json!({"value": v})));
    }

    for (kind, val) in sensor_entries {
        sqlx::query(
            "INSERT INTO sensors (device_id, sensor_type, sensor_value, created_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(device.id)
        .bind(kind)
        .bind(val.to_string())
        .bind(&ts)
        .execute(&state.db)
        .await?;
    }

    let mq2_threshold = state.runtime_settings.read().await.mq2_threshold;
    if let Some(mq2) = data.mq2 {
        if mq2 >= mq2_threshold {
            let al_row = sqlx::query(
                "INSERT INTO alerts (detection_id, alert_type, severity, description, status, created_at)
                 VALUES (NULL, 'gas_mq2', 'CRITICAL', $1, 'OPEN', $2) RETURNING id",
            )
            .bind(format!(
                "BÁO CHÁY / KHÍ GAS: Cảm biến MQ-2 vượt ngưỡng ({:.0} ppm) tại {}",
                mq2, device.device_name
            ))
            .bind(&ts)
            .fetch_one(&state.db)
            .await?;
            let aid = row_i64(&al_row, "id");
            broadcast(
                &state,
                "alert.created",
                json!({
                    "id": aid,
                    "alert_type": "gas_mq2",
                    "severity": "CRITICAL",
                    "status": "OPEN",
                }),
            );
        }
    }

    let _ = record_system_log(
        &state.db,
        "SENSOR_TELEMETRY",
        &format!(
            "Cập nhật cảm biến PCCC từ {} (Flame={:?}, MQ-2={:?} ppm)",
            device.device_code, device.latest_pir, device.latest_mq2
        ),
        None,
    )
    .await;

    broadcast(
        &state,
        "device.status",
        json!({
            "device_code": device.device_code,
            "status": device.status,
            "last_seen": ts,
        }),
    );

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "accepted": true,
            "device_id": device.device_code,
        })),
    ))
}

pub async fn list_sensors(
    headers: HeaderMap,
    Query(q): Query<SensorsQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let limit = q.limit.clamp(1, 500);
    let offset = q.offset.max(0);

    let rows = sqlx::query(
        "SELECT s.*, d.device_code FROM sensors s JOIN devices d ON s.device_id = d.id ORDER BY s.created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;

    let filtered: Vec<_> = rows
        .iter()
        .map(map_sensor)
        .filter(|s| {
            let match_dev = q
                .device_id
                .as_ref()
                .map(|id| &s.device_id == id)
                .unwrap_or(true);
            let match_type = q
                .sensor_type
                .as_ref()
                .map(|t| &s.sensor_type == t)
                .unwrap_or(true);
            match_dev && match_type
        })
        .collect();

    let total = filtered.len() as i64;
    let items: Vec<_> = filtered
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    Ok(Json(
        serde_json::to_value(envelope(items, total, limit, offset)).unwrap(),
    ))
}
