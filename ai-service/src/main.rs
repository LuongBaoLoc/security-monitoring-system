use std::time::Duration;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;

use ai_service::{
    config::AiConfig,
    detector::detect,
    event_handler::send_detection,
};

#[derive(Clone)]
struct AiState {
    config: AiConfig,
    http_client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct DetectQuery {
    camera_id: i64,
}

struct ApiErr(StatusCode, String);

impl IntoResponse for ApiErr {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "detail": self.1 }))).into_response()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,security_monitoring_ai_service=info".into()),
        )
        .init();

    let config = AiConfig::from_env();
    let max_body = config.max_image_bytes + 1024 * 1024;
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let state = AiState {
        config: config.clone(),
        http_client,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/detect", post(detect_frame))
        .layer(DefaultBodyLimit::max(max_body))
        .with_state(state);

    let bind_addr = format!("{}:{}", config.host, config.port);
    info!("Starting Rust AI Detection Service on http://{}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health(State(state): State<AiState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "model_configured": state.config.is_model_configured(),
    }))
}

async fn detect_frame(
    Query(query): Query<DetectQuery>,
    headers: HeaderMap,
    State(state): State<AiState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiErr> {
    if !state.config.ai_service_token.is_empty() {
        let auth = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if auth != format!("Bearer {}", state.config.ai_service_token) {
            return Err(ApiErr(
                StatusCode::UNAUTHORIZED,
                "Invalid AI service token".to_string(),
            ));
        }
    }

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut content_type_ok = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiErr(StatusCode::BAD_REQUEST, "Invalid multipart upload".to_string()))?
    {
        if field.name() == Some("file") {
            let ct = field.content_type().unwrap_or("").to_string();
            if ct.starts_with("image/") {
                content_type_ok = true;
            }
            let bytes = field
                .bytes()
                .await
                .map_err(|_| ApiErr(StatusCode::BAD_REQUEST, "Failed to read uploaded file".to_string()))?;
            file_bytes = Some(bytes.to_vec());
            break;
        }
    }

    let Some(contents) = file_bytes else {
        return Err(ApiErr(
            StatusCode::BAD_REQUEST,
            "Missing 'file' field in multipart form".to_string(),
        ));
    };

    if !content_type_ok {
        return Err(ApiErr(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Upload must be an image".to_string(),
        ));
    }

    if contents.len() > state.config.max_image_bytes {
        return Err(ApiErr(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Image exceeds configured size limit".to_string(),
        ));
    }

    if state.config.model_path.is_empty() {
        return Err(ApiErr(
            StatusCode::SERVICE_UNAVAILABLE,
            "No YOLO model configured. Set MODEL_PATH to a model file.".to_string(),
        ));
    }

    let detections = detect(&contents, &state.config)
        .map_err(|_| ApiErr(StatusCode::BAD_REQUEST, "Invalid image".to_string()))?;

    let mut forwarded = Vec::with_capacity(detections.len());
    for item in &detections {
        let resp = send_detection(&state.http_client, &state.config, query.camera_id, item)
            .await
            .map_err(|_| {
                ApiErr(
                    StatusCode::BAD_GATEWAY,
                    "Could not submit detection to backend".to_string(),
                )
            })?;
        forwarded.push(resp);
    }

    Ok(Json(json!({
        "camera_id": query.camera_id,
        "count": detections.len(),
        "detections": forwarded,
    })))
}
