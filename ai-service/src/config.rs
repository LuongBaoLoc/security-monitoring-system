use std::env;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct AiConfig {
    pub backend_url: String,
    pub ai_service_token: String,
    pub model_path: String,
    pub confidence_threshold: f64,
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
            model_path: env::var("MODEL_PATH").unwrap_or_default(),
            confidence_threshold: env::var("CONFIDENCE_THRESHOLD")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.6),
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
}
