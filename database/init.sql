-- Bootstrap schema cho PostgreSQL mới. Schema chạy thật nên được tiến hóa bằng Alembic.
CREATE TABLE IF NOT EXISTS users (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(80) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    full_name VARCHAR(160) NOT NULL,
    role VARCHAR(20) NOT NULL DEFAULT 'OPERATOR' CHECK (role IN ('ADMIN', 'OPERATOR')),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS cameras (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(120) NOT NULL,
    ip_address INET,
    rtsp_url TEXT NOT NULL,
    location VARCHAR(255),
    status VARCHAR(20) NOT NULL DEFAULT 'UNKNOWN' CHECK (status IN ('ONLINE', 'OFFLINE', 'UNKNOWN')),
    last_seen TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS ix_cameras_status ON cameras(status);

CREATE TABLE IF NOT EXISTS devices (
    id BIGSERIAL PRIMARY KEY,
    device_code VARCHAR(100) NOT NULL UNIQUE,
    device_name VARCHAR(120) NOT NULL,
    device_type VARCHAR(60) NOT NULL DEFAULT 'ESP32',
    status VARCHAR(20) NOT NULL DEFAULT 'UNKNOWN' CHECK (status IN ('ONLINE', 'OFFLINE', 'UNKNOWN')),
    last_seen TIMESTAMPTZ,
    latest_pir BOOLEAN,
    latest_mq2 DOUBLE PRECISION CHECK (latest_mq2 IS NULL OR latest_mq2 >= 0),
    relay BOOLEAN,
    buzzer BOOLEAN,
    lock_engaged BOOLEAN,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS ix_devices_status ON devices(status);

CREATE TABLE IF NOT EXISTS sensors (
    id BIGSERIAL PRIMARY KEY,
    device_id BIGINT NOT NULL REFERENCES devices(id) ON DELETE RESTRICT,
    sensor_type VARCHAR(60) NOT NULL,
    sensor_value JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS ix_sensors_device_type_time ON sensors(device_id, sensor_type, created_at DESC);

CREATE TABLE IF NOT EXISTS detection_events (
    id BIGSERIAL PRIMARY KEY,
    camera_id BIGINT NOT NULL REFERENCES cameras(id) ON DELETE RESTRICT,
    object_type VARCHAR(80) NOT NULL,
    confidence REAL NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    event_type VARCHAR(80) NOT NULL,
    image_path TEXT,
    event_timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS ix_detections_camera_time ON detection_events(camera_id, event_timestamp DESC);
CREATE INDEX IF NOT EXISTS ix_detections_type_time ON detection_events(object_type, event_timestamp DESC);

CREATE TABLE IF NOT EXISTS alerts (
    id BIGSERIAL PRIMARY KEY,
    detection_id BIGINT REFERENCES detection_events(id) ON DELETE RESTRICT,
    alert_type VARCHAR(80) NOT NULL,
    severity VARCHAR(20) NOT NULL CHECK (severity IN ('INFO', 'WARNING', 'CRITICAL')),
    description TEXT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN', 'RESOLVED', 'ARCHIVED')),
    handled_by BIGINT REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    handled_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS ix_alerts_status_time ON alerts(status, created_at DESC);
CREATE INDEX IF NOT EXISTS ix_alerts_severity_time ON alerts(severity, created_at DESC);
CREATE INDEX IF NOT EXISTS ix_alerts_detection ON alerts(detection_id);

CREATE TABLE IF NOT EXISTS device_logs (
    id BIGSERIAL PRIMARY KEY,
    device_id BIGINT NOT NULL REFERENCES devices(id) ON DELETE RESTRICT,
    command_id UUID UNIQUE,
    command VARCHAR(80) NOT NULL,
    result VARCHAR(30) NOT NULL DEFAULT 'PENDING' CHECK (result IN ('PENDING', 'ACCEPTED', 'REJECTED', 'TIMEOUT', 'ERROR')),
    detail JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS ix_device_logs_device_time ON device_logs(device_id, created_at DESC);

CREATE TABLE IF NOT EXISTS system_logs (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
    action VARCHAR(100) NOT NULL,
    description TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS ix_system_logs_time ON system_logs(created_at DESC);
CREATE INDEX IF NOT EXISTS ix_system_logs_user_time ON system_logs(user_id, created_at DESC);
