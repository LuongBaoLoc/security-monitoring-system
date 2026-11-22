-- Dữ liệu mẫu PCCC (Tuần 4) cho kịch bản Tòa nhà / Khu công nghiệp / Nhà kho / Bãi xe thông minh.
-- Không chứa tài khoản/mật khẩu mặc định dạng rõ; tài khoản Admin khởi tạo qua BOOTSTRAP_ADMIN_* trong .env.

INSERT INTO cameras (id, name, ip_address, rtsp_url, location, status, last_seen)
VALUES
    (1, 'CAM-01 · Nhà kho Hóa chất', '192.168.10.21', 'rtsp://mediamtx:8554/cam_kho_hoa_chat', 'Nhà kho Khu A', 'ONLINE', NOW() - INTERVAL '1 minute'),
    (2, 'CAM-02 · Xưởng Sản xuất A', '192.168.10.22', 'rtsp://mediamtx:8554/cam_xuong_san_xuat_a', 'Phân xưởng 1', 'ONLINE', NOW() - INTERVAL '2 minutes'),
    (3, 'CAM-03 · Tầng hầm Bãi xe B1', '192.168.10.23', 'rtsp://mediamtx:8554/cam_tang_ham_b1', 'Tầng hầm B1', 'ONLINE', NOW() - INTERVAL '3 minutes'),
    (4, 'CAM-04 · Phòng Kỹ thuật Điện', '192.168.10.24', 'rtsp://mediamtx:8554/cam_phong_dien', 'Tầng kỹ thuật', 'OFFLINE', NOW() - INTERVAL '2 hours')
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('cameras', 'id'), COALESCE((SELECT MAX(id) FROM cameras), 1), true);

-- Thiết bị ESP32 PCCC: MQ-2 khói/gas, cảm biến lửa, Mạch Relay bơm/phun sương, Còi 12V, Khóa từ Solenoid cửa thoát hiểm
INSERT INTO devices (id, device_code, device_name, device_type, status, last_seen, latest_pir, latest_mq2, relay, buzzer, lock_engaged)
VALUES
    (1, 'ESP32_KHO_01', 'ESP32 PCCC · Nhà kho Khu A', 'ESP32', 'ONLINE', NOW() - INTERVAL '25 seconds', FALSE, 340.0, FALSE, FALSE, TRUE),
    (2, 'ESP32_BAIXE_B1', 'ESP32 PCCC · Tầng hầm Bãi xe B1', 'ESP32', 'ONLINE', NOW() - INTERVAL '40 seconds', TRUE, 720.0, TRUE, TRUE, FALSE),
    (3, 'ESP32_DIEN_01', 'ESP32 PCCC · Phòng Kỹ thuật Điện', 'ESP32', 'OFFLINE', NULL, NULL, NULL, NULL, NULL, NULL)
ON CONFLICT (device_code) DO NOTHING;

SELECT setval(pg_get_serial_sequence('devices', 'id'), COALESCE((SELECT MAX(id) FROM devices), 1), true);

INSERT INTO sensors (id, device_id, sensor_type, sensor_value, created_at)
VALUES
    (1, 1, 'mq2', '{"value": 340, "unit": "ppm"}'::jsonb, NOW() - INTERVAL '5 minutes'),
    (2, 1, 'lock', '{"value": true, "label": "solenoid_locked"}'::jsonb, NOW() - INTERVAL '5 minutes'),
    (3, 2, 'mq2', '{"value": 720, "unit": "ppm"}'::jsonb, NOW() - INTERVAL '2 minutes'),
    (4, 2, 'relay', '{"value": true, "label": "pump_sprinkler_on"}'::jsonb, NOW() - INTERVAL '2 minutes'),
    (5, 2, 'buzzer', '{"value": true, "label": "siren_12v_on"}'::jsonb, NOW() - INTERVAL '2 minutes'),
    (6, 2, 'lock', '{"value": false, "label": "solenoid_emergency_unlocked"}'::jsonb, NOW() - INTERVAL '2 minutes')
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('sensors', 'id'), COALESCE((SELECT MAX(id) FROM sensors), 1), true);

-- Sự kiện nhận diện YOLOv8 chuyên PCCC (Lửa fire & Khói smoke)
INSERT INTO detection_events (id, camera_id, object_type, confidence, event_type, image_path, event_timestamp)
VALUES
    (1, 1, 'fire', 0.962, 'fire', 'uploads/events/fire_kho_a_001.jpg', NOW() - INTERVAL '12 minutes'),
    (2, 3, 'smoke', 0.894, 'smoke', 'uploads/events/smoke_bai_xe_002.jpg', NOW() - INTERVAL '28 minutes'),
    (3, 2, 'smoke', 0.845, 'smoke', 'uploads/events/smoke_xuong_a_003.jpg', NOW() - INTERVAL '75 minutes'),
    (4, 1, 'fire', 0.918, 'fire', 'uploads/events/fire_kho_a_004.jpg', NOW() - INTERVAL '26 hours'),
    (5, 3, 'smoke', 0.812, 'smoke', 'uploads/events/smoke_bai_xe_005.jpg', NOW() - INTERVAL '50 hours')
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('detection_events', 'id'), COALESCE((SELECT MAX(id) FROM detection_events), 1), true);

-- Cảnh báo PCCC: Lửa (CRITICAL), Khói (WARNING), MQ-2 vượt ngưỡng (CRITICAL), Camera mất kết nối
INSERT INTO alerts (id, detection_id, alert_type, severity, description, status, created_at, handled_at)
VALUES
    (1, 1, 'fire', 'CRITICAL', 'BÁO CHÁY KHẨN CẤP: YOLOv8 phát hiện Lửa (96.2%) tại CAM-01 · Nhà kho Hóa chất', 'OPEN', NOW() - INTERVAL '12 minutes', NULL),
    (2, 2, 'smoke', 'WARNING', 'Cảnh báo sớm: YOLOv8 phát hiện Khói (89.4%) tại CAM-03 · Tầng hầm Bãi xe B1', 'OPEN', NOW() - INTERVAL '28 minutes', NULL),
    (3, NULL, 'gas_mq2', 'CRITICAL', 'Cảm biến khói/gas MQ-2 vượt ngưỡng (720 ppm) tại ESP32_BAIXE_B1 · Tầng hầm Bãi xe B1', 'OPEN', NOW() - INTERVAL '2 minutes', NULL),
    (4, 3, 'smoke', 'WARNING', 'Phát hiện Khói nhẹ (84.5%) tại CAM-02 · Xưởng Sản xuất A (đã kiểm tra an toàn)', 'RESOLVED', NOW() - INTERVAL '75 minutes', NOW() - INTERVAL '55 minutes')
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('alerts', 'id'), COALESCE((SELECT MAX(id) FROM alerts), 1), true);

-- Nhật ký điều khiển ứng cứu PCCC đã xác nhận ACK từ ESP32
INSERT INTO device_logs (id, device_id, command_id, command, result, detail, created_at, completed_at)
VALUES
    (1, 2, '00000000-0000-4000-8000-000000000001', 'buzzer_on', 'ACCEPTED', '{"status": "accepted", "device_id": "ESP32_BAIXE_B1", "hardware": "siren_12v"}'::jsonb, NOW() - INTERVAL '2 minutes', NOW() - INTERVAL '1 minute 59 seconds'),
    (2, 2, '00000000-0000-4000-8000-000000000002', 'unlock', 'ACCEPTED', '{"status": "accepted", "device_id": "ESP32_BAIXE_B1", "hardware": "solenoid_exit_door"}'::jsonb, NOW() - INTERVAL '2 minutes', NOW() - INTERVAL '1 minute 58 seconds'),
    (3, 2, '00000000-0000-4000-8000-000000000003', 'relay_on', 'ACCEPTED', '{"status": "accepted", "device_id": "ESP32_BAIXE_B1", "hardware": "relay_pump_sprinkler"}'::jsonb, NOW() - INTERVAL '2 minutes', NOW() - INTERVAL '1 minute 58 seconds')
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('device_logs', 'id'), COALESCE((SELECT MAX(id) FROM device_logs), 1), true);

INSERT INTO system_logs (id, user_id, action, description, created_at)
VALUES
    (1, NULL, 'AI_DETECTION', 'YOLOv8 phát hiện Lửa (fire · 96.2%) tại CAM-01 · Nhà kho Hóa chất', NOW() - INTERVAL '12 minutes'),
    (2, NULL, 'SENSOR_TELEMETRY', 'Cảnh báo nồng độ khói/gas MQ-2 = 720 ppm từ mạch ESP32_BAIXE_B1', NOW() - INTERVAL '2 minutes'),
    (3, NULL, 'DEVICE_COMMAND', 'Kích hoạt Còi 12V, Relay bơm chữa cháy và mở Khóa từ Solenoid thoát hiểm tại ESP32_BAIXE_B1 (ACK OK)', NOW() - INTERVAL '2 minutes'),
    (4, NULL, 'ALERT_RESOLVED', 'Trực ban PCCC xác nhận xử lý cảnh báo Khói tại CAM-02 · Xưởng Sản xuất A', NOW() - INTERVAL '55 minutes')
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('system_logs', 'id'), COALESCE((SELECT MAX(id) FROM system_logs), 1), true);
