use axum::{
    extract::{DefaultBodyLimit, Multipart, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;
use tracing::{info, warn};

use ai_service::{
    camera::capture_rtsp_frame,
    config::AiConfig,
    detector::{DetectionError, DetectionItem, Detector},
    event_handler::send_detection,
};

#[derive(Clone)]
struct AiState {
    config: AiConfig,
    http_client: reqwest::Client,
    detector: Option<Arc<Mutex<Detector>>>,
    models: Vec<&'static str>,
    available: Arc<Semaphore>,
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

    let detector = match Detector::load(&config) {
        Ok(detector) => Some(detector),
        Err(error) => {
            warn!(%error, "YOLOv8 inference is unavailable");
            None
        }
    };
    let models = detector.as_ref().map(Detector::models).unwrap_or_default();
    info!(?models, "YOLOv8 model status");
    let state = AiState {
        config: config.clone(),
        http_client,
        detector: detector.map(|detector| Arc::new(Mutex::new(detector))),
        models,
        available: Arc::new(Semaphore::new(1)),
    };

    if let (Some(camera_id), Some(url), Some(_)) =
        (config.camera_id, &config.rtsp_url, &state.detector)
    {
        if url.starts_with("rtsp://") || url.starts_with("rtsps://") {
            tokio::spawn(monitor_camera(state.clone(), camera_id, url.clone()));
        } else {
            warn!("AI_RTSP_URL must use rtsp:// or rtsps://");
        }
    }

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
        "model_configured": !state.models.is_empty(),
        "models": state.models,
    }))
}

async fn detect_frame(
    Query(query): Query<DetectQuery>,
    headers: HeaderMap,
    State(state): State<AiState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiErr> {
    if query.camera_id <= 0 {
        return Err(ApiErr(
            StatusCode::BAD_REQUEST,
            "camera_id must be positive".into(),
        ));
    }
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

    let mut file_bytes = None;
    let mut content_type_ok = false;

    while let Some(field) = multipart.next_field().await.map_err(|_| {
        ApiErr(
            StatusCode::BAD_REQUEST,
            "Invalid multipart upload".to_string(),
        )
    })? {
        if field.name() == Some("file") {
            let ct = field.content_type().unwrap_or("").to_string();
            if ct.starts_with("image/") {
                content_type_ok = true;
            }
            let bytes = field.bytes().await.map_err(|_| {
                ApiErr(
                    StatusCode::BAD_REQUEST,
                    "Failed to read uploaded file".to_string(),
                )
            })?;
            file_bytes = Some(bytes);
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

    let detections = analyze(&state, contents.to_vec()).await?;

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

async fn analyze(state: &AiState, bytes: Vec<u8>) -> Result<Vec<DetectionItem>, ApiErr> {
    let detector = state.detector.clone().ok_or_else(|| {
        ApiErr(
            StatusCode::SERVICE_UNAVAILABLE,
            "No usable YOLOv8 ONNX model configured".into(),
        )
    })?;
    let permit = state.available.clone().try_acquire_owned().map_err(|_| {
        ApiErr(
            StatusCode::SERVICE_UNAVAILABLE,
            "YOLOv8 inference is busy".into(),
        )
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut detector = detector.lock().map_err(|_| {
            ApiErr(
                StatusCode::SERVICE_UNAVAILABLE,
                "YOLOv8 inference is unavailable".into(),
            )
        })?;
        detector.detect(&bytes).map_err(|error| match error {
            DetectionError::InvalidImage => ApiErr(StatusCode::BAD_REQUEST, "Invalid image".into()),
            DetectionError::Inference(error) => {
                warn!(%error, "YOLOv8 inference failed");
                ApiErr(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "YOLOv8 inference failed".into(),
                )
            }
        })
    })
    .await
    .map_err(|_| {
        ApiErr(
            StatusCode::INTERNAL_SERVER_ERROR,
            "YOLOv8 worker failed".into(),
        )
    })?
}

async fn monitor_camera(state: AiState, camera_id: i64, url: String) {
    info!(camera_id, "Started YOLOv8 RTSP monitoring");
    let delay = Duration::from_secs(state.config.frame_interval_seconds);
    loop {
        match capture_rtsp_frame(&url).await {
            Ok(frame) if frame.len() <= state.config.max_image_bytes => {
                match analyze(&state, frame).await {
                    Ok(detections) => {
                        let mut published = HashSet::new();
                        for detection in &detections {
                            if published.insert(detection.object_type.as_str()) {
                                if let Err(error) = send_detection(
                                    &state.http_client,
                                    &state.config,
                                    camera_id,
                                    detection,
                                )
                                .await
                                {
                                    warn!(camera_id, status = ?error.status(), "Unable to forward camera detection");
                                }
                            }
                        }
                    }
                    Err(error) => {
                        warn!(camera_id, status = %error.0, "Unable to analyze camera frame")
                    }
                }
            }
            _ => warn!(
                camera_id,
                "Unable to read a frame from the configured RTSP camera"
            ),
        }
        tokio::time::sleep(delay).await;
    }
}
