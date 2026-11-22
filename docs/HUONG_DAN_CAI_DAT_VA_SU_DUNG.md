# Hướng Dẫn Cài Đặt, Phát Triển và Sử Dụng Hệ Thống PCCC SmartSecure

Tài liệu này hướng dẫn cài đặt môi trường, khởi chạy toàn bộ hạ tầng **Phòng cháy Chữa cháy thông minh (PCCC — YOLOv8 nhận diện Lửa & Khói + WireGuard VPN + Docker + ESP32)**, quy trình phát triển cho **3 thành viên** và cách vận hành trên **Web Dashboard SmartSecure**.

---

## Mục Lục

1. [Tổng quan Dịch vụ, Cổng & Phân công 3 Thành viên](#1-tổng-quan-dịch-vụ-cổng--phân-công-3-thành-viên)
2. [Công cụ cần cài đặt trước (Prerequisites)](#2-công-cụ-cần-cài-đặt-trước-prerequisites)
3. [Cài đặt & Khởi chạy bằng Docker Compose (Stack cốt lõi & Profiles mở rộng)](#3-cài-đặt--khởi-chạy-bằng-docker-compose-stack-cốt-lõi--profiles-mở-rộng)
4. [Hướng dẫn phát triển riêng cho từng thành viên](#4-hướng-dẫn-phát-triển-riêng-cho-từng-thành-viên)
5. [Hướng dẫn sử dụng Web Dashboard PCCC](#5-hướng-dẫn-sử-dụng-web-dashboard-pccc)
6. [Quy trình phối hợp Git cho nhóm 3 người](#6-quy-trình-phối-hợp-git-cho-nhóm-3-người)
7. [Xử lý lỗi thường gặp (Troubleshooting)](#7-xử-lý-lỗi-thường-gặp-troubleshooting)

---

## 1. Tổng quan Dịch vụ, Cổng & Phân công 3 Thành viên

### 1.1. Danh sách Container & Cổng (Ports)

| Dịch vụ (Container) | Công nghệ | Profile | Cổng (Port) | Vai trò PCCC |
| :--- | :--- | :---: | :---: | :--- |
| **Web Dashboard (`frontend`)** | Nginx + HTML5/CSS3/JS | Mặc định | `8080` | Giao diện trực ban PCCC, proxy `/api`, `/ws`, `/uploads` (chặn `/api/internal/`) |
| **Backend API (`backend`)** | Rust (`Axum` + `SQLx` + `Rumqttc`) | Mặc định | `8000` | REST API, JWT/RBAC, lưu snapshot, MQTT ESP32, WebSocket |
| **AI Service (`ai-service`)** | Rust (`Axum` + `tract-onnx`) | Mặc định | `8001` | Suy luận **YOLOv8 ONNX** phát hiện **Lửa (`fire`)** & **Khói (`smoke`)** |
| **PostgreSQL (`db`)** | PostgreSQL 16 | Mặc định | `5432` | Lưu 8 bảng dữ liệu bền vững & cấp dữ liệu cho Grafana |
| **MQTT Broker (`mqtt`)** | Eclipse Mosquitto 2 | Mặc định | `1883` | Giao tiếp telemetry **MQ-2** & lệnh **Còi 12V / Solenoid / Relay** với ESP32 |
| **RTSP Server (`mediamtx`)** | RTSP Simple Server (MediaMTX) | `rtsp` / `full` | `8554`, `8888` | Tiếp nhận & phân phối luồng RTSP từ Camera IP / Webcam USB |
| **WireGuard (`wireguard`)** | WireGuard Server Container | `vpn` / `full` | `51820/udp` | Đường hầm VPN mã hóa truy cập quản trị từ xa |
| **Grafana (`grafana`)** | Grafana 11 | `monitoring` / `full` | `3000` | Dashboard mở rộng theo dõi biểu đồ khói/gas MQ-2 & sự cố cháy |

### 1.2. Phân chia Thư mục theo Thành viên

| Thành viên | Phụ trách | Thư mục làm việc chính | Tài liệu tham chiếu |
| :--- | :--- | :--- | :--- |
| **Thành viên 1** | Backend Rust (9 thư mục tiếng Việt), PostgreSQL, MQTT, WireGuard VPN, MediaMTX, Docker | `backend/`, `database/`, `wireguard/`, `docker-compose.yml` | `ARCHITECTURE.md`, `API_DOCUMENTATION.md`, `DATABASE_DOCUMENTATION.md` |
| **Thành viên 2** | Web Dashboard PCCC, kết nối REST/WebSocket, phân quyền UI, xem snapshot AI | `frontend/` (`index.html`, `css/style.css`, `js/app.js`, `js/api.js`) | `WIREFRAMES_AND_UI_DESIGN.md`, `API_DOCUMENTATION.md` |
| **Thành viên 3** | AI Service Rust (YOLOv8 Lửa & Khói), Camera RTSP/Webcam, Firmware Rust ESP32 PCCC | `ai-service/`, `iot-service/esp32/`, `models/` | `ai-service/README.md`, `iot-service/README.md` |

---

## 2. Công cụ cần cài đặt trước (Prerequisites)

1. **Git**: Đồng bộ mã nguồn nhóm.
2. **Docker Desktop**: Bắt buộc để dựng nhanh toàn bộ stack container (chờ trạng thái **Engine running**).
3. **Rust Toolchain (`rustup`, Cargo 1.80+)** *(dành cho TV1 & TV3 biên dịch/chạy trực tiếp không qua Docker)*.
4. **ESP32 Rust Toolchain (`espup`, `espflash`, `ldproxy`)** *(dành cho TV3 nạp firmware Rust vào mạch ESP32 thật)*.
5. **Visual Studio Code (VS Code)** kèm extension `rust-analyzer`.

---

## 3. Cài đặt & Khởi chạy bằng Docker Compose (Stack cốt lõi & Profiles mở rộng)

### Bước 1: Tạo tệp cấu hình `.env`
```powershell
cd d:\DoAnChuyenNganh\security-monitoring-system
Copy-Item .env.example .env
```
Mở `.env` và điền các giá trị bí mật riêng (`POSTGRES_PASSWORD`, `SECRET_KEY`, `AI_SERVICE_TOKEN`, `BOOTSTRAP_ADMIN_PASSWORD`). Các thông số PCCC mặc định:
```env
MODEL_PATH=/models/yolov8n_fire_smoke.onnx
YOLO_CLASSES=fire,smoke
CONFIDENCE_THRESHOLD=0.6
NMS_IOU_THRESHOLD=0.45
FFMPEG_TIMEOUT_SECS=10
SEED_DEMO_DATA=false
```
> Đặt `SEED_DEMO_DATA=true` nếu muốn Backend tự nạp bộ dữ liệu mẫu PCCC (4 camera nhà kho/xưởng/tầng hầm, ESP32 MQ-2, sự kiện lửa/khói) trong lần khởi động đầu tiên khi CSDL trống.

### Bước 2: Khởi chạy Stack cốt lõi (nhẹ, mặc định)
```powershell
docker compose up --build -d
docker compose ps
```

### Bước 3: Bật thêm các Dịch vụ Hạ tầng Mở rộng khi cần demo (Profiles)
```powershell
# Chỉ bật RTSP Simple Server (MediaMTX)
docker compose --profile rtsp up -d

# Chỉ bật WireGuard VPN Server Container
docker compose --profile vpn up -d

# Chỉ bật Dashboard Grafana
docker compose --profile monitoring up -d

# Bật toàn bộ hạ tầng đầy đủ (Core + MediaMTX + WireGuard + Grafana)
docker compose --profile full up -d
```

### Bước 4: Truy cập các Dịch vụ
- **Web Dashboard PCCC**: <http://localhost:8080> (đăng nhập bằng `BOOTSTRAP_ADMIN_USERNAME` / `BOOTSTRAP_ADMIN_PASSWORD`).
- **Kiểm tra trạng thái thực tế Backend**: <http://localhost:8000/api/system/health>.
- **Kiểm tra AI Service**: <http://localhost:8001/health>.
- **Grafana (khi bật profile `monitoring`)**: <http://localhost:3000>.

---

## 4. Hướng dẫn phát triển riêng cho từng thành viên

### 4.1. Thành viên 1 — Backend Rust (9 Thư mục Tiếng Việt), CSDL & VPN

Cấu trúc mã nguồn trong `backend/src/`:
- `cau_hinh/mod.rs`: Đọc `.env`.
- `bao_mat/mod.rs`: JWT, Bcrypt, `current_user`, `admin_user`, `require_ai_service_token`.
- `co_so_du_lieu/{ket_noi,khoi_tao_bang}.rs`: Kết nối SQLx và tự động tạo 8 bảng SQL.
- `mo_hinh_du_lieu/mod.rs` & `khuon_mau_du_lieu/mod.rs`: Struct bảng & validate RTSP/User/Detection.
- `dinh_tuyen_api/`: 8 tệp API tách biệt (`xac_thuc.rs`, `quan_ly_nguoi_dung.rs`, `quan_ly_camera.rs`, `thiet_bi_va_cam_bien.rs`, `su_kien_ai.rs`, `canh_bao_an_ninh.rs`, `thong_ke_va_nhat_ky.rs`, `he_thong_va_cai_dat.rs`).
- `dich_vu_nghiep_vu/mod.rs`, `giao_tiep_mqtt/mod.rs`, `ket_noi_websocket/mod.rs`.

Kiểm tra biên dịch và chạy test bằng Cargo:
```powershell
cd backend
cargo check
cargo test
cargo run
```
*(Nếu Windows Application Control chặn build script cục bộ, có thể chạy `cargo check` bên trong container `rust:slim-bookworm` như cấu hình trong `backend/Dockerfile`).*

### 4.2. Thành viên 2 — Frontend Web Dashboard PCCC

Thư mục `./frontend` được gắn volume trực tiếp vào container Nginx (`:8080`):
1. Sửa `frontend/index.html`, `frontend/css/style.css`, `frontend/js/api.js`, `frontend/js/app.js`.
2. Lưu file và nhấn **`Ctrl + F5`** tại <http://localhost:8080> để xem ngay thay đổi mà không cần build lại Docker.
3. Kiểm tra cú pháp JS nhanh: `node --check frontend/js/api.js`.

### 4.3. Thành viên 3 — Edge AI (YOLOv8 Lửa/Khói) & ESP32 PCCC

#### A. Chuẩn bị Model YOLOv8 Lửa & Khói (`.onnx`) và kiểm thử luồng AI
1. Huấn luyện/tải model **YOLOv8 Nano / Small** nhận diện 2 lớp `fire`, `smoke`, xuất sang ONNX (640×640) và đặt tại `models/yolov8n_fire_smoke.onnx`:
   ```bash
   yolo export model=yolov8n_fire_smoke.pt format=onnx imgsz=640
   ```
   *(Chạy local được trên Jetson Nano, PC GPU Nvidia GTX 1650+ hoặc CPU qua `tract-onnx`).*
2. Chạy CLI `test_yolo_sample` kiểm thử trọn luồng ảnh/RTSP → inference → upload snapshot → `POST /api/detections`:
   ```powershell
   cd ai-service
   cargo run --bin test_yolo_sample -- --image duong_dan_anh_chay.jpg --camera-id 1 --submit
   # Hoặc lấy RTSP từ endpoint nội bộ của Backend (cần AI_SERVICE_TOKEN)
   cargo run --bin test_yolo_sample -- --fetch-mapping --camera-id 1 --submit
   ```
3. Giả lập gửi sự kiện phát hiện **Lửa (`fire`)** lên Backend qua PowerShell để kiểm tra báo cháy `CRITICAL` trên Dashboard:
   ```powershell
   Invoke-RestMethod -Uri "http://localhost:8000/api/detections" -Method POST `
     -Headers @{ "Authorization" = "Bearer <AI_SERVICE_TOKEN>"; "Content-Type" = "application/json" } `
     -Body '{"camera_id": 1, "object_type": "fire", "confidence": 0.95, "event_type": "fire"}'
   ```

#### B. Lắp mạch & Nạp Firmware Rust cho ESP32 (`iot-service/esp32/`)
Xem chi tiết sơ đồ đấu nối GPIO cho **MQ-2 (GPIO 34)**, **Cảm biến lửa Flame (GPIO 27)**, **Mạch Relay bơm/phun sương (GPIO 26)**, **Còi báo động 12V (GPIO 25)** và **Khóa từ Solenoid cửa thoát hiểm (GPIO 33)** tại [`iot-service/README.md`](../iot-service/README.md).

Giả lập mạch `ESP32_KHO_01` gửi cảnh báo nồng độ khói/gas **MQ-2 vượt ngưỡng (`820 ppm`)**:
```powershell
Invoke-RestMethod -Uri "http://localhost:8000/api/sensors/data" -Method POST `
  -Headers @{ "Content-Type" = "application/json" } `
  -Body '{"device_id": "ESP32_KHO_01", "pir": true, "mq2": 820, "relay": false, "buzzer": false, "lock": true}'
```

---

## 5. Hướng dẫn sử dụng Web Dashboard PCCC

### 5.1. Ý nghĩa Đèn Trạng thái Thực tế (`.dot`)
- **Chấm xanh lá có sóng lan tỏa (Pulse/Ripple Halo)**: Thành phần đang chạy và kết nối thực tế.
- **Chấm cam / đỏ nhấp nháy nhanh**: Có cảnh báo sớm Khói (`WARNING`) hoặc báo cháy Khẩn cấp Lửa (`CRITICAL`).
- **Chấm xám đứng yên**: Thành phần **Tạm thời chưa hoạt động** (chưa bật WireGuard VPN, chưa có Camera RTSP `ONLINE`, chưa đặt model ONNX hoặc chưa cắm mạch ESP32).

### 5.2. Phân quyền `ADMIN` vs `OPERATOR` (Trực ban PCCC)
- **`ADMIN`**: Toàn quyền trên 10 màn hình, bao gồm thêm/sửa/xóa Camera, thêm/đổi quyền/khóa/xóa tài khoản tại **Quản lý người dùng**, thêm/xóa mạch ESP32 và lưu **Cài đặt hệ thống**.
- **`OPERATOR`**: Xem toàn bộ dữ liệu giám sát, **xác nhận xử lý cảnh báo cháy (`Resolve`)** và **điều khiển thiết bị ứng cứu** (Bật Còi 12V, Bật Relay bơm/phun sương, Nhả Khóa từ Solenoid mở cửa thoát hiểm). Mọi nút quản trị tự động ẩn và bị Backend chặn `403`.

---

## 6. Quy trình phối hợp Git cho nhóm 3 người

1. Luôn kéo code mới nhất từ `main` trước khi làm việc; tạo nhánh riêng theo vai trò (`tv1/...`, `tv2/...`, `tv3/...`).
2. Không commit `.env`, `wireguard/config/`, `target/`, file trọng số `.onnx`/`.pt` kích thước lớn hoặc ảnh test trong `backend/uploads/events/`.
3. Khi thay đổi contract API hoặc payload MQTT, cập nhật đồng bộ vào `docs/API_DOCUMENTATION.md`.

---

## 7. Xử lý lỗi thường gặp (Troubleshooting)

- **Không đăng nhập được `admin`**: Nếu đã đổi `BOOTSTRAP_ADMIN_PASSWORD` trong `.env` sau khi DB từng khởi tạo trước đó, chạy `docker compose down -v && docker compose up --build -d` để tạo lại volume DB sạch.
- **Xung đột cổng (`8080`, `8000`, `5432`, `1883`, `8554`, `3000`)**: Kiểm tra tiến trình đang chiếm cổng trên Windows và tắt dịch vụ trùng trước khi chạy Compose.
- **ESP32 không nối được MQTT Broker**: Điền đúng IPv4 LAN của máy chủ Docker vào `MQTT_HOST` trong `iot-service/esp32/src/main.rs` và mở cổng TCP `1883` trong Windows Firewall.
- **Trình duyệt giữ bản giao diện cũ**: Nhấn **`Ctrl + F5`** (Hard Reload) tại <http://localhost:8080>.
