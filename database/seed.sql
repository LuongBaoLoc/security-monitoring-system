-- Dữ liệu demo không bao gồm user/mật khẩu mặc định.
INSERT INTO cameras (name, ip_address, rtsp_url, location, status)
VALUES ('Camera Demo 01', '192.0.2.10', 'rtsp://replace-with-secure-camera-url/stream1', 'Khu vực thử nghiệm', 'UNKNOWN')
ON CONFLICT DO NOTHING;

INSERT INTO devices (device_code, device_name, device_type, status)
VALUES ('ESP32_DEMO_001', 'ESP32 Demo (chưa kết nối)', 'ESP32', 'UNKNOWN')
ON CONFLICT (device_code) DO NOTHING;
