mod bao_mat; // 2. Bảo mật, JWT & phân quyền
mod cau_hinh; // 1. Cấu hình hệ thống
mod co_so_du_lieu; // 3. Kết nối & khởi tạo CSDL
mod dich_vu_nghiep_vu; // 7. Dịch vụ nghiệp vụ nền
mod dinh_tuyen_api; // 6. Định tuyến API theo từng chức năng
mod giao_tiep_mqtt; // 8. Giao tiếp MQTT với ESP32
mod ket_noi_websocket;
mod khuon_mau_du_lieu; // 5. Khuôn mẫu JSON Request/Response
mod mo_hinh_du_lieu; // 4. Mô hình bảng dữ liệu // 9. WebSocket thời gian thực

use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};
use sqlx::any::AnyPoolOptions;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::{
    cau_hinh::Settings,
    co_so_du_lieu::{init_schema, AppState},
    dich_vu_nghiep_vu::{bootstrap_admin_if_configured, seed_demo_data_if_empty},
    ket_noi_websocket::ws_dashboard,
    mo_hinh_du_lieu::RuntimeSettings,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,security_monitoring_backend=info".into()),
        )
        .init();

    sqlx::any::install_default_drivers();

    let settings = Settings::from_env();
    info!("Connecting to database: {}", settings.database_url);

    let db = AnyPoolOptions::new()
        .max_connections(10)
        .connect(&settings.database_url)
        .await?;

    init_schema(&db, settings.is_postgres).await?;

    let runtime_settings = Arc::new(RwLock::new(RuntimeSettings {
        mq2_threshold: 600.0,
        alert_confidence_threshold: settings.alert_confidence_threshold,
        alert_deduplication_seconds: settings.alert_deduplication_seconds,
        after_hours_alert: true,
        offline_device_alert: true,
        critical_notification: true,
        camera_offline_notification: true,
        daily_digest: false,
        retention_days: 30,
        auto_save_snapshot: true,
    }));

    let (ws_tx, _) = broadcast::channel::<Value>(256);

    let state = AppState {
        db,
        settings: settings.clone(),
        runtime_settings,
        ws_tx,
        mqtt_client: Arc::new(RwLock::new(None)),
    };

    bootstrap_admin_if_configured(&state).await?;
    if settings.seed_demo_data {
        seed_demo_data_if_empty(&state.db).await?;
    }

    giao_tiep_mqtt::start_mqtt(state.clone()).await;

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/ws/dashboard", get(ws_dashboard))
        .nest("/api", dinh_tuyen_api::api_router())
        .layer(cors)
        .with_state(state);

    let bind_addr = format!("{}:{}", settings.host, settings.port);
    info!("Starting Rust Backend API on http://{}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "name": state.settings.app_name,
        "runtime": "rust-axum",
        "docs": "/docs",
        "health": "/api/system/health",
    }))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
