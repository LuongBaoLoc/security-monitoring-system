# AI Detection Service (Rust - Axum)

Dịch vụ AI viết bằng **Rust (`axum`, `tokio`, `image`, `reqwest`)** nhận khung hình tại `POST /detect?camera_id=...`, xử lý nhận diện đối tượng (YOLO pipeline), sau đó gửi các sự kiện phát hiện về Backend qua `POST /api/detections`.

## Cấu trúc thư mục

- `Cargo.toml`: Khai báo crate `security-monitoring-ai-service` và binary CLI `test_yolo_sample`.
- `src/main.rs`: Axum HTTP server cung cấp endpoint `GET /health` và `POST /detect`.
- `src/camera.rs`: Hàm chụp 1 khung hình JPEG từ file/luồng RTSP (`capture_frame_bytes`) và sinh ảnh mẫu thử nghiệm (`create_sample_test_image`).
- `src/detector.rs`: Giải mã ảnh và trích xuất danh sách đối tượng (`object_type`, `confidence`, `event_type`).
- `src/event_handler.rs`: Gửi JSON sự kiện phát hiện sang Backend kèm `AI_SERVICE_TOKEN`.
- `src/bin/test_yolo_sample.rs`: Binary CLI kiểm thử đọc frame và chạy inference YOLO trên ảnh mẫu (đầu ra Tuần 3 – Tuần 4).

## Cài đặt & chạy thử bằng Cargo

```powershell
cd ai-service

# Chạy dịch vụ AI HTTP Server trên cổng 8001
cargo run --bin security-monitoring-ai-service

# Chạy thử nghiệm pipeline YOLO với ảnh mẫu tự sinh (hoặc truyền --image / --rtsp)
cargo run --bin test_yolo_sample
cargo run --bin test_yolo_sample -- --image path/to/sample.jpg
cargo run --bin test_yolo_sample -- --rtsp rtsp://192.168.1.21:554/stream1
```
