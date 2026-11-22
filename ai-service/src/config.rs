use std::env;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct AiConfig {
    pub backend_url: String,
    pub ai_service_token: String,
    pub model_path: String,
    pub yolo_classes: Vec<String>,
    pub rtsp_server_url: String,
    pub confidence_threshold: f64,
    pub nms_iou_threshold: f32,
    pub input_size: usize,
    pub ffmpeg_timeout_secs: u64,
    pub max_image_bytes: usize,
    pub host: String,
    pub port: u16,
}

impl AiConfig {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        let classes_raw = env::var("YOLO_CLASSES").unwrap_or_else(|_| "fire,smoke".to_string());
        let yolo_classes: Vec<String> = classes_raw
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        let yolo_classes = if yolo_classes.is_empty() {
            vec!["fire".to_string(), "smoke".to_string()]
        } else {
            yolo_classes
        };

        Self {
            backend_url: env::var("BACKEND_URL")
                .unwrap_or_else(|_| "http://localhost:8000".to_string()),
            ai_service_token: env::var("AI_SERVICE_TOKEN").unwrap_or_default(),
            model_path: env::var("MODEL_PATH").unwrap_or_default(),
            yolo_classes,
            rtsp_server_url: env::var("RTSP_SERVER_URL")
                .unwrap_or_else(|_| "rtsp://mediamtx:8554".to_string()),
            confidence_threshold: env::var("CONFIDENCE_THRESHOLD")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.6),
            nms_iou_threshold: env::var("NMS_IOU_THRESHOLD")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(0.45),
            input_size: env::var("YOLO_INPUT_SIZE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|&n| (64..=2048).contains(&n))
                .unwrap_or(640),
            ffmpeg_timeout_secs: env::var("FFMPEG_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|&s| (1..=120).contains(&s))
                .unwrap_or(10),
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

    pub fn is_model_configured(&self) -> bool {
        !self.model_path.is_empty() && Path::new(&self.model_path).is_file()
    }

    pub fn is_onnx_model(&self) -> bool {
        self.is_model_configured()
            && Path::new(&self.model_path)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("onnx"))
                .unwrap_or(false)
    }

    pub fn ffmpeg_timeout(&self) -> Duration {
        Duration::from_secs(self.ffmpeg_timeout_secs)
    }
}
