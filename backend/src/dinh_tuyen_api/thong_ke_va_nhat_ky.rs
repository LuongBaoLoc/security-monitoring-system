//! Số liệu Tổng quan, biểu đồ và nhật ký (/api/statistics/*, /api/logs/*).

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{Duration, Timelike, Utc};
use serde_json::{json, Value};

use crate::{
    bao_mat::{admin_user, current_user, ApiError},
    co_so_du_lieu::{map_device_log, map_system_log, row_i64, row_str, AppState},
    khuon_mau_du_lieu::envelope,
};

use super::*;

pub async fn system_logs(
    headers: HeaderMap,
    Query(q): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);
    let total_row = sqlx::query("SELECT COUNT(id) AS cnt FROM system_logs")
        .fetch_one(&state.db)
        .await?;
    let total = row_i64(&total_row, "cnt");

    let rows = sqlx::query("SELECT * FROM system_logs ORDER BY created_at DESC LIMIT $1 OFFSET $2")
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?;
    let items: Vec<_> = rows.iter().map(map_system_log).collect();
    Ok(Json(
        serde_json::to_value(envelope(items, total, limit, offset)).unwrap(),
    ))
}

pub async fn clear_system_logs(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    let _ = admin_user(&headers, &state).await?;
    sqlx::query("DELETE FROM system_logs")
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM device_logs")
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn device_logs(
    headers: HeaderMap,
    Query(q): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);
    let total_row = sqlx::query("SELECT COUNT(id) AS cnt FROM device_logs")
        .fetch_one(&state.db)
        .await?;
    let total = row_i64(&total_row, "cnt");

    let rows = sqlx::query("SELECT * FROM device_logs ORDER BY created_at DESC LIMIT $1 OFFSET $2")
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?;
    let items: Vec<_> = rows.iter().map(map_device_log).collect();
    Ok(Json(
        serde_json::to_value(envelope(items, total, limit, offset)).unwrap(),
    ))
}

pub async fn statistics_overview(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let today = Utc::now()
        .with_hour(0)
        .unwrap()
        .with_minute(0)
        .unwrap()
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap()
        .to_rfc3339();

    let camera_total = count_query(&state.db, "SELECT COUNT(id) AS cnt FROM cameras", None).await?;
    let camera_online = count_query(
        &state.db,
        "SELECT COUNT(id) AS cnt FROM cameras WHERE status = 'ONLINE'",
        None,
    )
    .await?;
    let device_total = count_query(&state.db, "SELECT COUNT(id) AS cnt FROM devices", None).await?;
    let device_online = count_query(
        &state.db,
        "SELECT COUNT(id) AS cnt FROM devices WHERE status = 'ONLINE'",
        None,
    )
    .await?;
    let detections_today = count_query(
        &state.db,
        "SELECT COUNT(id) AS cnt FROM detection_events WHERE event_timestamp >= $1",
        Some(&today),
    )
    .await?;
    let alerts_today = count_query(
        &state.db,
        "SELECT COUNT(id) AS cnt FROM alerts WHERE created_at >= $1",
        Some(&today),
    )
    .await?;
    let alerts_open = count_query(
        &state.db,
        "SELECT COUNT(id) AS cnt FROM alerts WHERE status = 'OPEN'",
        None,
    )
    .await?;
    let critical_open = count_query(
        &state.db,
        "SELECT COUNT(id) AS cnt FROM alerts WHERE status = 'OPEN' AND severity = 'CRITICAL'",
        None,
    )
    .await?;

    Ok(Json(json!({
        "camera_total": camera_total,
        "camera_online": camera_online,
        "device_total": device_total,
        "device_online": device_online,
        "detections_today": detections_today,
        "alerts_today": alerts_today,
        "alerts_open": alerts_open,
        "critical_open": critical_open,
    })))
}

pub async fn statistics_alerts(
    headers: HeaderMap,
    Query(q): Query<DaysQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let days = q.days.clamp(1, 90);
    let start = (Utc::now() - Duration::days(days)).to_rfc3339();
    let rows = sqlx::query(
        "SELECT SUBSTR(created_at, 1, 10) AS day, COUNT(id) AS cnt
         FROM alerts
         WHERE created_at >= $1
         GROUP BY SUBSTR(created_at, 1, 10)
         ORDER BY day ASC",
    )
    .bind(&start)
    .fetch_all(&state.db)
    .await?;

    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "date": row_str(r, "day"),
                "count": row_i64(r, "cnt"),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items })))
}

pub async fn statistics_detections(
    headers: HeaderMap,
    Query(q): Query<DaysQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let days = q.days.clamp(1, 90);
    let start = (Utc::now() - Duration::days(days)).to_rfc3339();
    let rows = sqlx::query(
        "SELECT object_type, COUNT(id) AS cnt
         FROM detection_events
         WHERE event_timestamp >= $1
         GROUP BY object_type
         ORDER BY cnt DESC",
    )
    .bind(&start)
    .fetch_all(&state.db)
    .await?;

    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "object_type": row_str(r, "object_type"),
                "count": row_i64(r, "cnt"),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items })))
}
