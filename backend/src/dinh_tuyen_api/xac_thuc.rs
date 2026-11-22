//! Đăng nhập, Đăng ký, Lấy thông tin phiên (/api/auth/*).

use std::sync::OnceLock;

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
    khuon_mau_du_lieu::{validate_full_name, validate_password, validate_username, Login, Register},
    mo_hinh_du_lieu::now_iso,
};

const INVALID_CREDENTIALS: &str = "Sai tên đăng nhập hoặc mật khẩu";

/// Hash giả dùng khi username không tồn tại, để thời gian phản hồi giống hệt trường hợp
/// sai mật khẩu (tránh dò tên đăng nhập bằng cách đo thời gian).
fn dummy_password_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        bcrypt::hash("smartsecure-timing-equalizer", bcrypt::DEFAULT_COST).unwrap_or_default()
    })
}

/// bcrypt tốn CPU (~100–300ms) nên chạy trên thread riêng, không chặn runtime async.
async fn verify_password_blocking(password: String, hashed: String) -> bool {
    tokio::task::spawn_blocking(move || verify_password(&password, &hashed))
        .await
        .unwrap_or(false)
}

/// Rút gọn username đưa vào nhật ký (chỉ ký tự in được, tối đa 32 ký tự).
fn sanitize_for_log(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(32)
        .collect()
}

fn user_json(user: &crate::mo_hinh_du_lieu::User) -> Value {
    json!({
        "id": user.id,
        "username": user.username,
        "full_name": user.full_name,
        "role": user.role,
        "is_active": user.is_active,
    })
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<Login>,
) -> Result<Json<Value>, ApiError> {
    let username = payload.username.trim().to_string();
    if username.is_empty() || payload.password.is_empty() {
        return Err(ApiError::unprocessable(
            "Vui lòng nhập tên đăng nhập và mật khẩu",
        ));
    }
    if username.chars().count() > 64 || payload.password.chars().count() > 128 {
        return Err(ApiError::unauthorized(INVALID_CREDENTIALS));
    }

    let row = sqlx::query("SELECT * FROM users WHERE username = $1")
        .bind(&username)
        .fetch_optional(&state.db)
        .await?;
    let user = row.as_ref().map(map_user);
    let hashed = user
        .as_ref()
        .map(|u| u.password_hash.clone())
        .unwrap_or_else(|| dummy_password_hash().to_string());
    let password_ok = verify_password_blocking(payload.password.clone(), hashed).await;

    let user = match user {
        Some(u) if password_ok => u,
        _ => {
            let _ = record_system_log(
                &state.db,
                "USER_LOGIN_FAILED",
                &format!(
                    "Đăng nhập thất bại với tên đăng nhập \"{}\"",
                    sanitize_for_log(&username)
                ),
                None,
            )
            .await;
            return Err(ApiError::unauthorized(INVALID_CREDENTIALS));
        }
    };
    if !user.is_active {
        return Err(ApiError::forbidden(
            "Tài khoản đã bị khóa. Vui lòng liên hệ Quản trị viên.",
        ));
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
        "user": user_json(&user),
    })))
}

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<Register>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !state.settings.registration_enabled {
        return Err(ApiError::forbidden("Registration is disabled"));
    }
    let username = validate_username(&payload.username).map_err(ApiError::unprocessable)?;
    let full_name = validate_full_name(&payload.full_name).map_err(ApiError::unprocessable)?;
    validate_password(&payload.password).map_err(ApiError::unprocessable)?;

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
         VALUES ($1, $2, $3, 'OPERATOR', TRUE, $4) RETURNING id",
    )
    .bind(&username)
    .bind(hashed)
    .bind(&full_name)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;
    let user_id = row_i64(&row, "id");

    let _ = record_system_log(
        &state.db,
        "USER_REGISTER",
        &format!("Đăng ký tài khoản mới: {username}"),
        Some(user_id),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": user_id,
            "username": username,
            "full_name": full_name,
            "role": "OPERATOR",
        })),
    ))
}

pub async fn me(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let user = current_user(&headers, &state).await?;
    Ok(Json(user_json(&user)))
}
