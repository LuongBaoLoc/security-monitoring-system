# Đặc tả Giao tiếp API, MQTT & WebSocket — Hệ thống PCCC (Tuần 2)

- **Base URL REST API:** `/api` (JSON UTF-8, thời gian ISO-8601 chuẩn UTC).
- **WebSocket:** `/ws/dashboard?token=<jwt_access_token>`.
- **Static Snapshots:** `/uploads/events/<filename>` (phục vụ ảnh bằng chứng cháy/khói qua Axum `ServeDir` & Nginx proxy).

---

## 1. Xác thực & Quản lý Người dùng (JWT + RBAC)

| Method | Endpoint | Quyền | Mô tả |
|---|---|---|---|
| `POST` | `/api/auth/login` | Công khai | Đăng nhập `{ "username", "password" }` → trả `{ "access_token", "token_type": "bearer", "expires_in", "user": { id, username, full_name, role, is_active } }`. |
| `POST` | `/api/auth/register` | Khi bật `REGISTRATION_ENABLED=true` | Tạo tài khoản `OPERATOR` mới (không cho tự nâng quyền `ADMIN`). |
| `GET` | `/api/auth/me` | `ADMIN` / `OPERATOR` | Trả thông tin người dùng của phiên hiện tại. |
| `GET` / `POST` | `/api/users` | **`ADMIN`** | Lấy danh sách tài khoản / Admin thêm tài khoản mới (`ADMIN` hoặc `OPERATOR`). |
| `PUT` / `DELETE` | `/api/users/:id` | **`ADMIN`** | Đổi vai trò, đổi mật khẩu, Khóa/Mở khóa hoặc xóa tài khoản (chặn tự khóa/tự xóa chính mình). |

---

## 2. Quản lý Camera PCCC & Endpoint Nội bộ Bảo mật RTSP

| Method | Endpoint | Quyền | Mô tả |
|---|---|---|---|
| `GET` | `/api/cameras?search=&location=&status=&limit=&offset=` | `ADMIN` / `OPERATOR` | Lọc camera theo tên/vị trí/IP, khu vực PCCC và trạng thái (`ONLINE`/`OFFLINE`). Luôn che mật khẩu RTSP, trả kèm `camera_id`, `rtsp_has_credentials` và danh sách `locations`. |
| `POST` | `/api/cameras` | **`ADMIN`** | Thêm camera giám sát mới; bắt buộc `rtsp_url` chuẩn `rtsp://` hoặc `rtsps://`, mặc định `OFFLINE` đến khi có luồng thật. |
| `GET` | `/api/cameras/:id` | `ADMIN` / `OPERATOR` | Chi tiết camera (RTSP đã che mật khẩu). |
| `PUT` | `/api/cameras/:id` | **`ADMIN`** | Cập nhật camera; nếu `rtsp_url` để trống (`""` / `null`) hoặc gửi đúng bản đã che → **giữ nguyên `rtsp_url` gốc trong CSDL**. |
| `DELETE` | `/api/cameras/:id` | **`ADMIN`** | Xóa camera và các sự kiện/cảnh báo liên quan. |
| `GET` | `/api/internal/cameras/mappings` | **`AI_SERVICE_TOKEN`** | Endpoint nội bộ (bị Nginx Frontend chặn `403`) trả mapping `camera_id -> rtsp_url` gốc cho AI Service đọc luồng RTSP/MediaMTX. |

---

## 3. Sự kiện Nhận diện Lửa & Khói (YOLOv8 → Backend)

| Method | Endpoint | Quyền | Mô tả |
|---|---|---|---|
| `POST` | `/api/detections/snapshot` | `AI_SERVICE_TOKEN` / User | Tải lên ảnh snapshot (`multipart/form-data`, trường `file` + `camera_id`). Kiểm tra magic bytes JPEG/PNG/WebP, chống path traversal, lưu tại `uploads/events/`, trả `{ "image_path", "url", "size_bytes", "content_type" }`. |
| `POST` | `/api/detections` | `AI_SERVICE_TOKEN` / User | Ghi nhận sự kiện phát hiện Lửa/Khói từ YOLOv8, tự động đánh giá sinh cảnh báo PCCC (kèm chống trùng lặp) và broadcast WebSocket. |
| `GET` | `/api/detections?camera_id=&object_type=&event_type=&min_confidence=&limit=&offset=` | `ADMIN` / `OPERATOR` | Tra cứu lịch sử phát hiện Lửa (`fire`) / Khói (`smoke`) có lọc phía server, JOIN sẵn `camera_name`. |
| `GET` | `/api/detections/:id` | `ADMIN` / `OPERATOR` | Chi tiết sự kiện AI (JOIN `camera_name`, `camera_location`, `camera_status`, `image_url` và danh sách `alerts` liên quan). |

### Mẫu Payload `POST /api/detections` (Chuẩn PCCC)
```json
{
  "camera_id": 1,
  "object_type": "fire",
  "confidence": 0.94,
  "event_type": "fire",
  "image_path": "uploads/events/cam1_20261122T143000Z_a1b2c3d4e5f6.jpg",
  "timestamp": "2026-11-22T14:30:00Z"
}
```
- **Quy ước phân loại sự kiện PCCC:**
  - `object_type = "fire"` → `event_type = "fire"` → Cảnh báo **`CRITICAL`** (Báo cháy khẩn cấp).
  - `object_type = "smoke"` → `event_type = "smoke"` → Cảnh báo sớm **`WARNING`** (tự nâng mức **`CRITICAL`** khi độ tin cậy cao `>= 0.85` hoặc trùng khu vực có nồng độ khói/gas **MQ-2** vượt ngưỡng).
  - Phản hồi `201 Created`: trả về `id`, `camera_id`, `camera_name`, `object_type`, `confidence`, `event_type`, `image_path`, `image_url`, `timestamp`, `alert_id`, `alert_severity`, `deduplicated`.

---

## 4. Cảnh báo Cháy, Thống kê, Nhật ký & Trạng thái Hạ tầng

| Method | Endpoint | Quyền | Mô tả |
|---|---|---|---|
| `GET` | `/api/alerts?status=&severity=&limit=&offset=` | `ADMIN` / `OPERATOR` | Danh sách cảnh báo cháy/khói/MQ-2/thiết bị. |
| `GET` | `/api/alerts/:id` | `ADMIN` / `OPERATOR` | Chi tiết cảnh báo. |
| `PUT` | `/api/alerts/:id/resolve` | `ADMIN` / `OPERATOR` | Trực ban xác nhận đã xử lý cảnh báo (lưu `handled_by`, `handled_at`, broadcast `alert.resolved`). |
| `GET` | `/api/statistics/overview` | `ADMIN` / `OPERATOR` | 6 chỉ số tổng quan PCCC (Camera, Thiết bị ESP32, Sự kiện Lửa/Khói hôm nay, Cảnh báo đang mở/Nghiêm trọng). |
| `GET` | `/api/statistics/alerts?days=7` | `ADMIN` / `OPERATOR` | Biểu đồ cảnh báo cháy theo ngày. |
| `GET` | `/api/statistics/detections?days=7` | `ADMIN` / `OPERATOR` | Thống kê tỷ lệ Lửa (`fire`), Khói (`smoke`) theo khoảng thời gian. |
| `GET` / `DELETE` | `/api/logs/system` | Xem: tất cả / Xóa: `ADMIN` | Nhật ký kiểm toán hệ thống. |
| `GET` | `/api/logs/devices` | `ADMIN` / `OPERATOR` | Lịch sử lệnh điều khiển ESP32 và trạng thái `ACK`. |
| `GET` | `/api/system/health` | Công khai | Tự động dò trạng thái thực tế của `api`, `database`, `mqtt_broker`, `esp32`, `camera_rtsp`, `ai_worker`, `wireguard_vpn`. |
| `GET` / `PUT` | `/api/system/settings` | Xem: tất cả / Sửa: `ADMIN` | Cấu hình ngưỡng khói/gas `mq2_threshold` (ppm), độ tin cậy YOLOv8 tối thiểu, thời gian chống trùng cảnh báo (giây). |

---

## 5. Hợp đồng IoT ESP32 PCCC qua MQTT & REST

### 5.1. REST Endpoints Thiết bị & Cảm biến
- `GET /api/devices`, `POST /api/devices` (`ADMIN`), `GET /api/devices/:device_code`, `DELETE /api/devices/:device_code` (`ADMIN`).
- `POST /api/devices/:device_code/command` (`ADMIN` / `OPERATOR`): Gửi lệnh điều khiển PCCC xuống ESP32 qua MQTT.
- `GET /api/sensors?device_id=&sensor_type=&limit=&offset=`: Tra cứu lịch sử cảm biến MQ-2 / lửa / chấp hành.
- `POST /api/sensors/data`: Kênh HTTP fallback gửi telemetry cảm biến khi không qua MQTT.

### 5.2. MQTT Topics chuẩn
- `security/device/{device_id}/sensor` — ESP32 publish dữ liệu cảm biến định kỳ mỗi 5 giây.
- `security/device/{device_id}/status` — ESP32 publish trạng thái kết nối (`online` / `offline`).
- `security/device/{device_id}/command` — Backend publish lệnh ứng cứu PCCC xuống ESP32.
- `security/device/{device_id}/ack` — ESP32 publish xác nhận thực thi lệnh về Backend.

### 5.3. Payload Telemetry (ESP32 → Backend)
```json
{
  "device_id": "ESP32_KHO_01",
  "pir": false,
  "mq2": 320,
  "relay": false,
  "buzzer": false,
  "lock": true,
  "timestamp": "2026-11-22T14:30:00Z"
}
```
- `mq2`: Nồng độ khói / khí dễ cháy đo từ cảm biến **MQ-2** (ppm, `0..100000`). Nếu vượt `mq2_threshold` (mặc định `600 ppm`), Backend tự động sinh cảnh báo `WARNING`/`CRITICAL`.
- `pir`: Tín hiệu ngõ vào số từ cảm biến lửa hồng ngoại (**Flame Sensor**, GPIO 27; giữ tên khóa `pir` để tương thích đồng bộ schema CSDL).
- `relay`: Trạng thái **Mạch Relay kích hoạt** (bơm chữa cháy / van phun sương / đèn báo cháy, GPIO 26).
- `buzzer`: Trạng thái **Còi báo động 12V** (GPIO 25).
- `lock`: Trạng thái **Khóa từ Solenoid cửa thoát hiểm** (GPIO 33 — `true`: đang khóa bình thường, `false`: đã nhả khóa mở cửa thoát hiểm khẩn cấp).

### 5.4. Payload Lệnh điều khiển & Phản hồi ACK
- **Command (Backend → ESP32):**
  ```json
  {
    "command_id": "550e8400-e29b-41d4-a716-446655440000",
    "action": "unlock",
    "requested_at": "2026-11-22T14:30:05Z",
    "requested_by": "admin"
  }
  ```
  Các giá trị `action` hợp lệ:
  - `relay_on` / `relay_off`: Bật / Tắt **Mạch Relay kích hoạt** (bơm chữa cháy / van phun sương).
  - `buzzer_on` / `buzzer_off`: Bật / Tắt **Còi báo động 12V**.
  - `unlock` / `lock`: Nhả khóa **mở cửa thoát hiểm khẩn cấp** / Khóa lại **Khóa từ Solenoid**.

- **ACK (ESP32 → Backend):**
  ```json
  {
    "command_id": "550e8400-e29b-41d4-a716-446655440000",
    "device_id": "ESP32_KHO_01",
    "action": "unlock",
    "status": "accepted",
    "message": null,
    "timestamp": "2026-11-22T14:30:06Z"
  }
  ```
  Backend lưu bản ghi ở `device_logs` với `result = PENDING`, chỉ chuyển sang `ACCEPTED` hoặc `REJECTED` khi nhận đúng `command_id` từ bản tin `ACK`.

---

## 6. Sự kiện WebSocket Thời gian thực (`/ws/dashboard`)

Định dạng khung JSON:
```json
{
  "type": "detection.created",
  "data": {
    "id": 12,
    "camera_id": 1,
    "camera_name": "CAM-01 · Nhà kho Hóa chất",
    "object_type": "fire",
    "event_type": "fire",
    "confidence": 0.94,
    "image_path": "uploads/events/cam1_20261122T143000Z_a1b2c3d4e5f6.jpg",
    "alert_id": 8,
    "alert_severity": "CRITICAL"
  }
}
```
Danh sách sự kiện: `detection.created`, `alert.created`, `alert.resolved`, `device.status`, `device.command.updated`.

---

## 7. Quy ước Mã Trạng thái HTTP

`200` / `201` / `202` / `204` thành công; `400` yêu cầu không hợp lệ; `401` thiếu/sai token; `403` không đủ quyền (hoặc truy cập `/api/internal/*` từ bên ngoài); `404` không tìm thấy; `409` trùng dữ liệu; `415` sai định dạng ảnh snapshot; `422` dữ liệu vi phạm kiểm tra hợp lệ; `503` dịch vụ phụ thuộc chưa sẵn sàng.
