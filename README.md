# Security Monitoring System (Rust Edition)

Nền tảng giám sát an ninh tích hợp **Backend API (Rust / Axum / SQLx)**, **Web Dashboard**, **AI Inference Service (Rust / Axum / Image)**, **PostgreSQL/SQLite** và **Firmware ESP32 (Rust / `esp-idf-svc`)** qua MQTT.

## Thành phần và ranh giới

- **Backend (`backend/` — Rust Axum + SQLx + Tokio + Rumqttc)**: REST API, xác thực/phân quyền JWT + Bcrypt, persistence (PostgreSQL & SQLite), tiếp nhận sự kiện AI, điều phối lệnh thiết bị MQTT và WebSocket dashboard (`/ws/dashboard`).
- **PostgreSQL / SQLite**: dữ liệu bền vững (người dùng, camera, thiết bị, cảm biến, sự kiện AI, cảnh báo, nhật ký). Schema được khởi tạo tự động khi backend khởi động; `database/init.sql` là tài liệu tham chiếu SQL.
- **Frontend (`frontend/` — HTML/CSS/JS)**: dashboard trình duyệt gọi REST/WebSocket. Không kết nối trực tiếp cơ sở dữ liệu hoặc MQTT broker.
- **AI Service (`ai-service/` — Rust Axum)**: nhận khung hình tại `POST /detect?camera_id=...`, xử lý nhận diện đối tượng và gửi sự kiện phát hiện đến Backend qua `POST /api/detections`. Đi kèm công cụ CLI `test_yolo_sample` viết bằng Rust.
- **ESP32 Firmware (`iot-service/esp32/` — Rust `esp-idf-svc` + `esp-idf-hal`)**: đọc cảm biến PIR (GPIO 27), MQ-2 (GPIO 34 ADC), điều khiển Relay (GPIO 26), Buzzer (GPIO 25), Khóa cửa (GPIO 33) và giao tiếp qua MQTT broker theo contract.
- **Docker Compose**: môi trường tích hợp cục bộ cho toàn bộ dịch vụ với multi-stage Rust Dockerfiles.

## Luồng dữ liệu chính

1. Trình duyệt đăng nhập REST (`POST /api/auth/login`), nhận JWT access token và dùng token cho API/WebSocket.
2. AI Service gửi detection qua `POST /api/detections`; backend kiểm tra camera, lưu event, đánh giá chính sách cảnh báo đã cấu hình, chống trùng (deduplication), rồi phát sự kiện realtime tới dashboard.
3. ESP32 gửi telemetry vào `security/device/{device_id}/sensor` và trạng thái vào `security/device/{device_id}/status`; backend kiểm tra payload và lưu bản ghi gần nhất/lịch sử.
4. Người dùng gửi lệnh qua `POST /api/devices/{device_id}/command`; backend publish command có `command_id` qua MQTT và cập nhật log khi nhận ACK ở topic `security/device/{device_id}/ack`.
5. Dashboard lấy dữ liệu ban đầu qua REST và nhận cập nhật thời gian thực qua `/ws/dashboard`.

## Cấu trúc dự án Rust (Cargo Workspace)

```text
Cargo.toml                                  # Cargo Workspace (backend, ai-service)
backend/src/{main,config,db,models,schemas,security,services,routers,mqtt,websocket}.rs
ai-service/src/{lib,main,config,camera,detector,event_handler}.rs
ai-service/src/bin/test_yolo_sample.rs
iot-service/esp32/src/main.rs               # Firmware Rust cho ESP32 (xtensa-esp32-espidf)
database/{init.sql,seed.sql}
frontend/{index.html,css/style.css,js/app.js,js/api.js}
```

## Chạy nhanh bằng Docker Compose

1. Cài Docker Desktop, sao chép `.env.example` thành `.env`, rồi thay `POSTGRES_PASSWORD`, `SECRET_KEY`, `AI_SERVICE_TOKEN` và `BOOTSTRAP_ADMIN_PASSWORD` bằng giá trị riêng.
2. Tại thư mục dự án chạy `docker compose up --build`.
3. Mở dashboard tại <http://localhost:8080>; kiểm tra trạng thái Backend tại <http://localhost:8000/api/system/health>.
4. Đăng nhập bằng `BOOTSTRAP_ADMIN_USERNAME` và `BOOTSTRAP_ADMIN_PASSWORD` đã đặt trong `.env`.

## Chạy trực tiếp bằng Cargo (Rust Toolchain)

Yêu cầu cài đặt Rust (`rustup`, Rust 1.80+):

```powershell
# 1. Chạy Backend API (mặc định dùng SQLite nếu không đặt DATABASE_URL, lắng nghe cổng 8000)
cd backend
cargo run

# 2. Chạy AI Detection Service (lắng nghe cổng 8001)
cd ../ai-service
cargo run --bin security-monitoring-ai-service

# 3. Chạy thử nghiệm CLI đọc frame và nhận diện YOLO trên ảnh mẫu
cargo run --bin test_yolo_sample
```
