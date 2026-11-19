//! 7. DỊCH VỤ NGHIỆP VỤ — Tạo Admin mặc định, dữ liệu mẫu, ghi nhật ký hệ thống.

use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::AnyPool;
use url::Url;

use crate::{
    bao_mat::hash_password,
    co_so_du_lieu::{row_i64, AppState},
    mo_hinh_du_lieu::{now_iso, Camera},
};

pub fn redact_rtsp_url(rtsp_url: &str) -> String {
    if let Ok(mut parsed) = Url::parse(rtsp_url) {
        let _ = parsed.set_username("");
        let _ = parsed.set_password(None);
        parsed.set_query(None);
        parsed.set_fragment(None);
        parsed.to_string()
    } else {
        String::new()
    }
}

pub fn camera_json(camera: &Camera) -> Value {
    json!({
        "id": camera.id,
        "name": camera.name,
        "ip_address": camera.ip_address,
        "rtsp_url": redact_rtsp_url(&camera.rtsp_url),
        "location": camera.location,
        "status": camera.status,
        "last_seen": camera.last_seen,
        "created_at": camera.created_at,
        "updated_at": camera.updated_at,
    })
}

pub async fn record_system_log(
    pool: &AnyPool,
    action: &str,
    description: &str,
    user_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO system_logs (user_id, action, description, created_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(action)
    .bind(description)
    .bind(now_iso())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn bootstrap_admin_if_configured(state: &AppState) -> Result<(), sqlx::Error> {
    let (Some(username), Some(password)) = (
        state.settings.bootstrap_admin_username.as_ref(),
        state.settings.bootstrap_admin_password.as_ref(),
    ) else {
        return Ok(());
    };

    let existing = sqlx::query("SELECT id FROM users WHERE username = $1")
        .bind(username)
        .fetch_optional(&state.db)
        .await?;

    if existing.is_none() {
        if let Ok(hashed) = hash_password(password) {
            sqlx::query(
                "INSERT INTO users (username, password_hash, full_name, role, is_active, created_at)
                 VALUES ($1, $2, $3, 'ADMIN', TRUE, $4)",
            )
            .bind(username)
            .bind(hashed)
            .bind("System Administrator")
            .bind(now_iso())
            .execute(&state.db)
            .await?;
        }
    }
    Ok(())
}

pub async fn seed_demo_data_if_empty(pool: &AnyPool) -> Result<(), sqlx::Error> {
    let cam_row = sqlx::query("SELECT COUNT(id) AS cnt FROM cameras")
        .fetch_one(pool)
        .await?;
    let dev_row = sqlx::query("SELECT COUNT(id) AS cnt FROM devices")
        .fetch_one(pool)
        .await?;
    if row_i64(&cam_row, "cnt") > 0 || row_i64(&dev_row, "cnt") > 0 {
        return Ok(());
    }

    let now = Utc::now();
    let admin_id = sqlx::query("SELECT id FROM users WHERE role = 'ADMIN' LIMIT 1")
        .fetch_optional(pool)
        .await?
        .as_ref()
        .map(|r| row_i64(r, "id"));

    let cameras = [
        (
            "CAM-01 · Cổng chính",
            "192.168.1.21",
            "rtsp://demo:secret@192.168.1.21:554/stream1",
            "Cổng phía Đông",
            "ONLINE",
            (now - Duration::minutes(1)).to_rfc3339(),
            (now - Duration::days(7)).to_rfc3339(),
        ),
        (
            "CAM-02 · Sảnh lễ tân",
            "192.168.1.22",
            "rtsp://demo:secret@192.168.1.22:554/stream1",
            "Tầng trệt",
            "ONLINE",
            (now - Duration::minutes(2)).to_rfc3339(),
            (now - Duration::days(7)).to_rfc3339(),
        ),
        (
            "CAM-03 · Hành lang A",
            "192.168.1.23",
            "rtsp://demo:secret@192.168.1.23:554/stream1",
            "Tầng 1",
            "ONLINE",
            (now - Duration::minutes(3)).to_rfc3339(),
            (now - Duration::days(6)).to_rfc3339(),
        ),
        (
            "CAM-08 · Kho hàng",
            "192.168.1.28",
            "rtsp://demo:secret@192.168.1.28:554/stream1",
            "Khu nhà kho",
            "OFFLINE",
            (now - Duration::hours(2)).to_rfc3339(),
            (now - Duration::days(5)).to_rfc3339(),
        ),
    ];

    let mut cam_ids = Vec::new();
    for (name, ip, rtsp, loc, st, last_seen, created) in cameras {
        let row = sqlx::query(
            "INSERT INTO cameras (name, ip_address, rtsp_url, location, status, last_seen, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
        )
        .bind(name)
        .bind(ip)
        .bind(rtsp)
        .bind(loc)
        .bind(st)
        .bind(&last_seen)
        .bind(&created)
        .bind(&last_seen)
        .fetch_one(pool)
        .await?;
        cam_ids.push(row_i64(&row, "id"));
    }

    let dev1 = sqlx::query(
        "INSERT INTO devices (device_code, device_name, device_type, status, last_seen, latest_pir, latest_mq2, relay, buzzer, lock_engaged, created_at)
         VALUES ($1, $2, 'ESP32', 'ONLINE', $3, FALSE, 350.0, FALSE, FALSE, TRUE, $4) RETURNING id",
    )
    .bind("ESP32_001")
    .bind("ESP32 khu vực A · Tầng 1")
    .bind((now - Duration::seconds(30)).to_rfc3339())
    .bind((now - Duration::days(7)).to_rfc3339())
    .fetch_one(pool)
    .await?;
    let dev1_id = row_i64(&dev1, "id");

    let dev2 = sqlx::query(
        "INSERT INTO devices (device_code, device_name, device_type, status, last_seen, latest_pir, latest_mq2, relay, buzzer, lock_engaged, created_at)
         VALUES ($1, $2, 'ESP32', 'OFFLINE', $3, FALSE, 280.0, FALSE, FALSE, TRUE, $4) RETURNING id",
    )
    .bind("ESP32_002")
    .bind("ESP32 khu vực B · Kho hàng")
    .bind((now - Duration::hours(5)).to_rfc3339())
    .bind((now - Duration::days(6)).to_rfc3339())
    .fetch_one(pool)
    .await?;
    let dev2_id = row_i64(&dev2, "id");

    let sensors = [
        (
            dev1_id,
            "pir",
            json!({"value": false}).to_string(),
            (now - Duration::minutes(5)).to_rfc3339(),
        ),
        (
            dev1_id,
            "mq2",
            json!({"value": 350.0}).to_string(),
            (now - Duration::minutes(5)).to_rfc3339(),
        ),
        (
            dev1_id,
            "lock",
            json!({"value": true}).to_string(),
            (now - Duration::minutes(5)).to_rfc3339(),
        ),
        (
            dev2_id,
            "mq2",
            json!({"value": 280.0}).to_string(),
            (now - Duration::hours(5)).to_rfc3339(),
        ),
    ];
    for (did, stype, sval, created) in sensors {
        sqlx::query(
            "INSERT INTO sensors (device_id, sensor_type, sensor_value, created_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(did)
        .bind(stype)
        .bind(sval)
        .bind(created)
        .execute(pool)
        .await?;
    }

    let detections = [
        (
            cam_ids[0],
            "person",
            0.968_f64,
            "intrusion",
            "uploads/events/event_001.jpg",
            (now - Duration::minutes(15)).to_rfc3339(),
        ),
        (
            cam_ids[2],
            "person",
            0.921_f64,
            "person",
            "uploads/events/event_002.jpg",
            (now - Duration::minutes(35)).to_rfc3339(),
        ),
        (
            cam_ids[0],
            "car",
            0.887_f64,
            "object",
            "uploads/events/event_003.jpg",
            (now - Duration::minutes(70)).to_rfc3339(),
        ),
        (
            cam_ids[1],
            "person",
            0.854_f64,
            "person",
            "uploads/events/event_004.jpg",
            (now - Duration::hours(26)).to_rfc3339(),
        ),
        (
            cam_ids[0],
            "truck",
            0.831_f64,
            "object",
            "uploads/events/event_005.jpg",
            (now - Duration::hours(51)).to_rfc3339(),
        ),
    ];

    let mut det_ids = Vec::new();
    for (cid, obj, conf, etype, img, ts) in detections {
        let row = sqlx::query(
            "INSERT INTO detection_events (camera_id, object_type, confidence, event_type, image_path, event_timestamp, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        )
        .bind(cid)
        .bind(obj)
        .bind(conf)
        .bind(etype)
        .bind(img)
        .bind(&ts)
        .bind(&ts)
        .fetch_one(pool)
        .await?;
        det_ids.push(row_i64(&row, "id"));
    }

    let alerts = [
        (
            Some(det_ids[0]),
            "intrusion",
            "CRITICAL",
            "Phát hiện người xâm nhập tại CAM-01 · Cổng chính",
            "OPEN",
            None::<i64>,
            (now - Duration::minutes(15)).to_rfc3339(),
            None::<String>,
        ),
        (
            Some(det_ids[1]),
            "person",
            "WARNING",
            "Chuyển động tại hành lang CAM-03 · Hành lang A",
            "OPEN",
            None::<i64>,
            (now - Duration::minutes(35)).to_rfc3339(),
            None::<String>,
        ),
        (
            None,
            "camera_offline",
            "WARNING",
            "Camera mất kết nối tại CAM-08 · Kho hàng",
            "OPEN",
            None::<i64>,
            (now - Duration::hours(2)).to_rfc3339(),
            None::<String>,
        ),
        (
            Some(det_ids[2]),
            "vehicle",
            "INFO",
            "Nhận diện phương tiện tại CAM-01 · Cổng chính",
            "RESOLVED",
            admin_id,
            (now - Duration::minutes(70)).to_rfc3339(),
            Some((now - Duration::minutes(50)).to_rfc3339()),
        ),
    ];

    for (did, atype, sev, desc, st, hby, created, handled) in alerts {
        sqlx::query(
            "INSERT INTO alerts (detection_id, alert_type, severity, description, status, handled_by, created_at, handled_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(did)
        .bind(atype)
        .bind(sev)
        .bind(desc)
        .bind(st)
        .bind(hby)
        .bind(created)
        .bind(handled)
        .execute(pool)
        .await?;
    }

    sqlx::query(
        "INSERT INTO device_logs (device_id, command_id, command, result, detail, created_at, completed_at)
         VALUES ($1, $2, 'buzzer_off', 'ACCEPTED', $3, $4, $5)",
    )
    .bind(dev1_id)
    .bind("00000000-0000-4000-8000-000000000001")
    .bind(json!({"status": "accepted", "device_id": "ESP32_001"}).to_string())
    .bind((now - Duration::minutes(45)).to_rfc3339())
    .bind((now - Duration::minutes(45) + Duration::seconds(1)).to_rfc3339())
    .execute(pool)
    .await?;

    let sys_logs = [
        (
            None::<i64>,
            "AI_DETECTION",
            "Phát hiện người tại CAM-01 · Cổng chính · Độ tin cậy 96.8%",
            (now - Duration::minutes(15)).to_rfc3339(),
        ),
        (
            None::<i64>,
            "SENSOR_TELEMETRY",
            "Cập nhật cảm biến MQ-2 (350 ppm) từ thiết bị ESP32_001",
            (now - Duration::minutes(30)).to_rfc3339(),
        ),
        (
            admin_id,
            "ALERT_RESOLVED",
            "Đã xử lý cảnh báo nhận diện phương tiện tại CAM-01",
            (now - Duration::minutes(50)).to_rfc3339(),
        ),
    ];
    for (uid, act, desc, created) in sys_logs {
        sqlx::query(
            "INSERT INTO system_logs (user_id, action, description, created_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(uid)
        .bind(act)
        .bind(desc)
        .bind(created)
        .execute(pool)
        .await?;
    }

    Ok(())
}
