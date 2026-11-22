//! Xem, Thêm, Sửa, Xóa Camera (/api/cameras/*) và mapping nội bộ cho AI Service
//! (/api/internal/cameras/mappings).
//!
//! Nguyên tắc bảo mật RTSP:
//! - RTSP URL gốc (có user/password) chỉ nằm trong DB và chỉ trả về cho AI Service qua
//!   endpoint nội bộ có `AI_SERVICE_TOKEN`.
//! - Mọi API cho Frontend đều trả `rtsp_url` đã che thông tin đăng nhập (`camera_json`).
//! - Khi sửa camera mà bỏ trống RTSP URL (hoặc gửi lại đúng bản đã che) thì giữ nguyên URL gốc.

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};

use crate::{
    bao_mat::{admin_user, current_user, require_ai_service_token, ApiError},
    co_so_du_lieu::{map_camera, row_i64, row_opt_str, AppState},
    dich_vu_nghiep_vu::{camera_json, record_system_log, redact_rtsp_url},
    khuon_mau_du_lieu::{envelope, normalize_camera_status, CameraCreate, CameraUpdate},
    mo_hinh_du_lieu::now_iso,
};

use super::*;

/// Escape ký tự đặc biệt của LIKE (`\`, `%`, `_`) để người dùng không chèn wildcard.
fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    for c in input.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Lấy giá trị bộ lọc: bỏ khoảng trắng, coi rỗng / `all` là không lọc.
fn filter_value(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("all"))
        .map(str::to_string)
}

/// So sánh RTSP URL bỏ qua dấu `/` cuối.
fn same_rtsp(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

async fn ensure_rtsp_unique(
    state: &AppState,
    rtsp_url: &str,
    exclude_id: Option<i64>,
) -> Result<(), ApiError> {
    let rows = sqlx::query("SELECT id, rtsp_url FROM cameras")
        .fetch_all(&state.db)
        .await?;
    for row in &rows {
        let id = row_i64(row, "id");
        if Some(id) == exclude_id {
            continue;
        }
        let existing = row_opt_str(row, "rtsp_url").unwrap_or_default();
        if same_rtsp(&existing, rtsp_url) {
            return Err(ApiError::conflict(format!(
                "RTSP URL này đã được gán cho camera_id = {id}"
            )));
        }
    }
    Ok(())
}

/// `GET /api/cameras?search=&location=&status=&limit=&offset=`
pub async fn list_cameras(
    headers: HeaderMap,
    Query(q): Query<CameraListQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = current_user(&headers, &state).await?;
    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);

    let search = filter_value(q.search.as_deref());
    let location = filter_value(q.location.as_deref());
    let status = match filter_value(q.status.as_deref()) {
        Some(st) => Some(normalize_camera_status(&st).map_err(ApiError::unprocessable)?),
        None => None,
    };
    if search.as_ref().is_some_and(|s| s.chars().count() > 100) {
        return Err(ApiError::unprocessable("Từ khóa tìm kiếm tối đa 100 ký tự"));
    }
    if location.as_ref().is_some_and(|s| s.chars().count() > 120) {
        return Err(ApiError::unprocessable("Khu vực lọc tối đa 120 ký tự"));
    }

    let mut conditions: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    if let Some(ref s) = search {
        binds.push(format!("%{}%", escape_like(&s.to_lowercase())));
        let n = binds.len();
        // Không tìm trong rtsp_url để tránh dò user/password qua chức năng tìm kiếm.
        conditions.push(format!(
            "(LOWER(name) LIKE ${n} ESCAPE '\\' \
             OR LOWER(COALESCE(location, '')) LIKE ${n} ESCAPE '\\' \
             OR LOWER(COALESCE(ip_address, '')) LIKE ${n} ESCAPE '\\')"
        ));
    }
    if let Some(ref loc) = location {
        binds.push(loc.to_lowercase());
        conditions.push(format!("LOWER(COALESCE(location, '')) = ${}", binds.len()));
    }
    if let Some(ref st) = status {
        binds.push(st.clone());
        conditions.push(format!("UPPER(status) = ${}", binds.len()));
    }
    let where_sql = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(id) AS cnt FROM cameras{where_sql}");
    let mut count_query = sqlx::query(&count_sql);
    for value in &binds {
        count_query = count_query.bind(value.clone());
    }
    let total = row_i64(&count_query.fetch_one(&state.db).await?, "cnt");

    let list_sql = format!(
        "SELECT * FROM cameras{where_sql} ORDER BY id ASC LIMIT ${} OFFSET ${}",
        binds.len() + 1,
        binds.len() + 2
    );
    let mut list_query = sqlx::query(&list_sql);
    for value in &binds {
        list_query = list_query.bind(value.clone());
    }
    let rows = list_query
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?;
    let items: Vec<Value> = rows.iter().map(|r| camera_json(&map_camera(r))).collect();

    // Danh sách khu vực có trong DB để Frontend dựng ô lọc "Khu vực".
    let loc_rows = sqlx::query(
        "SELECT DISTINCT location FROM cameras WHERE location IS NOT NULL AND location <> '' ORDER BY location",
    )
    .fetch_all(&state.db)
    .await?;
    let locations: Vec<String> = loc_rows
        .iter()
        .filter_map(|r| row_opt_str(r, "location"))
        .collect();

    let mut body = serde_json::to_value(envelope(items, total, limit, offset))
        .map_err(ApiError::internal)?;
    body["filters"] = json!({
        "search": search,
        "location": location,
        "status": status,
    });
    body["locations"] = json!(locations);
    Ok(Json(body))
}

pub async fn create_camera(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(data): Json<CameraCreate>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let user = admin_user(&headers, &state).await?;
    let fields = data.validate().map_err(ApiError::unprocessable)?;
    let rtsp_url = fields.rtsp_url.unwrap_or_default();
    let status = fields.status.unwrap_or_else(|| "OFFLINE".to_string());
    ensure_rtsp_unique(&state, &rtsp_url, None).await?;

    let now = now_iso();
    let row = sqlx::query(
        "INSERT INTO cameras (name, ip_address, rtsp_url, location, status, last_seen, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *",
    )
    .bind(&fields.name)
    .bind(&fields.ip_address)
    .bind(&rtsp_url)
    .bind(&fields.location)
    .bind(&status)
    .bind(Option::<String>::None)
    .bind(&now)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;
    let cam = map_camera(&row);

    // Không ghi RTSP URL vào nhật ký để tránh lộ thông tin đăng nhập camera.
    let _ = record_system_log(
        &state.db,
        "CAMERA_CREATE",
        &format!(
            "Thêm camera_id = {} · {} ({})",
            cam.id,
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
    let fields = data.validate().map_err(ApiError::unprocessable)?;
    let row = sqlx::query("SELECT * FROM cameras WHERE id = $1")
        .bind(camera_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Camera not found"));
    };
    let mut cam = map_camera(&row);

    // Bỏ trống RTSP URL hoặc gửi lại đúng bản đã che mật khẩu => giữ nguyên RTSP URL gốc.
    let mut rtsp_changed = false;
    if let Some(new_url) = fields.rtsp_url {
        let unchanged = same_rtsp(&new_url, &cam.rtsp_url)
            || same_rtsp(&new_url, &redact_rtsp_url(&cam.rtsp_url));
        if !unchanged {
            ensure_rtsp_unique(&state, &new_url, Some(cam.id)).await?;
            cam.rtsp_url = new_url;
            rtsp_changed = true;
        }
    }
    cam.name = fields.name;
    cam.ip_address = fields.ip_address;
    cam.location = fields.location;
    if let Some(status) = fields.status {
        cam.status = status;
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
        &format!(
            "Cập nhật camera_id = {} · {} ({}) · RTSP: {}",
            cam.id,
            cam.name,
            cam.status,
            if rtsp_changed { "đã thay đổi" } else { "giữ nguyên" }
        ),
        Some(user.id),
    )
    .await;

    let mut body = camera_json(&cam);
    body["rtsp_url_changed"] = json!(rtsp_changed);
    Ok(Json(body))
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
        &format!("Xóa camera_id = {} · {}", cam.id, cam.name),
        Some(user.id),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/internal/cameras/mappings` — chỉ AI Service gọi (Bearer `AI_SERVICE_TOKEN`).
///
/// Trả mapping `camera_id -> rtsp_url` gốc để AI Service đọc luồng. Endpoint này
/// không nhận JWT người dùng, được chặn ở Nginx Frontend và không bao giờ cache.
pub async fn internal_camera_mappings(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    require_ai_service_token(&headers, &state.settings)?;

    let rows = sqlx::query("SELECT * FROM cameras ORDER BY id ASC")
        .fetch_all(&state.db)
        .await?;
    let items: Vec<Value> = rows
        .iter()
        .map(map_camera)
        .map(|cam| {
            json!({
                "camera_id": cam.id,
                "name": cam.name,
                "location": cam.location,
                "status": cam.status,
                "rtsp_url": cam.rtsp_url,
            })
        })
        .collect();

    Ok((
        [
            (header::CACHE_CONTROL, "no-store, max-age=0"),
            (header::PRAGMA, "no-cache"),
        ],
        Json(json!({
            "total": items.len(),
            "items": items,
            "generated_at": now_iso(),
        })),
    ))
}
