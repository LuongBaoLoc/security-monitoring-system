//! Danh sách cảnh báo và xử lý cảnh báo (/api/alerts/*).

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use serde_json::{json, Value};

use crate::{
    bao_mat::{current_user, ApiError},
    co_so_du_lieu::{map_alert, AppState},
    dich_vu_nghiep_vu::record_system_log,
    ket_noi_websocket::broadcast,
    khuon_mau_du_lieu::envelope,
    mo_hinh_du_lieu::now_iso,
};

use super::*;

pub async fn list_alerts(
    headers: HeaderMap,
    Query(q): Query<AlertsQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);

    let rows = sqlx::query(
        "SELECT a.*, u.full_name AS handled_by_name
         FROM alerts a
         LEFT JOIN users u ON a.handled_by = u.id
         ORDER BY a.created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;

    let status_filter = q.status.as_ref().map(|s| s.to_ascii_uppercase());
    let sev_filter = q.severity.as_ref().map(|s| s.to_ascii_uppercase());

    let filtered: Vec<_> = rows
        .iter()
        .map(map_alert)
        .filter(|a| {
            let match_st = status_filter
                .as_ref()
                .map(|st| &a.status == st)
                .unwrap_or(true);
            let match_sev = sev_filter
                .as_ref()
                .map(|sv| &a.severity == sv)
                .unwrap_or(true);
            match_st && match_sev
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

pub async fn alert_detail(
    headers: HeaderMap,
    Path(alert_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let row = sqlx::query("SELECT * FROM alerts WHERE id = $1")
        .bind(alert_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Alert not found"));
    };
    Ok(Json(serde_json::to_value(map_alert(&row)).unwrap()))
}

pub async fn resolve_alert(
    headers: HeaderMap,
    Path(alert_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let user = current_user(&headers, &state).await?;
    let row = sqlx::query("SELECT * FROM alerts WHERE id = $1")
        .bind(alert_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Alert not found"));
    };
    let alert = map_alert(&row);
    if alert.status != "OPEN" {
        return Err(ApiError::bad_request("Alert is already resolved"));
    }

    let now = now_iso();
    sqlx::query(
        "UPDATE alerts SET status = 'RESOLVED', handled_by = $1, handled_at = $2 WHERE id = $3",
    )
    .bind(user.id)
    .bind(&now)
    .bind(alert_id)
    .execute(&state.db)
    .await?;

    let _ = record_system_log(
        &state.db,
        "ALERT_RESOLVED",
        &format!("{} đã xử lý cảnh báo #ALT-{:04}", user.full_name, alert_id),
        Some(user.id),
    )
    .await;

    broadcast(
        &state,
        "alert.resolved",
        json!({
            "id": alert_id,
            "status": "RESOLVED",
        }),
    );

    Ok(Json(json!({
        "id": alert_id,
        "status": "RESOLVED",
        "handled_by": user.id,
        "handled_by_name": user.full_name,
        "handled_at": now,
    })))
}
