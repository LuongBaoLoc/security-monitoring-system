//! Xem, Thêm, Sửa, Xóa Camera (/api/cameras/*).

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::Value;

use crate::{
    bao_mat::{admin_user, current_user, ApiError},
    co_so_du_lieu::{map_camera, row_i64, AppState},
    dich_vu_nghiep_vu::{camera_json, record_system_log},
    khuon_mau_du_lieu::{envelope, CameraCreate, CameraUpdate},
    mo_hinh_du_lieu::now_iso,
};

use super::*;

pub async fn list_cameras(
    headers: HeaderMap,
    Query(q): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);
    let total_row = sqlx::query("SELECT COUNT(id) AS cnt FROM cameras")
        .fetch_one(&state.db)
        .await?;
    let total = row_i64(&total_row, "cnt");

    let rows = sqlx::query("SELECT * FROM cameras ORDER BY id ASC LIMIT $1 OFFSET $2")
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?;
    let items: Vec<Value> = rows.iter().map(|r| camera_json(&map_camera(r))).collect();
    Ok(Json(
        serde_json::to_value(envelope(items, total, limit, offset)).unwrap(),
    ))
}

pub async fn create_camera(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(data): Json<CameraCreate>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let user = admin_user(&headers, &state).await?;
    let status = if data.status.trim().is_empty() {
        "ONLINE".to_string()
    } else {
        data.status.trim().to_ascii_uppercase()
    };
    let now = now_iso();
    let row = sqlx::query(
        "INSERT INTO cameras (name, ip_address, rtsp_url, location, status, last_seen, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *",
    )
    .bind(data.name.trim())
    .bind(&data.ip_address)
    .bind(data.rtsp_url.trim())
    .bind(&data.location)
    .bind(&status)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;
    let cam = map_camera(&row);

    let _ = record_system_log(
        &state.db,
        "CAMERA_CREATE",
        &format!(
            "Thêm camera {} ({})",
            cam.name,
            cam.location.as_deref().unwrap_or("Chưa rõ vị trí")
        ),
        Some(user.id),
    )
    .await;

    Ok((StatusCode::CREATED, Json(camera_json(&cam))))
}

pub async fn get_camera(
    headers: HeaderMap,
    Path(camera_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let row = sqlx::query("SELECT * FROM cameras WHERE id = $1")
        .bind(camera_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Camera not found"));
    };
    Ok(Json(camera_json(&map_camera(&row))))
}

pub async fn update_camera(
    headers: HeaderMap,
    Path(camera_id): Path<i64>,
    State(state): State<AppState>,
    Json(data): Json<CameraUpdate>,
) -> Result<Json<Value>, ApiError> {
    let user = admin_user(&headers, &state).await?;
    let row = sqlx::query("SELECT * FROM cameras WHERE id = $1")
        .bind(camera_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Camera not found"));
    };
    let mut cam = map_camera(&row);
    cam.name = data.name.trim().to_string();
    cam.ip_address = data.ip_address;
    if let Some(ref rtsp) = data.rtsp_url {
        if !rtsp.trim().is_empty() {
            cam.rtsp_url = rtsp.trim().to_string();
        }
    }
    cam.location = data.location;
    if !data.status.trim().is_empty() {
        cam.status = data.status.trim().to_ascii_uppercase();
    }
    cam.updated_at = now_iso();

    sqlx::query(
        "UPDATE cameras SET name = $1, ip_address = $2, rtsp_url = $3, location = $4, status = $5, updated_at = $6 WHERE id = $7",
    )
    .bind(&cam.name)
    .bind(&cam.ip_address)
    .bind(&cam.rtsp_url)
    .bind(&cam.location)
    .bind(&cam.status)
    .bind(&cam.updated_at)
    .bind(cam.id)
    .execute(&state.db)
    .await?;

    let _ = record_system_log(
        &state.db,
        "CAMERA_UPDATE",
        &format!("Cập nhật camera {} ({})", cam.name, cam.status),
        Some(user.id),
    )
    .await;

    Ok(Json(camera_json(&cam)))
}

pub async fn delete_camera(
    headers: HeaderMap,
    Path(camera_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    let user = admin_user(&headers, &state).await?;
    let row = sqlx::query("SELECT * FROM cameras WHERE id = $1")
        .bind(camera_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Camera not found"));
    };
    let cam = map_camera(&row);

    sqlx::query(
        "DELETE FROM alerts WHERE detection_id IN (SELECT id FROM detection_events WHERE camera_id = $1)",
    )
    .bind(camera_id)
    .execute(&state.db)
    .await?;
    sqlx::query("DELETE FROM detection_events WHERE camera_id = $1")
        .bind(camera_id)
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM cameras WHERE id = $1")
        .bind(camera_id)
        .execute(&state.db)
        .await?;

    let _ = record_system_log(
        &state.db,
        "CAMERA_DELETE",
        &format!("Xóa camera {}", cam.name),
        Some(user.id),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}
