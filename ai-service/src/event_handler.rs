use serde_json::{json, Value};
use crate::{config::AiConfig, detector::DetectionItem};

pub async fn send_detection(
    client: &reqwest::Client,
    config: &AiConfig,
    camera_id: i64,
    detection: &DetectionItem,
) -> Result<Value, reqwest::Error> {
    let url = format!("{}/api/detections", config.backend_url.trim_end_matches('/'));
    let mut req = client.post(&url).json(&json!({
        "camera_id": camera_id,
        "object_type": detection.object_type,
        "confidence": detection.confidence,
        "event_type": detection.event_type,
    }));

    if !config.ai_service_token.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", config.ai_service_token));
    }

    let resp = req.send().await?.error_for_status()?;
    resp.json::<Value>().await
}
