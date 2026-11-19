//! Tiếp nhận và tra cứu sự kiện nhận diện AI YOLOv8 (/api/detections/*).

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    Json,
};
use chrono::{Duration, Utc};
use serde_json::{json, Value};

use crate::{
    bao_mat::{current_user, optional_user, ApiError},
    co_so_du_lieu::{map_camera, map_detection, row_i64, AppState},
    dich_vu_nghiep_vu::record_system_log,
    ket_noi_websocket::broadcast,
    khuon_mau_du_lieu::{envelope, DetectionCreate},
};

use super::*;

pub async fn create_detection(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(data): Json<DetectionCreate>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    data.validate().map_err(ApiError::unprocessable)?;

    let user = optional_user(&headers, &state).await;
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let ai_token = &state.settings.ai_service_token;
    let valid_ai_token = !ai_token.is_empty() && auth_header == format!("Bearer {ai_token}");

    if !valid_ai_token && user.is_none() {
        return Err(ApiError::unauthorized("Invalid AI service token"));
    }

    let cam_row = sqlx::query("SELECT * FROM cameras WHERE id = $1")
        .bind(data.camera_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(cam_row) = cam_row else {
        return Err(ApiError::not_found("Camera not registered"));
    };
    let camera = map_camera(&cam_row);

    let ts_dt = data.timestamp.unwrap_or_else(Utc::now);
    let timestamp = ts_dt.to_rfc3339();

    sqlx::query("UPDATE cameras SET last_seen = $1, status = 'ONLINE' WHERE id = $2")
        .bind(&timestamp)
        .bind(camera.id)
        .execute(&state.db)
        .await?;

    let ev_row = sqlx::query(
        "INSERT INTO detection_events (camera_id, object_type, confidence, event_type, image_path, event_timestamp, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(camera.id)
    .bind(&data.object_type)
    .bind(data.confidence)
    .bind(&data.event_type)
    .bind(&data.image_path)
    .bind(&timestamp)
    .bind(&timestamp)
    .fetch_one(&state.db)
    .await?;
    let event_id = row_i64(&ev_row, "id");

    let (conf_threshold, dedup_seconds) = {
        let rt = state.runtime_settings.read().await;
        (
            rt.alert_confidence_threshold,
            rt.alert_deduplication_seconds,
        )
    };

    let mut alert_id: Option<i64> = None;
    let mut alert_severity: Option<String> = None;
    let mut deduplicated = false;

    let ev_lower = data.event_type.to_ascii_lowercase();
    let alertable = matches!(
        ev_lower.as_str(),
        "intrusion" | "person" | "weapon" | "fire" | "smoke"
    );

    if data.confidence >= conf_threshold && alertable {
        let cutoff = (ts_dt - Duration::seconds(dedup_seconds)).to_rfc3339();
        let dup_row = sqlx::query(
            "SELECT a.id FROM alerts a
             JOIN detection_events d ON a.detection_id = d.id
             WHERE a.status = 'OPEN' AND a.alert_type = $1 AND d.camera_id = $2 AND a.created_at >= $3
             LIMIT 1",
        )
        .bind(&data.event_type)
        .bind(camera.id)
        .bind(&cutoff)
        .fetch_optional(&state.db)
        .await?;

        if dup_row.is_some() {
            deduplicated = true;
        } else {
            let sev = if matches!(ev_lower.as_str(), "intrusion" | "weapon" | "fire") {
                "CRITICAL"
            } else {
                "WARNING"
            };
            let desc = format!(
                "Phát hiện {} ({}) tại {}",
                data.object_type, data.event_type, camera.name
            );
            let al_row = sqlx::query(
                "INSERT INTO alerts (detection_id, alert_type, severity, description, status, created_at)
                 VALUES ($1, $2, $3, $4, 'OPEN', $5) RETURNING id",
            )
            .bind(event_id)
            .bind(&data.event_type)
            .bind(sev)
            .bind(&desc)
            .bind(&timestamp)
            .fetch_one(&state.db)
            .await?;
            alert_id = Some(row_i64(&al_row, "id"));
            alert_severity = Some(sev.to_string());
        }
    }

    let _ = record_system_log(
        &state.db,
        "AI_DETECTION",
        &format!(
            "Phát hiện {} ({:.1}%) tại {}",
            data.object_type,
            data.confidence * 100.0,
            camera.name
        ),
        user.as_ref().map(|u| u.id),
    )
    .await;

    let result = json!({
        "id": event_id,
        "camera_id": camera.id,
        "object_type": data.object_type,
        "confidence": data.confidence,
        "event_type": data.event_type,
        "timestamp": timestamp,
        "alert_id": alert_id,
        "alert_severity": alert_severity,
        "deduplicated": deduplicated,
    });

    broadcast(&state, "detection.created", result.clone());
    if let (Some(aid), Some(ref sev)) = (alert_id, alert_severity) {
        broadcast(
            &state,
            "alert.created",
            json!({
                "id": aid,
                "severity": sev,
                "status": "OPEN",
            }),
        );
    }

    Ok((StatusCode::CREATED, Json(result)))
}

pub async fn list_detections(
    headers: HeaderMap,
    Query(q): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);
    let total_row = sqlx::query("SELECT COUNT(id) AS cnt FROM detection_events")
        .fetch_one(&state.db)
        .await?;
    let total = row_i64(&total_row, "cnt");

    let rows = sqlx::query(
        "SELECT d.*, c.name AS camera_name
         FROM detection_events d
         JOIN cameras c ON d.camera_id = c.id
         ORDER BY d.event_timestamp DESC
         LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let items: Vec<_> = rows.iter().map(map_detection).collect();
    Ok(Json(
        serde_json::to_value(envelope(items, total, limit, offset)).unwrap(),
    ))
}

pub async fn detection_detail(
    headers: HeaderMap,
    Path(event_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let row = sqlx::query("SELECT * FROM detection_events WHERE id = $1")
        .bind(event_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Detection not found"));
    };
    Ok(Json(serde_json::to_value(map_detection(&row)).unwrap()))
}
