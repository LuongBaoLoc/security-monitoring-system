//! 2. BẢO MẬT — Mã hóa mật khẩu, JWT và kiểm tra phân quyền (current_user, admin_user).

use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    cau_hinh::Settings,
    co_so_du_lieu::{map_user, AppState},
    mo_hinh_du_lieu::User,
};

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub detail: String,
}

impl ApiError {
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            status,
            detail: detail.into(),
        }
    }

    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, detail)
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, detail)
    }

    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, detail)
    }

    pub fn conflict(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, detail)
    }

    pub fn unprocessable(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, detail)
    }

    pub fn internal(err: impl std::fmt::Display) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "detail": self.detail }))).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        Self::internal(err)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub role: String,
}

pub fn hash_password(value: &str) -> Result<String, ApiError> {
    bcrypt::hash(value, bcrypt::DEFAULT_COST).map_err(ApiError::internal)
}

pub fn verify_password(value: &str, hashed: &str) -> bool {
    bcrypt::verify(value, hashed).unwrap_or(false)
}

pub fn create_token(user: &User, settings: &Settings) -> Result<String, ApiError> {
    let expires = Utc::now() + Duration::minutes(settings.access_token_expire_minutes);
    let claims = Claims {
        sub: user.id.to_string(),
        exp: expires.timestamp() as usize,
        role: user.role.clone(),
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(settings.secret_key.as_bytes()),
    )
    .map_err(ApiError::internal)
}

pub fn decode_token_user_id(token: &str, secret_key: &str) -> Result<i64, ApiError> {
    let validation = Validation::new(Algorithm::HS256);
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret_key.as_bytes()),
        &validation,
    )
    .map_err(|_| ApiError::unauthorized("Invalid or expired token"))?;
    token_data
        .claims
        .sub
        .parse::<i64>()
        .map_err(|_| ApiError::unauthorized("Invalid or expired token"))
}

pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth_val = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    auth_val
        .strip_prefix("Bearer ")
        .or_else(|| auth_val.strip_prefix("bearer "))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub async fn current_user(headers: &HeaderMap, state: &AppState) -> Result<User, ApiError> {
    let token = extract_bearer_token(headers)
        .ok_or_else(|| ApiError::unauthorized("Authentication required"))?;
    let user_id = decode_token_user_id(&token, &state.settings.secret_key)?;
    let row = sqlx::query("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;
    let user = row
        .as_ref()
        .map(map_user)
        .ok_or_else(|| ApiError::unauthorized("Inactive user"))?;
    if !user.is_active {
        return Err(ApiError::unauthorized("Inactive user"));
    }
    Ok(user)
}

pub async fn optional_user(headers: &HeaderMap, state: &AppState) -> Option<User> {
    let token = extract_bearer_token(headers)?;
    let user_id = decode_token_user_id(&token, &state.settings.secret_key).ok()?;
    let row = sqlx::query("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .ok()??;
    let user = map_user(&row);
    if user.is_active {
        Some(user)
    } else {
        None
    }
}

pub async fn admin_user(headers: &HeaderMap, state: &AppState) -> Result<User, ApiError> {
    let user = current_user(headers, state).await?;
    if user.role != "ADMIN" {
        return Err(ApiError::forbidden("Administrator role required"));
    }
    Ok(user)
}

/// So sánh 2 chuỗi bí mật với thời gian không phụ thuộc vị trí ký tự sai (chống timing attack).
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Kiểm tra header `Authorization: Bearer <AI_SERVICE_TOKEN>` (dùng cho AI Service gọi API nội bộ).
/// Trả về `false` nếu Backend chưa cấu hình `AI_SERVICE_TOKEN`.
pub fn is_valid_ai_service_token(headers: &HeaderMap, settings: &Settings) -> bool {
    let expected = settings.ai_service_token.trim();
    if expected.is_empty() {
        return false;
    }
    match extract_bearer_token(headers) {
        Some(token) => constant_time_eq(token.as_bytes(), expected.as_bytes()),
        None => false,
    }
}

/// Bắt buộc request nội bộ phải mang đúng `AI_SERVICE_TOKEN` (fail-closed nếu chưa cấu hình).
pub fn require_ai_service_token(headers: &HeaderMap, settings: &Settings) -> Result<(), ApiError> {
    if settings.ai_service_token.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "AI_SERVICE_TOKEN chưa được cấu hình trên Backend",
        ));
    }
    if !is_valid_ai_service_token(headers, settings) {
        return Err(ApiError::unauthorized("Invalid AI service token"));
    }
    Ok(())
}
