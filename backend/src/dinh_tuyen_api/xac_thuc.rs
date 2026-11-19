//! Đăng nhập, Đăng ký, Lấy thông tin phiên (/api/auth/*).

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::{json, Value};

use crate::{
    bao_mat::{create_token, current_user, hash_password, verify_password, ApiError},
    co_so_du_lieu::{map_user, row_i64, AppState},
    dich_vu_nghiep_vu::record_system_log,
    khuon_mau_du_lieu::{Login, Register},
    mo_hinh_du_lieu::now_iso,
};

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<Login>,
) -> Result<Json<Value>, ApiError> {
    let row = sqlx::query("SELECT * FROM users WHERE username = $1")
        .bind(&payload.username)
        .fetch_optional(&state.db)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::unauthorized("Sai tên đăng nhập hoặc mật khẩu"));
    };
    let user = map_user(&row);
    if !verify_password(&payload.password, &user.password_hash) {
        return Err(ApiError::unauthorized("Sai tên đăng nhập hoặc mật khẩu"));
    }
    if !user.is_active {
        return Err(ApiError::forbidden("Tài khoản đã bị khóa"));
    }

    let _ = record_system_log(
        &state.db,
        "USER_LOGIN",
        &format!(
            "Người dùng {} ({}) đăng nhập hệ thống",
            user.username, user.role
        ),
        Some(user.id),
    )
    .await;

    let token = create_token(&user, &state.settings)?;
    Ok(Json(json!({
        "access_token": token,
        "token_type": "bearer",
        "expires_in": state.settings.access_token_expire_minutes * 60,
    })))
}

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<Register>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !state.settings.registration_enabled {
        return Err(ApiError::forbidden("Registration is disabled"));
    }
    if payload.password.len() < 8 {
        return Err(ApiError::unprocessable(
            "Password must be at least 8 characters",
        ));
    }
    let existing = sqlx::query("SELECT id FROM users WHERE username = $1")
        .bind(payload.username.trim())
        .fetch_optional(&state.db)
        .await?;
    if existing.is_some() {
        return Err(ApiError::conflict("Username already exists"));
    }

    let hashed = hash_password(&payload.password)?;
    let now = now_iso();
    let row = sqlx::query(
        "INSERT INTO users (username, password_hash, full_name, role, is_active, created_at)
         VALUES ($1, $2, $3, 'OPERATOR', TRUE, $4) RETURNING id",
    )
    .bind(payload.username.trim())
    .bind(hashed)
    .bind(payload.full_name.trim())
    .bind(&now)
    .fetch_one(&state.db)
    .await?;
    let user_id = row_i64(&row, "id");

    let _ = record_system_log(
        &state.db,
        "USER_REGISTER",
        &format!("Đăng ký tài khoản mới: {}", payload.username.trim()),
        Some(user_id),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": user_id,
            "username": payload.username.trim(),
            "full_name": payload.full_name.trim(),
            "role": "OPERATOR",
        })),
    ))
}

pub async fn me(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let user = current_user(&headers, &state).await?;
    Ok(Json(json!({
        "id": user.id,
        "username": user.username,
        "full_name": user.full_name,
        "role": user.role,
        "is_active": user.is_active,
    })))
}
