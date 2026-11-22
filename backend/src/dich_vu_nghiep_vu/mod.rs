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

/// Che user/password, query và fragment của RTSP URL trước khi trả cho Frontend.
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

/// RTSP URL có chứa thông tin đăng nhập (user/password) hay không — chỉ trả cờ, không trả giá trị.
pub fn rtsp_has_credentials(rtsp_url: &str) -> bool {
    Url::parse(rtsp_url)
        .map(|u| !u.username().is_empty() || u.password().is_some())
        .unwrap_or(false)
}

/// JSON camera an toàn cho Frontend (RTSP URL đã che thông tin đăng nhập).
pub fn camera_json(camera: &Camera) -> Value {
    json!({
        "id": camera.id,
        "camera_id": camera.id,
        "name": camera.name,
        "ip_address": camera.ip_address,
        "rtsp_url": redact_rtsp_url(&camera.rtsp_url),
        "rtsp_has_credentials": rtsp_has_credentials(&camera.rtsp_url),
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
            "CAM-01 · Nhà kho Hóa chất",
            "192.168.10.21",
            "rtsp://mediamtx:8554/cam_kho_hoa_chat",
            "Nhà kho Khu A",
            "ONLINE",
            (now - Duration::minutes(1)).to_rfc3339(),
            (now - Duration::days(7)).to_rfc3339(),
        ),
        (
            "CAM-02 · Xưởng Sản xuất A",
            "192.168.10.22",
            "rtsp://mediamtx:8554/cam_xuong_san_xuat_a",
            "Phân xưởng 1",
            "ONLINE",
            (now - Duration::minutes(2)).to_rfc3339(),
            (now - Duration::days(7)).to_rfc3339(),
        ),
        (
            "CAM-03 · Tầng hầm Bãi xe B1",
            "192.168.10.23",
            "rtsp://mediamtx:8554/cam_tang_ham_b1",
            "Tầng hầm B1",
            "ONLINE",
            (now - Duration::minutes(3)).to_rfc3339(),
            (now - Duration::days(6)).to_rfc3339(),
        ),
        (
            "CAM-04 · Phòng Kỹ thuật Điện",
            "192.168.10.24",
            "rtsp://mediamtx:8554/cam_phong_dien",
            "Tầng kỹ thuật",
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
         VALUES ($1, $2, 'ESP32', 'ONLINE', $3, FALSE, 340.0, FALSE, FALSE, TRUE, $4) RETURNING id",
    )
    .bind("ESP32_KHO_01")
    .bind("ESP32 PCCC · Nhà kho Khu A")
    .bind((now - Duration::seconds(25)).to_rfc3339())
    .bind((now - Duration::days(7)).to_rfc3339())
    .fetch_one(pool)
    .await?;
    let dev1_id = row_i64(&dev1, "id");

    let dev2 = sqlx::query(
        "INSERT INTO devices (device_code, device_name, device_type, status, last_seen, latest_pir, latest_mq2, relay, buzzer, lock_engaged, created_at)
         VALUES ($1, $2, 'ESP32', 'ONLINE', $3, TRUE, 720.0, TRUE, TRUE, FALSE, $4) RETURNING id",
    )
    .bind("ESP32_BAIXE_B1")
    .bind("ESP32 PCCC · Tầng hầm Bãi xe B1")
    .bind((now - Duration::seconds(40)).to_rfc3339())
    .bind((now - Duration::days(6)).to_rfc3339())
    .fetch_one(pool)
    .await?;
    let dev2_id = row_i64(&dev2, "id");

    let sensors = [
        (
            dev1_id,
            "mq2",
            json!({"value": 340.0, "unit": "ppm"}).to_string(),
            (now - Duration::minutes(5)).to_rfc3339(),
        ),
        (
            dev1_id,
            "lock",
            json!({"value": true, "label": "solenoid_locked"}).to_string(),
            (now - Duration::minutes(5)).to_rfc3339(),
        ),
        (
            dev2_id,
            "mq2",
            json!({"value": 720.0, "unit": "ppm"}).to_string(),
            (now - Duration::minutes(2)).to_rfc3339(),
        ),
        (
            dev2_id,
            "relay",
            json!({"value": true, "label": "pump_sprinkler_on"}).to_string(),
            (now - Duration::minutes(2)).to_rfc3339(),
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
            "fire",
            0.962_f64,
            "fire",
            "uploads/events/fire_kho_a_001.jpg",
            (now - Duration::minutes(12)).to_rfc3339(),
        ),
        (
            cam_ids[2],
            "smoke",
            0.894_f64,
            "smoke",
            "uploads/events/smoke_bai_xe_002.jpg",
            (now - Duration::minutes(28)).to_rfc3339(),
        ),
        (
            cam_ids[1],
            "smoke",
            0.845_f64,
            "smoke",
            "uploads/events/smoke_xuong_a_003.jpg",
            (now - Duration::minutes(75)).to_rfc3339(),
        ),
        (
            cam_ids[0],
            "fire",
            0.918_f64,
            "fire",
            "uploads/events/fire_kho_a_004.jpg",
            (now - Duration::hours(26)).to_rfc3339(),
        ),
        (
            cam_ids[2],
            "smoke",
            0.812_f64,
            "smoke",
            "uploads/events/smoke_bai_xe_005.jpg",
            (now - Duration::hours(50)).to_rfc3339(),
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
            "fire",
            "CRITICAL",
            "BÁO CHÁY KHẨN CẤP: YOLOv8 phát hiện Lửa (96.2%) tại CAM-01 · Nhà kho Hóa chất",
            "OPEN",
            None::<i64>,
            (now - Duration::minutes(12)).to_rfc3339(),
            None::<String>,
        ),
        (
            Some(det_ids[1]),
            "smoke",
            "WARNING",
            "Cảnh báo sớm: YOLOv8 phát hiện Khói (89.4%) tại CAM-03 · Tầng hầm Bãi xe B1",
            "OPEN",
            None::<i64>,
            (now - Duration::minutes(28)).to_rfc3339(),
            None::<String>,
        ),
        (
            None,
            "gas_mq2",
            "CRITICAL",
            "Cảm biến khói/gas MQ-2 vượt ngưỡng (720 ppm) tại ESP32_BAIXE_B1 · Tầng hầm Bãi xe B1",
            "OPEN",
            None::<i64>,
            (now - Duration::minutes(2)).to_rfc3339(),
            None::<String>,
        ),
        (
            Some(det_ids[2]),
            "smoke",
            "WARNING",
            "Phát hiện Khói nhẹ (84.5%) tại CAM-02 · Xưởng Sản xuất A (đã kiểm tra an toàn)",
            "RESOLVED",
            admin_id,
            (now - Duration::minutes(75)).to_rfc3339(),
            Some((now - Duration::minutes(55)).to_rfc3339()),
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
         VALUES ($1, $2, 'unlock', 'ACCEPTED', $3, $4, $5)",
    )
    .bind(dev2_id)
    .bind("00000000-0000-4000-8000-000000000001")
    .bind(json!({"status": "accepted", "device_id": "ESP32_BAIXE_B1", "hardware": "solenoid_exit_door"}).to_string())
    .bind((now - Duration::minutes(2)).to_rfc3339())
    .bind((now - Duration::minutes(2) + Duration::seconds(1)).to_rfc3339())
    .execute(pool)
    .await?;

    let sys_logs = [
        (
            None::<i64>,
            "AI_DETECTION",
            "YOLOv8 phát hiện Lửa (fire · 96.2%) tại CAM-01 · Nhà kho Hóa chất",
            (now - Duration::minutes(12)).to_rfc3339(),
        ),
        (
            None::<i64>,
            "SENSOR_TELEMETRY",
            "Cảnh báo nồng độ khói/gas MQ-2 (720 ppm) từ mạch ESP32_BAIXE_B1",
            (now - Duration::minutes(2)).to_rfc3339(),
        ),
        (
            admin_id,
            "ALERT_RESOLVED",
            "Trực ban PCCC xác nhận xử lý cảnh báo Khói tại CAM-02 · Xưởng Sản xuất A",
            (now - Duration::minutes(55)).to_rfc3339(),
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
