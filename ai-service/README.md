# AI Detection Service — Stream Processor YOLOv8 Lửa & Khói (Rust)

Dịch vụ Edge AI viết bằng **Rust (`axum`, `tokio`, `tract-onnx`, `image`, `reqwest`)** chuyên nhận diện **Lửa (`fire`)** và **Khói (`smoke`)** thời gian thực cho hệ thống **Phòng cháy Chữa cháy thông minh (PCCC)**. Chạy trực tiếp tại biên trên **Jetson Nano**, **PC có GPU Nvidia GTX 1650+** hoặc **CPU** mà không cần môi trường Python nặng.

## Cấu trúc Thư mục

- `Cargo.toml`: Khai báo crate `security-monitoring-ai-service` (`tract-onnx`, `url`, `chrono`) và CLI kiểm thử `test_yolo_sample`.
- `src/config.rs`: Đọc biến môi trường (`MODEL_PATH`, `YOLO_CLASSES=fire,smoke`, `CONFIDENCE_THRESHOLD`, `NMS_IOU_THRESHOLD`, `FFMPEG_TIMEOUT_SECS`, `RTSP_SERVER_URL`).
- `src/main.rs`: HTTP server cổng `8001` cung cấp `GET /health` và `POST /detect?camera_id=...` (xác thực `AI_SERVICE_TOKEN`, chạy suy luận ONNX trên `spawn_blocking`, tự động upload ảnh snapshot và gửi sự kiện cháy/khói sang Backend).
- `src/camera.rs`: Trích xuất khung hình JPEG từ Camera IP RTSP / **RTSP Simple Server (MediaMTX)** / Webcam USB qua `ffmpeg` kèm giới hạn thời gian (`FFMPEG_TIMEOUT_SECS`) và tự động che `user:password` trong mọi log lỗi (`redact_rtsp_url`, `sanitize_rtsp_log`).
- `src/detector.rs`: Suy luận **YOLOv8 Nano / Small (ONNX)** qua `tract-onnx` (letterbox 640×640, lọc ngưỡng tin cậy, Non-Maximum Suppression theo lớp, ánh xạ nhãn `fire` / `smoke`).
- `src/event_handler.rs`: Lấy ánh xạ RTSP gốc qua `GET /api/internal/cameras/mappings`, tải ảnh snapshot lên `POST /api/detections/snapshot` và gửi sự kiện đầy đủ về `POST /api/detections`.
- `src/bin/test_yolo_sample.rs`: Công cụ CLI kiểm thử trọn luồng ảnh/RTSP/MediaMTX → YOLOv8 ONNX → upload snapshot → Backend API.

## Chuẩn bị Model & Chạy thử bằng Cargo

```bash
# Xuất model YOLOv8 Lửa & Khói sang định dạng ONNX (640x640) và đặt vào models/
yolo export model=yolov8n_fire_smoke.pt format=onnx imgsz=640
```

```powershell
cd ai-service

# Chạy AI Service HTTP Server trên cổng 8001
cargo run --bin security-monitoring-ai-service

# Kiểm thử trọn luồng ảnh cháy/khói -> YOLOv8 ONNX -> Upload snapshot -> Backend API
cargo run --bin test_yolo_sample -- --image path/to/fire_sample.jpg --camera-id 1 --submit

# Lấy RTSP nội bộ từ Backend (hoặc truyền --rtsp rtsp://mediamtx:8554/cam1) rồi gửi sự kiện
cargo run --bin test_yolo_sample -- --fetch-mapping --camera-id 1 --submit
```
