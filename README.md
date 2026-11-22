# Hạ tầng Giám sát & Báo động An ninh tích hợp Edge AI (YOLOv8) và Mạng VPN WireGuard qua Docker

> **Định hướng chuyên sâu:** Hệ thống **Phòng cháy Chữa cháy thông minh (PCCC — SmartSecure Fire & Smoke)** ứng dụng cho **Tòa nhà, Khu công nghiệp, Nhà kho và Bãi xe thông minh**, sử dụng **YOLOv8 nhận diện Lửa (`fire`) & Khói (`smoke`)** thời gian thực kết hợp mạng **VPN WireGuard**, phân đoạn **VLAN**, **RTSP Simple Server (MediaMTX)** và thiết bị ứng cứu **IoT ESP32**.

---

## 1. Kiến trúc Thành phần & Công nghệ (Rust + Docker)

- **Backend API (`backend/` — Rust Axum + SQLx + Tokio + Rumqttc)**:
  - Tổ chức mã nguồn rõ ràng theo **9 thư mục tiếng Việt** (`cau_hinh`, `bao_mat`, `co_so_du_lieu`, `mo_hinh_du_lieu`, `khuon_mau_du_lieu`, `dinh_tuyen_api`, `dich_vu_nghiep_vu`, `giao_tiep_mqtt`, `ket_noi_websocket`).
  - Xác thực JWT + Bcrypt, phân quyền `ADMIN` / `OPERATOR`, bảo mật che mật khẩu RTSP, endpoint nội bộ cấp mapping RTSP cho AI Service, lưu ảnh snapshot cháy/khói (`uploads/events/`), đánh giá quy tắc báo cháy & chống trùng lặp (deduplication), điều phối lệnh ESP32 có `ACK` qua MQTT và đẩy cảnh báo thời gian thực qua WebSocket (`/ws/dashboard`).
- **Stream Processor / Edge AI (`ai-service/` — Rust Axum + `tract-onnx`)**:
  - Chạy mô hình **YOLOv8 Nano / Small** (định dạng `.onnx`, hỗ trợ chạy local trên **Jetson Nano**, **PC GPU Nvidia GTX 1650+** hoặc **CPU**) chuyên nhận diện **Lửa (`fire`)** và **Khói (`smoke`)** thời gian thực.
  - Đọc khung hình từ **Camera IP RTSP / Webcam USB** qua `ffmpeg` (có timeout & tự động che mật khẩu RTSP trong log), tải ảnh snapshot và gửi sự kiện về `POST /api/detections`. Đi kèm công cụ CLI `test_yolo_sample`.
- **Thiết bị IoT Ứng cứu PCCC (`iot-service/esp32/` — Rust `esp-idf-svc` + `esp-idf-hal`)**:
  - Vi điều khiển **ESP32** đọc **Cảm biến khói/gas MQ-2** (`GPIO 34 ADC`), cảm biến lửa/tín hiệu số (`GPIO 27`) và điều khiển **Mạch Relay kích hoạt bơm chữa cháy / van phun sương** (`GPIO 26`), **Còi báo động 12V** (`GPIO 25`), **Khóa từ Solenoid mở cửa thoát hiểm khẩn cấp** (`GPIO 33`) qua **MQTT Broker** có phản hồi `ACK`.
- **Hạ tầng Mạng, Container & Giám sát Mở rộng (`docker-compose.yml`, `wireguard/`)**:
  - **Phân đoạn VLAN** (VLAN Camera/IoT hiện trường, VLAN Edge AI/Backend, VLAN Quản trị).
  - **WireGuard Server Container** (profile `vpn`): tạo đường hầm VPN mã hóa kết nối quản trị từ xa an toàn.
  - **RTSP Simple Server / MediaMTX** (profile `rtsp`): tiếp nhận và phân phối luồng video RTSP từ Camera IP / Webcam USB.
  - **PostgreSQL 16** + **Eclipse Mosquitto MQTT Broker** + **Web Dashboard (Nginx + HTML5/CSS3/JS)**.
  - **Dashboard Grafana** (profile `monitoring`): mở rộng trực quan hóa chuỗi thời gian nồng độ khói/gas MQ-2 và tần suất báo cháy từ PostgreSQL.

---

## 2. Luồng Dữ liệu PCCC Chính

1. **Xác thực & Quản trị**: Quản trị viên (`ADMIN`) hoặc trực ban PCCC (`OPERATOR`) đăng nhập qua `POST /api/auth/login` (truy cập qua kênh mạng nội bộ hoặc đường hầm **WireGuard VPN**), nhận JWT token cho REST API và `/ws/dashboard`.
2. **Phát hiện Lửa & Khói bằng Edge AI**: Camera IP / Webcam USB đẩy luồng video vào **RTSP Simple Server (MediaMTX)**; `ai-service` lấy danh sách RTSP từ endpoint nội bộ `/api/internal/cameras/mappings`, chụp frame, chạy **YOLOv8 (`fire`, `smoke`)**, tải snapshot lên `/api/detections/snapshot` và gửi sự kiện về `POST /api/detections`.
3. **Đánh giá Cảnh báo Cháy**: Backend lưu sự kiện, phân mức **Khẩn cấp (`CRITICAL`)** cho **Lửa (`fire`)** và **Cảnh báo sớm (`WARNING`)** cho **Khói (`smoke`)** (nâng `CRITICAL` khi độ tin cậy cao hoặc kết hợp MQ-2 vượt ngưỡng), chống trùng lặp theo cooldown, và phát sự kiện realtime `detection.created` / `alert.created` lên Dashboard.
4. **Cảm biến & Ứng cứu Phần cứng ESP32**: ESP32 gửi định kỳ chỉ số khói/gas MQ-2 vào `security/device/{device_id}/sensor` và trạng thái vào `security/device/{device_id}/status`. Khi trực ban bấm kích hoạt **Bơm/Phun sương (`relay_on`)**, **Còi báo động 12V (`buzzer_on`)** hoặc **Mở khóa Solenoid thoát hiểm (`unlock`)**, Backend gửi lệnh có `command_id` xuống MQTT và chỉ xác nhận hoàn tất khi nhận bản tin `ACK` ở `security/device/{device_id}/ack`.

---

## 3. Cấu trúc Mã nguồn Dự án

```text
security-monitoring-system/
├── Cargo.toml                          # Cargo Workspace (backend, ai-service)
├── docker-compose.yml                  # Stack cốt lõi + profiles (vpn, rtsp, monitoring)
├── .env.example                        # Biến môi trường mẫu cho PCCC, VPN, RTSP, Grafana
├── backend/src/                        # Backend Rust Axum — 9 thư mục tiếng Việt
│   ├── main.rs                         # Điểm khởi chạy server & ServeDir /uploads
│   ├── cau_hinh/                       # 1. Đọc biến môi trường (.env)
│   ├── bao_mat/                        # 2. JWT, Bcrypt, phân quyền & AI_SERVICE_TOKEN
│   ├── co_so_du_lieu/                  # 3. Pool kết nối & tự động tạo bảng SQL
│   ├── mo_hinh_du_lieu/                # 4. Struct mô hình dữ liệu
│   ├── khuon_mau_du_lieu/              # 5. Schema JSON & kiểm tra hợp lệ RTSP/User
│   ├── dinh_tuyen_api/                 # 6. Tách riêng 8 nhánh API chức năng
│   ├── dich_vu_nghiep_vu/              # 7. Bootstrap Admin, seed PCCC, che RTSP, log
│   ├── giao_tiep_mqtt/                 # 8. Lắng nghe telemetry/ACK & gửi lệnh ESP32
│   └── ket_noi_websocket/              # 9. Kênh /ws/dashboard realtime
├── ai-service/                         # Stream Processor YOLOv8 Lửa & Khói (Rust)
│   └── src/{lib,main,config,camera,detector,event_handler}.rs + bin/test_yolo_sample.rs
├── iot-service/esp32/                  # Firmware Rust ESP32 (MQ-2, Còi 12V, Solenoid, Relay)
├── frontend/                           # Web Dashboard PCCC (Nginx + HTML/CSS/JS)
├── database/                           # init.sql (Schema) & seed.sql (Dữ liệu mẫu PCCC)
├── wireguard/                          # Tài liệu & cấu hình WireGuard VPN + phân đoạn VLAN
└── docs/                               # Hồ sơ phân tích, kiến trúc, API, CSDL, hướng dẫn
```

---

## 4. Khởi chạy Nhanh bằng Docker Compose

1. Sao chép `.env.example` thành `.env` và thay các khóa bí mật (`POSTGRES_PASSWORD`, `SECRET_KEY`, `AI_SERVICE_TOKEN`, `BOOTSTRAP_ADMIN_PASSWORD`).
2. Chạy stack cốt lõi (PostgreSQL, Mosquitto MQTT, Backend Rust, AI Service Rust, Web Dashboard Nginx):
   ```powershell
   docker compose up --build -d
   ```
3. Bật thêm các dịch vụ hạ tầng mở rộng khi cần demo toàn diện (WireGuard VPN, RTSP Simple Server MediaMTX, Dashboard Grafana):
   ```powershell
   docker compose --profile rtsp --profile vpn --profile monitoring up -d
   ```
4. Truy cập Web Dashboard tại **<http://localhost:8080>** và kiểm tra tình trạng dịch vụ thực tế tại **<http://localhost:8000/api/system/health>**.

Xem hướng dẫn chi tiết từng thành viên tại [`docs/HUONG_DAN_CAI_DAT_VA_SU_DUNG.md`](docs/HUONG_DAN_CAI_DAT_VA_SU_DUNG.md).
