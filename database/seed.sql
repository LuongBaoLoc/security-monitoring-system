-- Dữ liệu mẫu khởi tạo (Tuần 4) để kiểm tra schema và giao diện tổng quan.
-- Không chứa tài khoản/mật khẩu mặc định dạng rõ; tài khoản Admin tạo qua biến môi trường BOOTSTRAP_ADMIN_*.

INSERT INTO cameras (id, name, ip_address, rtsp_url, location, status, last_seen)
VALUES
    (1, 'CAM-01 · Cổng chính', '192.168.1.21', 'rtsp://replace-with-secure-camera-url/stream1', 'Cổng phía Đông', 'ONLINE', NOW() - INTERVAL '1 minute'),
    (2, 'CAM-02 · Sảnh lễ tân', '192.168.1.22', 'rtsp://replace-with-secure-camera-url/stream2', 'Tầng trệt', 'ONLINE', NOW() - INTERVAL '2 minutes'),
    (3, 'CAM-03 · Hành lang A', '192.168.1.23', 'rtsp://replace-with-secure-camera-url/stream3', 'Tầng 1', 'ONLINE', NOW() - INTERVAL '3 minutes'),
    (4, 'CAM-08 · Kho hàng', '192.168.1.28', 'rtsp://replace-with-secure-camera-url/stream8', 'Khu nhà kho', 'OFFLINE', NOW() - INTERVAL '2 hours')
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('cameras', 'id'), COALESCE((SELECT MAX(id) FROM cameras), 1), true);

INSERT INTO devices (id, device_code, device_name, device_type, status, last_seen, latest_pir, latest_mq2, relay, buzzer, lock_engaged)
VALUES
    (1, 'ESP32_001', 'ESP32 khu vực A · Tầng 1', 'ESP32', 'ONLINE', NOW() - INTERVAL '30 seconds', FALSE, 350.0, FALSE, FALSE, TRUE),
    (2, 'ESP32_002', 'ESP32 khu vực B · Kho hàng', 'ESP32', 'OFFLINE', NOW() - INTERVAL '5 hours', FALSE, 280.0, FALSE, FALSE, TRUE),
    (3, 'ESP32_DEMO_001', 'ESP32 Demo (chưa kết nối)', 'ESP32', 'UNKNOWN', NULL, NULL, NULL, NULL, NULL, NULL)
ON CONFLICT (device_code) DO NOTHING;

SELECT setval(pg_get_serial_sequence('devices', 'id'), COALESCE((SELECT MAX(id) FROM devices), 1), true);

INSERT INTO sensors (id, device_id, sensor_type, sensor_value, created_at)
VALUES
    (1, 1, 'pir', '{"value": false}'::jsonb, NOW() - INTERVAL '5 minutes'),
    (2, 1, 'mq2', '{"value": 350}'::jsonb, NOW() - INTERVAL '5 minutes'),
    (3, 1, 'lock', '{"value": true}'::jsonb, NOW() - INTERVAL '5 minutes'),
    (4, 2, 'mq2', '{"value": 280}'::jsonb, NOW() - INTERVAL '5 hours')
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('sensors', 'id'), COALESCE((SELECT MAX(id) FROM sensors), 1), true);

INSERT INTO detection_events (id, camera_id, object_type, confidence, event_type, image_path, event_timestamp)
VALUES
    (1, 1, 'person', 0.968, 'intrusion', 'uploads/events/event_001.jpg', NOW() - INTERVAL '15 minutes'),
    (2, 3, 'person', 0.921, 'person', 'uploads/events/event_002.jpg', NOW() - INTERVAL '35 minutes'),
    (3, 1, 'car', 0.887, 'object', 'uploads/events/event_003.jpg', NOW() - INTERVAL '70 minutes'),
    (4, 2, 'person', 0.854, 'person', 'uploads/events/event_004.jpg', NOW() - INTERVAL '26 hours'),
    (5, 1, 'truck', 0.831, 'object', 'uploads/events/event_005.jpg', NOW() - INTERVAL '51 hours')
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('detection_events', 'id'), COALESCE((SELECT MAX(id) FROM detection_events), 1), true);

INSERT INTO alerts (id, detection_id, alert_type, severity, description, status, created_at, handled_at)
VALUES
    (1, 1, 'intrusion', 'CRITICAL', 'Phát hiện người xâm nhập tại CAM-01 · Cổng chính', 'OPEN', NOW() - INTERVAL '15 minutes', NULL),
    (2, 2, 'person', 'WARNING', 'Chuyển động tại hành lang CAM-03 · Hành lang A', 'OPEN', NOW() - INTERVAL '35 minutes', NULL),
    (3, NULL, 'camera_offline', 'WARNING', 'Camera mất kết nối tại CAM-08 · Kho hàng', 'OPEN', NOW() - INTERVAL '2 hours', NULL),
    (4, 3, 'vehicle', 'INFO', 'Nhận diện phương tiện tại CAM-01 · Cổng chính', 'RESOLVED', NOW() - INTERVAL '70 minutes', NOW() - INTERVAL '50 minutes')
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('alerts', 'id'), COALESCE((SELECT MAX(id) FROM alerts), 1), true);

INSERT INTO device_logs (id, device_id, command_id, command, result, detail, created_at, completed_at)
VALUES
    (1, 1, '00000000-0000-4000-8000-000000000001', 'buzzer_off', 'ACCEPTED', '{"status": "accepted", "device_id": "ESP32_001"}'::jsonb, NOW() - INTERVAL '45 minutes', NOW() - INTERVAL '44 minutes 59 seconds')
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('device_logs', 'id'), COALESCE((SELECT MAX(id) FROM device_logs), 1), true);

INSERT INTO system_logs (id, user_id, action, description, created_at)
VALUES
    (1, NULL, 'AI_DETECTION', 'Phát hiện người tại CAM-01 · Cổng chính · Độ tin cậy 96.8%', NOW() - INTERVAL '15 minutes'),
    (2, NULL, 'SENSOR_TELEMETRY', 'Cập nhật cảm biến MQ-2 (350 ppm) từ thiết bị ESP32_001', NOW() - INTERVAL '30 minutes'),
    (3, NULL, 'ALERT_RESOLVED', 'Đã xử lý cảnh báo nhận diện phương tiện tại CAM-01', NOW() - INTERVAL '50 minutes')
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('system_logs', 'id'), COALESCE((SELECT MAX(id) FROM system_logs), 1), true);
