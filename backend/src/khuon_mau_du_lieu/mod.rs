//! 5. KHUÔN MẪU DỮ LIỆU — JSON Request/Response (Login, UserCreate, CameraCreate, CommandCreate...).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Login {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct Register {
    pub username: String,
    pub password: String,
    pub full_name: String,
}

#[derive(Debug, Deserialize)]
pub struct UserCreate {
    pub username: String,
    pub password: String,
    pub full_name: String,
    #[serde(default = "default_operator_role")]
    pub role: String,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct UserUpdate {
    pub full_name: Option<String>,
    pub role: Option<String>,
    pub is_active: Option<bool>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CameraCreate {
    pub name: String,
    pub ip_address: Option<String>,
    pub rtsp_url: String,
    pub location: Option<String>,
    /// Camera mới mặc định OFFLINE cho tới khi có tín hiệu thật (không báo "Đang hoạt động" giả).
    #[serde(default = "default_offline_status")]
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct CameraUpdate {
    pub name: String,
    pub ip_address: Option<String>,
    /// Bỏ trống / null => giữ nguyên RTSP URL gốc (kể cả user/password) đang lưu trong DB.
    pub rtsp_url: Option<String>,
    pub location: Option<String>,
    /// Bỏ trống / null => giữ nguyên trạng thái hiện tại.
    pub status: Option<String>,
}

/// Dữ liệu camera đã được kiểm tra hợp lệ và chuẩn hóa, sẵn sàng ghi vào DB.
#[derive(Debug)]
pub struct CameraFields {
    pub name: String,
    pub ip_address: Option<String>,
    /// `None` khi cập nhật mà bỏ trống RTSP URL (giữ nguyên giá trị cũ).
    pub rtsp_url: Option<String>,
    pub location: Option<String>,
    /// `None` khi cập nhật mà không gửi trạng thái (giữ nguyên giá trị cũ).
    pub status: Option<String>,
}

impl CameraCreate {
    pub fn validate(&self) -> Result<CameraFields, String> {
        let status = if self.status.trim().is_empty() {
            "OFFLINE".to_string()
        } else {
            normalize_camera_status(&self.status)?
        };
        Ok(CameraFields {
            name: validate_camera_name(&self.name)?,
            ip_address: validate_ip_address(self.ip_address.as_deref())?,
            rtsp_url: Some(validate_rtsp_url(&self.rtsp_url)?),
            location: validate_location(self.location.as_deref())?,
            status: Some(status),
        })
    }
}

impl CameraUpdate {
    pub fn validate(&self) -> Result<CameraFields, String> {
        let rtsp_url = match self.rtsp_url.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(url) => Some(validate_rtsp_url(url)?),
        };
        let status = match self.status.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(st) => Some(normalize_camera_status(st)?),
        };
        Ok(CameraFields {
            name: validate_camera_name(&self.name)?,
            ip_address: validate_ip_address(self.ip_address.as_deref())?,
            rtsp_url,
            location: validate_location(self.location.as_deref())?,
            status,
        })
    }
}

/// Kiểm tra RTSP URL: chỉ chấp nhận `rtsp://` hoặc `rtsps://`, bắt buộc có host,
/// không chứa khoảng trắng / ký tự điều khiển, độ dài tối đa 1024 ký tự.
pub fn validate_rtsp_url(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("RTSP URL không được để trống".to_string());
    }
    if value.len() > 1024 {
        return Err("RTSP URL quá dài (tối đa 1024 ký tự)".to_string());
    }
    if value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("RTSP URL không được chứa khoảng trắng hoặc ký tự điều khiển".to_string());
    }
    let parsed = url::Url::parse(value).map_err(|_| {
        "RTSP URL không hợp lệ. Định dạng đúng: rtsp://user:pass@192.168.1.21:554/stream1"
            .to_string()
    })?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if scheme != "rtsp" && scheme != "rtsps" {
        return Err("RTSP URL phải bắt đầu bằng rtsp:// hoặc rtsps://".to_string());
    }
    match parsed.host_str() {
        Some(host) if !host.trim().is_empty() => {}
        _ => return Err("RTSP URL phải có địa chỉ máy chủ (host/IP) của camera".to_string()),
    }
    if parsed.port() == Some(0) {
        return Err("Cổng (port) trong RTSP URL không hợp lệ".to_string());
    }
    if !parsed.username().is_empty() && parsed.password().is_none() {
        return Err("RTSP URL có tên đăng nhập nhưng thiếu mật khẩu (user:pass@host)".to_string());
    }
    Ok(value.to_string())
}

pub fn normalize_camera_status(value: &str) -> Result<String, String> {
    let status = value.trim().to_ascii_uppercase();
    if status != "ONLINE" && status != "OFFLINE" {
        return Err("Trạng thái camera phải là ONLINE hoặc OFFLINE".to_string());
    }
    Ok(status)
}

fn validate_camera_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("Tên camera không được để trống".to_string());
    }
    if name.chars().count() > 120 {
        return Err("Tên camera tối đa 120 ký tự".to_string());
    }
    Ok(name.to_string())
}

fn validate_location(raw: Option<&str>) -> Result<Option<String>, String> {
    match raw.map(str::trim) {
        None | Some("") => Ok(None),
        Some(loc) if loc.chars().count() > 120 => Err("Khu vực lắp đặt tối đa 120 ký tự".to_string()),
        Some(loc) => Ok(Some(loc.to_string())),
    }
}

fn validate_ip_address(raw: Option<&str>) -> Result<Option<String>, String> {
    match raw.map(str::trim) {
        None | Some("") => Ok(None),
        Some(ip) => ip
            .parse::<std::net::IpAddr>()
            .map(|addr| Some(addr.to_string()))
            .map_err(|_| format!("Địa chỉ IP \"{ip}\" không hợp lệ (vd: 192.168.1.21)")),
    }
}

/// Tên đăng nhập: 3–32 ký tự, chỉ gồm chữ cái không dấu, số và `_ . -`.
pub fn validate_username(raw: &str) -> Result<String, String> {
    let username = raw.trim();
    let len = username.chars().count();
    if !(3..=32).contains(&len) {
        return Err("Tên đăng nhập phải từ 3 đến 32 ký tự".to_string());
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        return Err(
            "Tên đăng nhập chỉ gồm chữ cái không dấu, số và các ký tự _ . -".to_string(),
        );
    }
    Ok(username.to_string())
}

pub fn validate_full_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("Họ tên không được để trống".to_string());
    }
    if name.chars().count() > 120 {
        return Err("Họ tên tối đa 120 ký tự".to_string());
    }
    Ok(name.to_string())
}

pub fn validate_password(raw: &str) -> Result<(), String> {
    let len = raw.chars().count();
    if len < 8 {
        return Err("Mật khẩu phải từ 8 ký tự trở lên".to_string());
    }
    if len > 128 {
        return Err("Mật khẩu tối đa 128 ký tự".to_string());
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct DeviceCreate {
    pub device_code: String,
    pub device_name: String,
    #[serde(default = "default_esp32_type")]
    pub device_type: String,
    #[serde(default = "default_online_status")]
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct DetectionCreate {
    pub camera_id: i64,
    pub object_type: String,
    pub confidence: f64,
    pub event_type: String,
    pub image_path: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
}

impl DetectionCreate {
    pub fn validate(&self) -> Result<(), String> {
        if self.object_type.trim().is_empty() || self.object_type.len() > 80 {
            return Err("object_type must be 1..80 chars".to_string());
        }
        if !(0.0..=1.0).contains(&self.confidence) {
            return Err("confidence must be between 0 and 1".to_string());
        }
        if self.event_type.trim().is_empty() || self.event_type.len() > 80 {
            return Err("event_type must be 1..80 chars".to_string());
        }
        if let Some(ref path) = self.image_path {
            let normalized = path.replace('\\', "/");
            if normalized.starts_with('/')
                || normalized.split('/').any(|p| p == "..")
                || normalized.contains(':')
            {
                return Err("image_path must be a relative path".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct SensorCreate {
    pub device_id: String,
    pub pir: Option<bool>,
    pub mq2: Option<f64>,
    pub relay: Option<bool>,
    pub buzzer: Option<bool>,
    pub lock: Option<bool>,
    pub timestamp: Option<DateTime<Utc>>,
}

impl SensorCreate {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(mq2) = self.mq2 {
            if !(0.0..=100_000.0).contains(&mq2) {
                return Err("mq2 must be between 0 and 100000".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct CommandCreate {
    pub action: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SettingsUpdate {
    #[serde(default = "default_mq2_threshold")]
    pub mq2_threshold: f64,
    #[serde(default = "default_conf_threshold")]
    pub alert_confidence_threshold: f64,
    #[serde(default = "default_dedup_seconds")]
    pub alert_deduplication_seconds: i64,
    #[serde(default = "default_true")]
    pub after_hours_alert: bool,
    #[serde(default = "default_true")]
    pub offline_device_alert: bool,
    #[serde(default = "default_true")]
    pub critical_notification: bool,
    #[serde(default = "default_true")]
    pub camera_offline_notification: bool,
    #[serde(default)]
    pub daily_digest: bool,
    #[serde(default = "default_retention_days")]
    pub retention_days: i64,
    #[serde(default = "default_true")]
    pub auto_save_snapshot: bool,
}

#[derive(Debug, Serialize)]
pub struct Envelope<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

pub fn envelope<T>(items: Vec<T>, total: i64, limit: i64, offset: i64) -> Envelope<T> {
    Envelope {
        items,
        total,
        limit,
        offset,
    }
}

pub fn normalize_role(value: &str) -> Result<String, String> {
    let role = value.trim().to_ascii_uppercase();
    if role != "ADMIN" && role != "OPERATOR" {
        return Err("role must be ADMIN or OPERATOR".to_string());
    }
    Ok(role)
}

fn default_operator_role() -> String {
    "OPERATOR".to_string()
}

fn default_online_status() -> String {
    "ONLINE".to_string()
}

fn default_offline_status() -> String {
    "OFFLINE".to_string()
}

fn default_esp32_type() -> String {
    "ESP32".to_string()
}

fn default_true() -> bool {
    true
}

fn default_mq2_threshold() -> f64 {
    600.0
}

fn default_conf_threshold() -> f64 {
    0.8
}

fn default_dedup_seconds() -> i64 {
    60
}

fn default_retention_days() -> i64 {
    30
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_rtsp_urls_correctly() {
        assert!(validate_rtsp_url("rtsp://admin:pass123@192.168.1.21:554/stream1").is_ok());
        assert!(validate_rtsp_url("rtsps://cam.local/live").is_ok());
        assert!(validate_rtsp_url("http://192.168.1.21/stream").is_err());
        assert!(validate_rtsp_url("rtsp://").is_err());
        assert!(validate_rtsp_url("rtsp://useronly@192.168.1.21/live").is_err());
        assert!(validate_rtsp_url("rtsp://192.168.1.21:554/my stream").is_err());
    }

    #[test]
    fn camera_update_blank_rtsp_preserves_original() {
        let upd = CameraUpdate {
            name: "CAM-01".to_string(),
            ip_address: Some("192.168.1.21".to_string()),
            rtsp_url: Some("   ".to_string()),
            location: Some("Nhà kho Khu A".to_string()),
            status: Some("ONLINE".to_string()),
        };
        let fields = upd.validate().expect("valid update");
        assert!(fields.rtsp_url.is_none());
    }
}
