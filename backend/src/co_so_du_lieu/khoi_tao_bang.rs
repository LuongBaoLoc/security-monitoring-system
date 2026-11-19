//! Tự động tạo các bảng SQL khi khởi động (init_schema).

use sqlx::AnyPool;

pub async fn init_schema(pool: &AnyPool, is_postgres: bool) -> Result<(), sqlx::Error> {
    let pk = if is_postgres {
        "BIGSERIAL PRIMARY KEY"
    } else {
        "INTEGER PRIMARY KEY AUTOINCREMENT"
    };
    let float_ty = if is_postgres {
        "DOUBLE PRECISION"
    } else {
        "REAL"
    };

    let queries = vec![
        format!(
            "CREATE TABLE IF NOT EXISTS users (
                id {pk},
                username VARCHAR(80) NOT NULL UNIQUE,
                password_hash VARCHAR(255) NOT NULL,
                full_name VARCHAR(160) NOT NULL,
                role VARCHAR(20) NOT NULL DEFAULT 'OPERATOR',
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                created_at TEXT NOT NULL
            )"
        ),
        format!(
            "CREATE TABLE IF NOT EXISTS cameras (
                id {pk},
                name VARCHAR(120) NOT NULL,
                ip_address VARCHAR(64),
                rtsp_url TEXT NOT NULL,
                location VARCHAR(255),
                status VARCHAR(20) NOT NULL DEFAULT 'UNKNOWN',
                last_seen TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )"
        ),
        format!(
            "CREATE TABLE IF NOT EXISTS devices (
                id {pk},
                device_code VARCHAR(100) NOT NULL UNIQUE,
                device_name VARCHAR(120) NOT NULL,
                device_type VARCHAR(60) NOT NULL DEFAULT 'ESP32',
                status VARCHAR(20) NOT NULL DEFAULT 'UNKNOWN',
                last_seen TEXT,
                latest_pir BOOLEAN,
                latest_mq2 {float_ty},
                relay BOOLEAN,
                buzzer BOOLEAN,
                lock_engaged BOOLEAN,
                created_at TEXT NOT NULL
            )"
        ),
        format!(
            "CREATE TABLE IF NOT EXISTS sensors (
                id {pk},
                device_id BIGINT NOT NULL REFERENCES devices(id),
                sensor_type VARCHAR(60) NOT NULL,
                sensor_value TEXT NOT NULL,
                created_at TEXT NOT NULL
            )"
        ),
        format!(
            "CREATE TABLE IF NOT EXISTS detection_events (
                id {pk},
                camera_id BIGINT NOT NULL REFERENCES cameras(id),
                object_type VARCHAR(80) NOT NULL,
                confidence {float_ty} NOT NULL,
                event_type VARCHAR(80) NOT NULL,
                image_path TEXT,
                event_timestamp TEXT NOT NULL,
                created_at TEXT NOT NULL
            )"
        ),
        format!(
            "CREATE TABLE IF NOT EXISTS alerts (
                id {pk},
                detection_id BIGINT REFERENCES detection_events(id),
                alert_type VARCHAR(80) NOT NULL,
                severity VARCHAR(20) NOT NULL DEFAULT 'WARNING',
                description TEXT NOT NULL,
                status VARCHAR(20) NOT NULL DEFAULT 'OPEN',
                handled_by BIGINT REFERENCES users(id),
                created_at TEXT NOT NULL,
                handled_at TEXT
            )"
        ),
        format!(
            "CREATE TABLE IF NOT EXISTS device_logs (
                id {pk},
                device_id BIGINT NOT NULL REFERENCES devices(id),
                command_id VARCHAR(36) NOT NULL UNIQUE,
                command VARCHAR(80) NOT NULL,
                result VARCHAR(30) NOT NULL DEFAULT 'PENDING',
                detail TEXT,
                created_at TEXT NOT NULL,
                completed_at TEXT
            )"
        ),
        format!(
            "CREATE TABLE IF NOT EXISTS system_logs (
                id {pk},
                user_id BIGINT REFERENCES users(id),
                action VARCHAR(100) NOT NULL,
                description TEXT NOT NULL,
                created_at TEXT NOT NULL
            )"
        ),
    ];

    for q in queries {
        sqlx::query(&q).execute(pool).await?;
    }

    if is_postgres {
        let migrations = [
            "ALTER TABLE users ALTER COLUMN created_at TYPE TEXT USING created_at::TEXT",
            "ALTER TABLE cameras ALTER COLUMN last_seen TYPE TEXT USING last_seen::TEXT",
            "ALTER TABLE cameras ALTER COLUMN created_at TYPE TEXT USING created_at::TEXT",
            "ALTER TABLE cameras ALTER COLUMN updated_at TYPE TEXT USING updated_at::TEXT",
            "ALTER TABLE devices ALTER COLUMN last_seen TYPE TEXT USING last_seen::TEXT",
            "ALTER TABLE devices ALTER COLUMN created_at TYPE TEXT USING created_at::TEXT",
            "ALTER TABLE sensors ALTER COLUMN sensor_value TYPE TEXT USING sensor_value::TEXT",
            "ALTER TABLE sensors ALTER COLUMN created_at TYPE TEXT USING created_at::TEXT",
            "ALTER TABLE detection_events ALTER COLUMN event_timestamp TYPE TEXT USING event_timestamp::TEXT",
            "ALTER TABLE detection_events ALTER COLUMN created_at TYPE TEXT USING created_at::TEXT",
            "ALTER TABLE alerts ALTER COLUMN created_at TYPE TEXT USING created_at::TEXT",
            "ALTER TABLE alerts ALTER COLUMN handled_at TYPE TEXT USING handled_at::TEXT",
            "ALTER TABLE device_logs ALTER COLUMN detail TYPE TEXT USING detail::TEXT",
            "ALTER TABLE device_logs ALTER COLUMN created_at TYPE TEXT USING created_at::TEXT",
            "ALTER TABLE device_logs ALTER COLUMN completed_at TYPE TEXT USING completed_at::TEXT",
            "ALTER TABLE system_logs ALTER COLUMN created_at TYPE TEXT USING created_at::TEXT",
        ];
        for m in migrations {
            let _ = sqlx::query(m).execute(pool).await;
        }
    }

    Ok(())
}
