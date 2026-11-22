//! Tiếp nhận, lưu ảnh snapshot và tra cứu sự kiện nhận diện AI YOLOv8 (/api/detections/*).

use std::path::{Component, Path as FsPath, PathBuf};

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    bao_mat::{current_user, is_valid_ai_service_token, optional_user, ApiError},
    co_so_du_lieu::{map_camera, map_detection, row_i64, row_opt_str, row_str, AppState},
    dich_vu_nghiep_vu::record_system_log,
    ket_noi_websocket::broadcast,
    khuon_mau_du_lieu::{envelope, DetectionCreate},
};

use super::*;

const MAX_SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;
const EVENTS_UPLOAD_DIR: &str = "uploads/events";

enum SqlBind {
    Int(i64),
    Float(f64),
    Str(String),
}

fn filter_text(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("all"))
        .map(str::to_string)
}

/// Kiểm tra magic bytes để đảm bảo file tải lên thực sự là ảnh JPEG/PNG/WebP.
fn detect_image_format(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("jpg", "image/jpeg"))
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(("png", "image/png"))
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(("webp", "image/webp"))
    } else {
        None
    }
}

/// Chuẩn hóa và chặn path traversal cho `image_path` trước khi lưu DB / trả về.
fn normalize_safe_image_path(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(val) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let normalized = val.replace('\\', "/");
    let cleaned = normalized.trim_start_matches("./");
    if cleaned.starts_with('/') || cleaned.contains(':') || cleaned.contains('\0') {
        return Err(ApiError::unprocessable(
            "image_path phải là đường dẫn tương đối an toàn (vd: uploads/events/xxx.jpg)",
        ));
    }
    let path = FsPath::new(cleaned);
    for comp in path.components() {
        if !matches!(comp, Component::Normal(_)) {
            return Err(ApiError::unprocessable(
                "image_path không được chứa ký tự điều hướng thư mục (..)",
            ));
        }
    }
    if !cleaned.starts_with("uploads/") {
        return Err(ApiError::unprocessable(
            "image_path phải nằm trong thư mục uploads/",
        ));
    }
    Ok(Some(cleaned.to_string()))
}

fn public_image_url(image_path: Option<&str>) -> Option<String> {
    image_path
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|p| {
            let clean = p.replace('\\', "/");
            if clean.starts_with('/') {
                clean
            } else {
                format!("/{clean}")
            }
        })
}

/// `POST /api/detections/snapshot` — Lưu ảnh snapshot sự kiện vào `uploads/events/`
/// (chống path traversal, chỉ chấp nhận ảnh JPEG/PNG/WebP hợp lệ).
pub async fn upload_detection_snapshot(
    headers: HeaderMap,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let user = optional_user(&headers, &state).await;
    let valid_ai_token = is_valid_ai_service_token(&headers, &state.settings);
    if !valid_ai_token && user.is_none() {
        return Err(ApiError::unauthorized("Invalid AI service token"));
    }

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut camera_id_prefix: Option<i64> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::bad_request("Dữ liệu multipart không hợp lệ"))?
    {
        match field.name() {
            Some("file") => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| ApiError::bad_request("Không thể đọc file ảnh tải lên"))?;
                if bytes.is_empty() {
                    return Err(ApiError::bad_request("File ảnh tải lên bị rỗng"));
                }
                if bytes.len() > MAX_SNAPSHOT_BYTES {
                    return Err(ApiError::new(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "Ảnh snapshot vượt quá giới hạn 8MB",
                    ));
                }
                file_bytes = Some(bytes.to_vec());
            }
            Some("camera_id") => {
                if let Ok(txt) = field.text().await {
                    if let Ok(cid) = txt.trim().parse::<i64>() {
                        if cid > 0 {
                            camera_id_prefix = Some(cid);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let Some(contents) = file_bytes else {
        return Err(ApiError::bad_request(
            "Thiếu trường 'file' trong multipart form",
        ));
    };

    let Some((ext, content_type)) = detect_image_format(&contents) else {
        return Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Chỉ chấp nhận file ảnh JPEG, PNG hoặc WebP hợp lệ",
        ));
    };

    tokio::fs::create_dir_all(EVENTS_UPLOAD_DIR)
        .await
        .map_err(ApiError::internal)?;

    // Tự sinh tên file an toàn (không dùng tên file do client gửi lên -> chống path traversal).
    let ts_tag = Utc::now().format("%Y%m%dT%H%M%SZ");
    let short_id = Uuid::new_v4().simple().to_string();
    let filename = match camera_id_prefix {
        Some(cid) => format!("cam{cid}_{ts_tag}_{}.{}", &short_id[..12], ext),
        None => format!("evt_{ts_tag}_{}.{}", &short_id[..12], ext),
    };

    let base_dir = tokio::fs::canonicalize(EVENTS_UPLOAD_DIR)
        .await
        .map_err(ApiError::internal)?;
    let target_path: PathBuf = base_dir.join(&filename);
    if target_path.parent() != Some(base_dir.as_path()) {
        return Err(ApiError::bad_request("Đường dẫn lưu file không hợp lệ"));
    }

    tokio::fs::write(&target_path, &contents)
        .await
        .map_err(ApiError::internal)?;

    let relative_path = format!("{EVENTS_UPLOAD_DIR}/{filename}");
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "image_path": relative_path,
            "url": format!("/{relative_path}"),
            "size_bytes": contents.len(),
            "content_type": content_type,
        })),
    ))
}

pub async fn create_detection(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(data): Json<DetectionCreate>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    data.validate().map_err(ApiError::unprocessable)?;
    let clean_image_path = normalize_safe_image_path(data.image_path.as_deref())?;

    let user = optional_user(&headers, &state).await;
    let valid_ai_token = is_valid_ai_service_token(&headers, &state.settings);

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
    let object_type = data.object_type.trim().to_string();
    let event_type = data.event_type.trim().to_string();

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
    .bind(&object_type)
    .bind(data.confidence)
    .bind(&event_type)
    .bind(&clean_image_path)
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

    // Quy tắc báo cháy PCCC:
    // - Lửa (`fire` / `flame`) -> CRITICAL (Báo cháy khẩn cấp)
    // - Khói (`smoke`) -> WARNING (tự nâng mức CRITICAL khi độ tin cậy >= 85% hoặc khói đậm)
    let ev_lower = event_type.to_ascii_lowercase();
    let obj_lower = object_type.to_ascii_lowercase();
    let alertable = matches!(
        ev_lower.as_str(),
        "fire" | "smoke" | "flame" | "gas_mq2"
    ) || matches!(obj_lower.as_str(), "fire" | "smoke" | "flame");

    if data.confidence >= conf_threshold && alertable {
        let cutoff = (ts_dt - Duration::seconds(dedup_seconds)).to_rfc3339();
        let dup_row = sqlx::query(
            "SELECT a.id FROM alerts a
             JOIN detection_events d ON a.detection_id = d.id
             WHERE a.status = 'OPEN' AND a.alert_type = $1 AND d.camera_id = $2 AND a.created_at >= $3
             LIMIT 1",
        )
        .bind(&event_type)
        .bind(camera.id)
        .bind(&cutoff)
        .fetch_optional(&state.db)
        .await?;

        if dup_row.is_some() {
            deduplicated = true;
        } else {
            let is_fire = matches!(ev_lower.as_str(), "fire" | "flame")
                || matches!(obj_lower.as_str(), "fire" | "flame");
            let is_high_smoke =
                (ev_lower == "smoke" || obj_lower == "smoke") && data.confidence >= 0.85;
            let sev = if is_fire || is_high_smoke || ev_lower == "gas_mq2" {
                "CRITICAL"
            } else {
                "WARNING"
            };
            let desc = if is_fire {
                format!(
                    "BÁO CHÁY KHẨN CẤP: YOLOv8 phát hiện Lửa ({:.1}%) tại {}",
                    data.confidence * 100.0,
                    camera.name
                )
            } else if ev_lower == "smoke" || obj_lower == "smoke" {
                format!(
                    "{}: YOLOv8 phát hiện Khói ({:.1}%) tại {}",
                    if sev == "CRITICAL" {
                        "CẢNH BÁO CHÁY KHẨN CẤP"
                    } else {
                        "Cảnh báo sớm PCCC"
                    },
                    data.confidence * 100.0,
                    camera.name
                )
            } else {
                format!(
                    "Cảnh báo PCCC: Phát hiện {} ({:.1}%) tại {}",
                    object_type,
                    data.confidence * 100.0,
                    camera.name
                )
            };
            let al_row = sqlx::query(
                "INSERT INTO alerts (detection_id, alert_type, severity, description, status, created_at)
                 VALUES ($1, $2, $3, $4, 'OPEN', $5) RETURNING id",
            )
            .bind(event_id)
            .bind(&event_type)
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
            "Phát hiện {} ({:.1}%) tại {} (camera_id={})",
            object_type,
            data.confidence * 100.0,
            camera.name,
            camera.id
        ),
        user.as_ref().map(|u| u.id),
    )
    .await;

    let result = json!({
        "id": event_id,
        "camera_id": camera.id,
        "camera_name": camera.name,
        "object_type": object_type,
        "confidence": data.confidence,
        "event_type": event_type,
        "image_path": clean_image_path,
        "image_url": public_image_url(clean_image_path.as_deref()),
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
                "detection_id": event_id,
                "camera_id": camera.id,
                "camera_name": camera.name,
                "severity": sev,
                "status": "OPEN",
            }),
        );
    }

    Ok((StatusCode::CREATED, Json(result)))
}

/// `GET /api/detections?camera_id=&object_type=&event_type=&min_confidence=&limit=&offset=`
pub async fn list_detections(
    headers: HeaderMap,
    Query(q): Query<DetectionsQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);

    if let Some(min_c) = q.min_confidence {
        if !(0.0..=1.0).contains(&min_c) {
            return Err(ApiError::unprocessable(
                "min_confidence phải nằm trong khoảng 0.0 đến 1.0",
            ));
        }
    }

    let obj_filter = filter_text(q.object_type.as_deref()).map(|s| s.to_ascii_lowercase());
    let evt_filter = filter_text(q.event_type.as_deref()).map(|s| s.to_ascii_lowercase());

    let mut conditions: Vec<String> = Vec::new();
    let mut binds: Vec<SqlBind> = Vec::new();

    if let Some(cid) = q.camera_id {
        if cid > 0 {
            binds.push(SqlBind::Int(cid));
            conditions.push(format!("d.camera_id = ${}", binds.len()));
        }
    }
    if let Some(ref obj) = obj_filter {
        if obj == "fire" {
            conditions.push("LOWER(d.object_type) IN ('fire', 'flame')".to_string());
        } else if obj == "smoke" {
            conditions.push("LOWER(d.object_type) = 'smoke'".to_string());
        } else if obj == "other" {
            conditions.push("LOWER(d.object_type) NOT IN ('fire', 'flame', 'smoke')".to_string());
        } else {
            binds.push(SqlBind::Str(obj.clone()));
            conditions.push(format!("LOWER(d.object_type) = ${}", binds.len()));
        }
    }
    if let Some(ref evt) = evt_filter {
        if evt == "fire" {
            conditions.push("LOWER(d.event_type) IN ('fire', 'flame')".to_string());
        } else if evt == "smoke" {
            conditions.push("LOWER(d.event_type) = 'smoke'".to_string());
        } else {
            binds.push(SqlBind::Str(evt.clone()));
            conditions.push(format!("LOWER(d.event_type) = ${}", binds.len()));
        }
    }
    if let Some(min_c) = q.min_confidence {
        binds.push(SqlBind::Float(min_c));
        conditions.push(format!("d.confidence >= ${}", binds.len()));
    }

    let where_sql = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(d.id) AS cnt FROM detection_events d{where_sql}");
    let mut count_q = sqlx::query(&count_sql);
    for b in &binds {
        count_q = match b {
            SqlBind::Int(v) => count_q.bind(*v),
            SqlBind::Float(v) => count_q.bind(*v),
            SqlBind::Str(v) => count_q.bind(v.clone()),
        };
    }
    let total = row_i64(&count_q.fetch_one(&state.db).await?, "cnt");

    let list_sql = format!(
        "SELECT d.*, c.name AS camera_name
         FROM detection_events d
         LEFT JOIN cameras c ON d.camera_id = c.id
         {where_sql}
         ORDER BY d.event_timestamp DESC, d.id DESC
         LIMIT ${} OFFSET ${}",
        binds.len() + 1,
        binds.len() + 2
    );
    let mut list_q = sqlx::query(&list_sql);
    for b in &binds {
        list_q = match b {
            SqlBind::Int(v) => list_q.bind(*v),
            SqlBind::Float(v) => list_q.bind(*v),
            SqlBind::Str(v) => list_q.bind(v.clone()),
        };
    }
    let rows = list_q
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?;

    let items: Vec<_> = rows.iter().map(map_detection).collect();
    let mut body = serde_json::to_value(envelope(items, total, limit, offset))
        .map_err(ApiError::internal)?;
    body["filters"] = json!({
        "camera_id": q.camera_id,
        "object_type": obj_filter,
        "event_type": evt_filter,
        "min_confidence": q.min_confidence,
    });
    Ok(Json(body))
}

/// `GET /api/detections/:event_id` — JOIN `cameras` để trả kèm `camera_name`, vị trí & cảnh báo liên quan.
pub async fn detection_detail(
    headers: HeaderMap,
    Path(event_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let row = sqlx::query(
        "SELECT d.*, c.name AS camera_name, c.location AS camera_location, c.status AS camera_status
         FROM detection_events d
         LEFT JOIN cameras c ON d.camera_id = c.id
         WHERE d.id = $1",
    )
    .bind(event_id)
    .fetch_optional(&state.db)
    .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Detection not found"));
    };

    let det = map_detection(&row);
    let camera_location = row_opt_str(&row, "camera_location");
    let camera_status = row_opt_str(&row, "camera_status");
    let created_at = row_opt_str(&row, "created_at");

    let alert_rows = sqlx::query(
        "SELECT id, alert_type, severity, description, status, created_at
         FROM alerts
         WHERE detection_id = $1
         ORDER BY id DESC",
    )
    .bind(event_id)
    .fetch_all(&state.db)
    .await?;
    let alerts: Vec<Value> = alert_rows
        .iter()
        .map(|r| {
            json!({
                "id": row_i64(r, "id"),
                "alert_type": row_str(r, "alert_type"),
                "severity": row_str(r, "severity"),
                "description": row_str(r, "description"),
                "status": row_str(r, "status"),
                "created_at": row_str(r, "created_at"),
            })
        })
        .collect();

    Ok(Json(json!({
        "id": det.id,
        "camera_id": det.camera_id,
        "camera_name": det.camera_name,
        "camera_location": camera_location,
        "camera_status": camera_status,
        "object_type": det.object_type,
        "confidence": det.confidence,
        "event_type": det.event_type,
        "image_path": det.image_path,
        "image_url": public_image_url(det.image_path.as_deref()),
        "timestamp": det.timestamp,
        "created_at": created_at,
        "alerts": alerts,
    })))
}
