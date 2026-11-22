use std::env;

#[derive(Clone, Debug)]
pub struct AiConfig {
    pub backend_url: String,
    pub ai_service_token: String,
    pub fire_model_path: String,
    pub security_model_path: String,
    pub confidence_threshold: f32,
    pub camera_id: Option<i64>,
    pub rtsp_url: Option<String>,
    pub frame_interval_seconds: u64,
    pub max_image_bytes: usize,
    pub host: String,
    pub port: u16,
}

impl AiConfig {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        Self {
            backend_url: env::var("BACKEND_URL")
                .unwrap_or_else(|_| "http://localhost:8000".to_string()),
            ai_service_token: env::var("AI_SERVICE_TOKEN").unwrap_or_default(),
            fire_model_path: env::var("FIRE_MODEL_PATH")
                .unwrap_or_else(|_| "models/fire-smoke.onnx".to_string()),
            security_model_path: env::var("SECURITY_MODEL_PATH")
                .unwrap_or_else(|_| "models/yolov8n.onnx".to_string()),
            confidence_threshold: env::var("CONFIDENCE_THRESHOLD")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| *v > 0.0 && *v <= 1.0)
                .unwrap_or(0.5),
            camera_id: env::var("AI_CAMERA_ID")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|id| *id > 0),
            rtsp_url: env::var("AI_RTSP_URL").ok().filter(|url| !url.is_empty()),
            frame_interval_seconds: env::var("AI_FRAME_INTERVAL_SECONDS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5)
                .clamp(1, 3600),
            max_image_bytes: env::var("MAX_IMAGE_BYTES")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(8 * 1024 * 1024),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(8001),
        }
    }
}
