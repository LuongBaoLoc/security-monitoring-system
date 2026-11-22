# API và contract tích hợp

Base path `/api`. Dữ liệu JSON UTF-8. Timestamp có offset theo ISO-8601 và được chuẩn hóa về UTC khi lưu. Swagger tại `/docs` trên cổng 8000.

## Xác thực

- `POST /auth/login` — JSON `{ "username": "...", "password": "..." }`; trả `{ "access_token": "...", "token_type": "bearer", "expires_in": 3600 }`.
- `POST /auth/register` — tạo tài khoản `OPERATOR` khi `REGISTRATION_ENABLED=true`. Khi tắt, trả `403`. Admin quản lý tài khoản qua `/users`.
- `GET /auth/me` — bearer token, trả `id, username, full_name, role`.
- UI/backend API dùng `Authorization: Bearer <token>`. Secret/credential không nằm trong source control.

## REST endpoints

| Method | Path | Ý nghĩa |
|---|---|---|
| GET/POST | `/cameras` | Danh sách / tạo camera |
| GET/PUT/DELETE | `/cameras/{id}` | Chi tiết / sửa / xóa camera |
| GET/POST | `/devices` | Danh sách trạng thái / đăng ký thiết bị |
| GET/DELETE | `/devices/{device_id}` | Chi tiết / xóa theo `device_code` |
| POST | `/devices/{device_id}/command` | Gửi command; chỉ hoàn tất sau ACK |
| GET | `/sensors` | Lịch sử cảm biến, lọc theo `device_id`, `sensor_type` và phân trang |
| POST | `/sensors/data` | Dashboard nhập telemetry bằng token người dùng |
| POST/GET | `/detections` | AI gửi detection / truy vấn lịch sử |
| GET | `/detections/{id}` | Chi tiết detection |
| GET | `/alerts` | Lọc theo `severity`, `status` và phân trang |
| GET | `/alerts/{id}` | Chi tiết alert |
| PUT | `/alerts/{id}/resolve` | Đánh dấu xử lý, lưu người xử lý và thời điểm |
| GET | `/statistics/overview` | Thẻ tổng quan dashboard |
| GET | `/statistics/alerts` | Chuỗi số alert theo ngày |
| GET | `/statistics/detections` | Thống kê loại object theo khoảng thời gian |
| GET | `/system/health` | Trạng thái DB, kết nối MQTT, AI và tín hiệu WireGuard trong môi trường backend |
| GET/PUT | `/system/settings` | Đọc / lưu ngưỡng và cài đặt hệ thống |

Danh sách phân trang dùng `{ "items": [], "total": 0, "limit": 50, "offset": 0 }`. Lỗi trả `detail` dạng chuỗi hoặc danh sách lỗi kiểm tra dữ liệu.

## AI -> Backend

`POST /api/detections` nhận `Authorization: Bearer <AI_SERVICE_TOKEN>` từ AI Service. Dashboard gửi sự kiện thử bằng JWT người dùng.

```json
{
  "camera_id": 1,
  "object_type": "person",
  "confidence": 0.92,
  "event_type": "intrusion",
  "image_path": "uploads/events/event_001.jpg",
  "timestamp": "2026-11-01T10:00:00Z"
}
```

`camera_id` phải là camera đã đăng ký; `confidence` thuộc `[0,1]`; `timestamp` ISO-8601. `image_path` là đường dẫn tương đối đã được AI upload/cấp phép, không phải đường dẫn tùy ý trên filesystem backend. Thành công trả `201` với `id`, `alert_id` nullable, `alert_severity` nullable, `deduplicated` boolean. Detection được lưu; chỉ tạo alert khi policy cấu hình khớp. Hệ thống không tự bật còi do detection đơn lẻ.

## ESP32 <-> Backend qua MQTT

Topic giữ đúng quy ước nhóm:

- `security/device/{device_id}/sensor` — ESP32 publish telemetry.
- `security/device/{device_id}/status` — ESP32 publish trạng thái/heartbeat.
- `security/device/{device_id}/command` — backend publish command.
- `security/device/{device_id}/ack` — ESP32 publish xác nhận.

Payload telemetry đề xuất (tương thích mẫu yêu cầu):

```json
{
  "device_id": "ESP32_001",
  "pir": true,
  "mq2": 350,
  "relay": false,
  "buzzer": false,
  "lock": false,
  "timestamp": "2026-11-01T10:00:00Z"
}
```

Thiết bị phải được đăng ký. Backend lấy mã thiết bị từ topic và đối chiếu với `device_id` nếu có. `pir` và trạng thái đầu ra là boolean, `mq2` là số trong khoảng 0–100000. Nếu firmware bỏ qua `timestamp`, backend dùng thời điểm nhận.

Command backend -> ESP32:

```json
{
  "command_id": "uuid",
  "action": "buzzer_on",
  "requested_at": "2026-11-01T10:00:00Z",
  "requested_by": "operator"
}
```

`action`: `relay_on|relay_off|buzzer_on|buzzer_off|unlock|lock`. ACK thiết bị -> backend:

```json
{
  "command_id": "uuid",
  "device_id": "ESP32_001",
  "action": "buzzer_on",
  "status": "accepted",
  "message": null,
  "timestamp": "2026-11-01T10:00:01Z"
}
```

API trả `202` với `status=pending` và `command_id`. ACK khớp thiết bị, mã lệnh và hành động chuyển nhật ký sang `ACCEPTED` hoặc `REJECTED`. Hết hạn cấu hình `MQTT_COMMAND_TIMEOUT_SECONDS` (mặc định 15 giây), lệnh chuyển sang `TIMEOUT`. Xem kết quả tại `GET /api/logs/devices` hoặc qua sự kiện WebSocket `device.command.updated`. Broker mất kết nối trả `503`.

## WebSocket dashboard

`/ws/dashboard?token=<access_token>` (dùng query token trong handshake do browser hạn chế header; production phải TLS và token ngắn hạn). Server gửi JSON event:

```json
{ "type": "alert.created", "data": { "id": 12, "severity": "CRITICAL", "status": "OPEN" } }
```

Loại event: `detection.created`, `alert.created`, `alert.resolved`, `device.status`, `device.command.updated`. WebSocket chỉ thông báo; client tải lại bản ghi qua REST khi cần dữ liệu đầy đủ.

## Mã HTTP cần thống nhất

`200/201/204` thành công, `202` đang chờ ACK, `400` trạng thái nghiệp vụ sai, `401` thiếu hoặc sai token, `403` thiếu quyền, `404` bản ghi không tồn tại, `409` xung đột, `422` dữ liệu sai định dạng, `503` broker không sẵn sàng.
