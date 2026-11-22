use chrono::Utc;
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{config::AiConfig, detector::DetectionItem};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraMapping {
    pub camera_id: i64,
    pub name: String,
    pub location: Option<String>,
    pub status: String,
    pub rtsp_url: String,
}

#[derive(Debug, Deserialize)]
struct MappingsResponse {
    items: Vec<CameraMapping>,
}

/// Lấy danh sách ánh xạ `camera_id -> rtsp_url` nội bộ từ Backend bằng `AI_SERVICE_TOKEN`.
pub async fn fetch_camera_mappings(
    client: &reqwest::Client,
    config: &AiConfig,
) -> Result<Vec<CameraMapping>, String> {
    if config.ai_service_token.trim().is_empty() {
        return Err("AI_SERVICE_TOKEN chưa được cấu hình".to_string());
    }
    let url = format!(
        "{}/api/internal/cameras/mappings",
        config.backend_url.trim_end_matches('/')
    );
    let resp = client
        .get(&url)
        .header(
            "Authorization",
            format!("Bearer {}", config.ai_service_token.trim()),
        )
        .send()
        .await
        .map_err(|e| format!("Không thể kết nối Backend lấy camera mappings: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Backend từ chối yêu cầu camera mappings (HTTP {})",
            resp.status()
        ));
    }
    let body: MappingsResponse = resp
        .json()
        .await
        .map_err(|e| format!("Lỗi đọc JSON camera mappings: {e}"))?;
    Ok(body.items)
}

/// Tải ảnh snapshot khung hình lên Backend (`POST /api/detections/snapshot`) để nhận `image_path` an toàn.
pub async fn upload_snapshot(
    client: &reqwest::Client,
    config: &AiConfig,
    camera_id: i64,
    image_bytes: &[u8],
) -> Result<String, String> {
    let url = format!(
        "{}/api/detections/snapshot",
        config.backend_url.trim_end_matches('/')
    );
    let file_part = Part::bytes(image_bytes.to_vec())
        .file_name(format!("cam_{camera_id}_snapshot.jpg"))
        .mime_str("image/jpeg")
        .map_err(|e| format!("Không thể tạo multipart ảnh: {e}"))?;

    let form = Form::new()
        .text("camera_id", camera_id.to_string())
        .part("file", file_part);

    let mut req = client.post(&url).multipart(form);
    if !config.ai_service_token.trim().is_empty() {
        req = req.header(
            "Authorization",
            format!("Bearer {}", config.ai_service_token.trim()),
        );
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("Lỗi kết nối khi tải snapshot lên Backend: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(format!("Upload snapshot thất bại (HTTP {status}): {detail}"));
    }

    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("Lỗi đọc phản hồi upload snapshot: {e}"))?;
    body.get("image_path")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| "Phản hồi upload snapshot thiếu trường image_path".to_string())
}

/// Gửi đầy đủ payload sự kiện nhận diện AI (`camera_id`, `object_type`, `confidence`,
/// `event_type`, `image_path`, `timestamp`) lên Backend `POST /api/detections`.
pub async fn send_detection(
    client: &reqwest::Client,
    config: &AiConfig,
    camera_id: i64,
    detection: &DetectionItem,
    image_path: Option<&str>,
    timestamp: Option<&str>,
) -> Result<Value, String> {
    let url = format!(
        "{}/api/detections",
        config.backend_url.trim_end_matches('/')
    );
    let ts = timestamp
        .map(str::to_string)
        .unwrap_or_else(|| Utc::now().to_rfc3339());

    let payload = json!({
        "camera_id": camera_id,
        "object_type": detection.object_type,
        "confidence": detection.confidence,
        "event_type": detection.event_type,
        "image_path": image_path,
        "timestamp": ts,
    });

    let mut req = client.post(&url).json(&payload);
    if !config.ai_service_token.trim().is_empty() {
        req = req.header(
            "Authorization",
            format!("Bearer {}", config.ai_service_token.trim()),
        );
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("Lỗi kết nối khi gửi sự kiện AI tới Backend: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(format!("Gửi sự kiện AI thất bại (HTTP {status}): {detail}"));
    }

    resp.json::<Value>()
        .await
        .map_err(|e| format!("Lỗi đọc phản hồi POST /api/detections: {e}"))
}
