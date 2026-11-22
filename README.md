# Security Monitoring System — Chủ đề 2

Hạ tầng giám sát và báo động an ninh tích hợp Edge AI, RTSP, MQTT và mạng VPN WireGuard. Docker Compose kết nối FastAPI, PostgreSQL, Mosquitto, dashboard và dịch vụ AI.

## Thành phần và ranh giới

- **Backend (`backend/app/` — FastAPI, SQLAlchemy, Paho MQTT)**: REST API, JWT và bcrypt, tiếp nhận sự kiện AI, ghi nhận ACK thiết bị và phát sự kiện qua `/ws/dashboard`.
- **PostgreSQL 16 / SQLite**: Compose dùng PostgreSQL. Chạy trực tiếp mặc định dùng SQLite. FastAPI tự tạo bảng và nâng cấp cột thời gian/JSON của dữ liệu backend Rust đã tạo. Cài đặt hệ thống được lưu trong cơ sở dữ liệu.
- **Frontend (`frontend/` — HTML/CSS/JS)**: dashboard trình duyệt gọi REST/WebSocket. Không kết nối trực tiếp cơ sở dữ liệu hoặc MQTT broker.
- **AI Service (`ai-service/` — Rust Axum)** nhận khung hình từ ảnh tải lên hoặc từ một camera RTSP đã cấu hình. YOLOv8 chạy qua ONNX Runtime, nhận diện người/phương tiện và lửa/khói, rồi gửi sự kiện đến backend. [Cài trọng số và thử AI Service](ai-service/README.md)
- **ESP32 Firmware (`iot-service/esp32/` — Rust `esp-idf-svc` + `esp-idf-hal`)**: đọc cảm biến PIR (GPIO 27), MQ-2 (GPIO 34 ADC), điều khiển Relay (GPIO 26), Buzzer (GPIO 25), Khóa cửa (GPIO 33) và giao tiếp qua MQTT broker theo contract.
- **WireGuard**: khi triển khai qua VPN, định tuyến mạng camera/ESP32 tại môi trường triển khai. Dashboard dùng REST/WebSocket qua Nginx.

## Luồng dữ liệu chính

1. Trình duyệt đăng nhập REST (`POST /api/auth/login`), nhận JWT access token và dùng token cho API/WebSocket.
2. AI Service gửi detection qua `POST /api/detections`; backend kiểm tra camera, lưu event, đánh giá chính sách cảnh báo đã cấu hình, chống trùng (deduplication), rồi phát sự kiện realtime tới dashboard.
3. ESP32 gửi telemetry vào `security/device/{device_id}/sensor` và trạng thái vào `security/device/{device_id}/status`; backend kiểm tra payload và lưu bản ghi gần nhất/lịch sử.
4. Người dùng gửi lệnh qua `POST /api/devices/{device_id}/command`. Backend trả `202 pending`, lưu `command_id` rồi chờ ACK ở topic `security/device/{device_id}/ack`. Mặc định quá 15 giây thì nhật ký chuyển sang `TIMEOUT`.
5. Dashboard lấy dữ liệu ban đầu qua REST và nhận cập nhật thời gian thực qua `/ws/dashboard`.

## Cấu trúc dự án

```text
backend/app/                                # FastAPI mặc định của Docker Compose
backend/requirements.txt                    # Phụ thuộc Python
backend/src/                                # Backend Rust của nhánh trước, còn trong Cargo workspace
ai-service/src/{lib,main,config,camera,detector,event_handler}.rs
ai-service/src/bin/test_yolo_sample.rs
iot-service/esp32/src/main.rs               # Firmware Rust cho ESP32 (xtensa-esp32-espidf)
database/{init.sql,seed.sql}
frontend/{index.html,css/style.css,js/app.js,js/api.js}
```

## Chạy nhanh bằng Docker Compose

1. Cài Docker Desktop, sao chép `.env.example` thành `.env`, rồi thay `POSTGRES_PASSWORD`, `SECRET_KEY`, `AI_SERVICE_TOKEN` và `BOOTSTRAP_ADMIN_PASSWORD` bằng giá trị riêng.
2. Tại thư mục dự án chạy `docker compose up --build`.
3. Mở dashboard tại <http://localhost:8080>, Swagger tại <http://localhost:8000/docs> và trạng thái tích hợp tại <http://localhost:8000/api/system/health>.
4. Đăng nhập bằng `BOOTSTRAP_ADMIN_USERNAME` và `BOOTSTRAP_ADMIN_PASSWORD` đã đặt trong `.env`.

Khi đổi từ backend Rust, dừng container backend cũ và giữ volume `postgres_data`. Sao lưu cơ sở dữ liệu trước khi khởi động bản FastAPI lần đầu. FastAPI nâng cấp schema của volume hiện tại trong một giao dịch; lần chuyển này thay đổi kiểu cột mà backend Rust dùng.

## Chạy trực tiếp

```powershell
py -3.11 -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r backend/requirements.txt
.\.venv\Scripts\python.exe -m uvicorn app.main:app --app-dir backend --port 8000
```

Lệnh trên đọc `.env` ở thư mục gốc. Đặt `DATABASE_URL=postgresql+psycopg://...` để dùng PostgreSQL ngoài Compose. Xem hướng dẫn chạy AI bằng Cargo và contract REST/WebSocket/MQTT. [AI Service](ai-service/README.md) · [API tích hợp](docs/API_DOCUMENTATION.md)
