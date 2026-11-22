//! Danh sách người dùng, Admin thêm/sửa/khóa/xóa tài khoản (/api/users/*).

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::{json, Value};

use crate::{
    bao_mat::{admin_user, hash_password, ApiError},
    co_so_du_lieu::{map_user, row_i64, AppState},
    dich_vu_nghiep_vu::record_system_log,
    khuon_mau_du_lieu::{
        envelope, normalize_role, validate_full_name, validate_password, validate_username,
        UserCreate, UserUpdate,
    },
    mo_hinh_du_lieu::{now_iso, User},
};

pub async fn list_users(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let _ = admin_user(&headers, &state).await?;
    let rows = sqlx::query("SELECT * FROM users ORDER BY id ASC")
        .fetch_all(&state.db)
        .await?;
    let items: Vec<User> = rows.iter().map(map_user).collect();
    let total = items.len() as i64;
    Ok(Json(
        serde_json::to_value(envelope(items, total, total, 0)).unwrap(),
    ))
}

pub async fn create_user(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<UserCreate>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let admin = admin_user(&headers, &state).await?;
    let username = validate_username(&payload.username).map_err(ApiError::unprocessable)?;
    let full_name = validate_full_name(&payload.full_name).map_err(ApiError::unprocessable)?;
    validate_password(&payload.password).map_err(ApiError::unprocessable)?;
    let role = normalize_role(&payload.role).map_err(ApiError::unprocessable)?;

    let existing = sqlx::query("SELECT id FROM users WHERE username = $1")
        .bind(&username)
        .fetch_optional(&state.db)
        .await?;
    if existing.is_some() {
        return Err(ApiError::conflict("Tên đăng nhập đã tồn tại"));
    }

    let hashed = hash_password(&payload.password)?;
    let now = now_iso();
    let row = sqlx::query(
        "INSERT INTO users (username, password_hash, full_name, role, is_active, created_at)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(&username)
    .bind(hashed)
    .bind(&full_name)
    .bind(&role)
    .bind(payload.is_active)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;
    let id = row_i64(&row, "id");

    let _ = record_system_log(
        &state.db,
        "USER_CREATE",
        &format!("Admin tạo tài khoản {} ({})", username, role),
        Some(admin.id),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "username": username,
            "full_name": full_name,
            "role": role,
            "is_active": payload.is_active,
            "created_at": now,
        })),
    ))
}

pub async fn update_user(
    headers: HeaderMap,
    Path(user_id): Path<i64>,
    State(state): State<AppState>,
    Json(payload): Json<UserUpdate>,
) -> Result<Json<Value>, ApiError> {
    let admin = admin_user(&headers, &state).await?;
    let row = sqlx::query("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Không tìm thấy người dùng"));
    };
    let mut target = map_user(&row);

    if target.id == admin.id {
        if payload.is_active == Some(false) {
            return Err(ApiError::bad_request(
                "Không thể tự khóa tài khoản của chính mình",
            ));
        }
        if let Some(ref r) = payload.role {
            let norm = normalize_role(r).map_err(ApiError::unprocessable)?;
            if norm != "ADMIN" {
                return Err(ApiError::bad_request(
                    "Không thể tự hạ quyền Quản trị viên của chính mình",
                ));
            }
        }
    }

    if let Some(ref name) = payload.full_name {
        target.full_name = validate_full_name(name).map_err(ApiError::unprocessable)?;
    }
    if let Some(ref role) = payload.role {
        target.role = normalize_role(role).map_err(ApiError::unprocessable)?;
    }
    if let Some(active) = payload.is_active {
        target.is_active = active;
    }
    if let Some(ref pwd) = payload.password {
        if !pwd.is_empty() {
            validate_password(pwd).map_err(ApiError::unprocessable)?;
            target.password_hash = hash_password(pwd)?;
        }
    }

    sqlx::query(
        "UPDATE users SET full_name = $1, role = $2, is_active = $3, password_hash = $4 WHERE id = $5",
    )
    .bind(&target.full_name)
    .bind(&target.role)
    .bind(target.is_active)
    .bind(&target.password_hash)
    .bind(target.id)
    .execute(&state.db)
    .await?;

    let _ = record_system_log(
        &state.db,
        "USER_UPDATE",
        &format!(
            "Cập nhật tài khoản {} (role={}, active={})",
            target.username, target.role, target.is_active
        ),
        Some(admin.id),
    )
    .await;

    Ok(Json(json!({
        "id": target.id,
        "username": target.username,
        "full_name": target.full_name,
        "role": target.role,
        "is_active": target.is_active,
        "created_at": target.created_at,
    })))
}

pub async fn delete_user(
    headers: HeaderMap,
    Path(user_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    let admin = admin_user(&headers, &state).await?;
    let row = sqlx::query("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Không tìm thấy người dùng"));
    };
    let target = map_user(&row);
    if target.id == admin.id {
        return Err(ApiError::bad_request(
            "Không thể xóa tài khoản đang đăng nhập",
        ));
    }

    sqlx::query("UPDATE alerts SET handled_by = NULL WHERE handled_by = $1")
        .bind(user_id)
        .execute(&state.db)
        .await?;
    sqlx::query("UPDATE system_logs SET user_id = NULL WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&state.db)
        .await?;

    let _ = record_system_log(
        &state.db,
        "USER_DELETE",
        &format!("Đã xóa tài khoản {}", target.username),
        Some(admin.id),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}
