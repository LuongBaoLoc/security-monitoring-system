//! 6. ĐỊNH TUYẾN API — Mỗi chức năng trên Web nằm ở 1 file riêng, gộp lại vào `/api`.

pub mod canh_bao_an_ninh;
pub mod he_thong_va_cai_dat;
pub mod quan_ly_camera;
pub mod quan_ly_nguoi_dung;
pub mod su_kien_ai;
pub mod thiet_bi_va_cam_bien;
pub mod thong_ke_va_nhat_ky;
pub mod xac_thuc;

use axum::{
    routing::{get, post, put},
    Router,
};
use serde::Deserialize;

use crate::co_so_du_lieu::{row_i64, AppState};

// ===== Tham số truy vấn dùng chung cho nhiều nhánh API =====
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_limit_50")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct SensorsQuery {
    pub device_id: Option<String>,
    pub sensor_type: Option<String>,
    #[serde(default = "default_limit_50")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct AlertsQuery {
    pub status: Option<String>,
    pub severity: Option<String>,
    #[serde(default = "default_limit_50")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct DaysQuery {
    #[serde(default = "default_days_7")]
    pub days: i64,
}

/// Bộ lọc `GET /api/cameras?search=&location=&status=&limit=&offset=`.
#[derive(Debug, Deserialize)]
pub struct CameraListQuery {
    /// Tìm theo tên, khu vực hoặc địa chỉ IP (không tìm trong RTSP URL để tránh dò mật khẩu).
    pub search: Option<String>,
    /// Lọc đúng khu vực lắp đặt (không phân biệt hoa/thường).
    pub location: Option<String>,
    /// `ONLINE` | `OFFLINE` (bỏ trống hoặc `all` = tất cả).
    pub status: Option<String>,
    #[serde(default = "default_limit_50")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

/// Bộ lọc `GET /api/detections?camera_id=&object_type=&event_type=&min_confidence=&limit=&offset=`.
#[derive(Debug, Deserialize)]
pub struct DetectionsQuery {
    pub camera_id: Option<i64>,
    pub object_type: Option<String>,
    pub event_type: Option<String>,
    pub min_confidence: Option<f64>,
    #[serde(default = "default_limit_50")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit_50() -> i64 {
    50
}

fn default_days_7() -> i64 {
    7
}

/// Hàm đếm số dòng dùng chung (thống kê & kiểm tra trạng thái hệ thống). Câu SQL phải trả về cột `cnt`.
pub async fn count_query(
    pool: &sqlx::AnyPool,
    sql: &str,
    bind_opt: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let row = if let Some(b) = bind_opt {
        sqlx::query(sql).bind(b).fetch_one(pool).await?
    } else {
        sqlx::query(sql).fetch_one(pool).await?
    };
    Ok(row_i64(&row, "cnt"))
}

pub fn api_router() -> Router<AppState> {
    Router::new()
        // Auth
        .route("/auth/login", post(xac_thuc::login))
        .route("/auth/register", post(xac_thuc::register))
        .route("/auth/me", get(xac_thuc::me))
        // Users
        .route(
            "/users",
            get(quan_ly_nguoi_dung::list_users).post(quan_ly_nguoi_dung::create_user),
        )
        .route(
            "/users/:user_id",
            put(quan_ly_nguoi_dung::update_user).delete(quan_ly_nguoi_dung::delete_user),
        )
        // Cameras
        .route(
            "/cameras",
            get(quan_ly_camera::list_cameras).post(quan_ly_camera::create_camera),
        )
        .route(
            "/cameras/:camera_id",
            get(quan_ly_camera::get_camera)
                .put(quan_ly_camera::update_camera)
                .delete(quan_ly_camera::delete_camera),
        )
        // Devices
        .route(
            "/devices",
            get(thiet_bi_va_cam_bien::list_devices).post(thiet_bi_va_cam_bien::create_device),
        )
        .route(
            "/devices/:device_id",
            get(thiet_bi_va_cam_bien::get_device).delete(thiet_bi_va_cam_bien::delete_device),
        )
        .route(
            "/devices/:device_id/command",
            post(thiet_bi_va_cam_bien::command_device),
        )
        // Sensors
        .route("/sensors/data", post(thiet_bi_va_cam_bien::sensor_data))
        .route("/sensors", get(thiet_bi_va_cam_bien::list_sensors))
        // Detections
        .route(
            "/detections",
            get(su_kien_ai::list_detections).post(su_kien_ai::create_detection),
        )
        .route(
            "/detections/snapshot",
            post(su_kien_ai::upload_detection_snapshot)
                .layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/detections/:event_id", get(su_kien_ai::detection_detail))
        // Alerts
        .route("/alerts", get(canh_bao_an_ninh::list_alerts))
        .route("/alerts/:alert_id", get(canh_bao_an_ninh::alert_detail))
        .route(
            "/alerts/:alert_id/resolve",
            put(canh_bao_an_ninh::resolve_alert),
        )
        // Logs
        .route(
            "/logs/system",
            get(thong_ke_va_nhat_ky::system_logs).delete(thong_ke_va_nhat_ky::clear_system_logs),
        )
        .route("/logs/devices", get(thong_ke_va_nhat_ky::device_logs))
        // Statistics
        .route(
            "/statistics/overview",
            get(thong_ke_va_nhat_ky::statistics_overview),
        )
        .route(
            "/statistics/alerts",
            get(thong_ke_va_nhat_ky::statistics_alerts),
        )
        .route(
            "/statistics/detections",
            get(thong_ke_va_nhat_ky::statistics_detections),
        )
        // System settings & health
        .route(
            "/system/settings",
            get(he_thong_va_cai_dat::get_system_settings)
                .put(he_thong_va_cai_dat::update_system_settings),
        )
        .route("/system/health", get(he_thong_va_cai_dat::system_health))
        // Internal (chỉ dành cho AI Service, xác thực bằng AI_SERVICE_TOKEN)
        .route(
            "/internal/cameras/mappings",
            get(quan_ly_camera::internal_camera_mappings),
        )
}
