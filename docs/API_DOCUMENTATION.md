# API & Integration Contract (đề xuất ban đầu)

Base path: `/api`. JSON UTF-8. Thời gian ISO-8601 có offset; backend chuẩn hóa UTC. Đây là contract để phối hợp, chưa phải bằng chứng dịch vụ AI/ESP32 đã kết nối.

## Xác thực

- `POST /auth/login` — JSON `{ "username": "...", "password": "..." }`; trả `{ "access_token": "...", "token_type": "bearer", "expires_in": 3600 }`.
- `POST /auth/register` — tạo Operator/Admin chỉ khi `REGISTRATION_ENABLED=true`; không cho client tự nâng role. Nếu tắt, trả `403`.
- `GET /auth/me` — bearer token, trả `id, username, full_name, role`.
- UI/backend API dùng `Authorization: Bearer <token>`. Secret/credential không nằm trong source control.

## REST endpoints

| Method | Path | Ý nghĩa |
|---|---|---|
| GET/POST | `/cameras` | Danh sách / tạo camera |
| GET/PUT/DELETE | `/cameras/{id}` | Chi tiết / sửa / xóa camera |
| GET | `/devices` | Danh sách thiết bị và trạng thái gần nhất |
| GET | `/devices/{device_id}` | `device_id` API là `device_code` |
| POST | `/devices/{device_id}/command` | Gửi command; chỉ hoàn tất sau ACK |
| GET | `/sensors` | Lịch sử sensor; phân trang và lọc thiết bị/type/thời gian |
| POST | `/sensors/data` | HTTP fallback tùy chọn; MQTT là đường chính ESP32 |
| POST/GET | `/detections` | AI gửi detection / truy vấn lịch sử |
| GET | `/detections/{id}` | Chi tiết detection |
| GET | `/alerts` | Lọc theo thời gian, severity, status, type; phân trang |
| GET | `/alerts/{id}` | Chi tiết alert |
| PUT | `/alerts/{id}/resolve` | Đánh dấu xử lý, lưu người xử lý và thời điểm |
| GET | `/statistics/overview` | Thẻ tổng quan dashboard |
| GET | `/statistics/alerts` | Chuỗi số alert theo ngày |
| GET | `/statistics/detections` | Thống kê loại object theo khoảng thời gian |
| GET | `/system/health` | Health của API/DB/MQTT/AI/VPN; dependency không xác định trả `unknown` |

Danh sách dùng envelope `{ "items": [], "total": 0, "limit": 50, "offset": 0 }`. Lỗi có dạng `{ "detail": { "code": "...", "message": "..." } }`; không trả stack trace/secret.

## AI -> Backend

`POST /api/detections` (service token qua `Authorization: Bearer ...` hoặc secret riêng cấu hình ngoài repo):

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

Backend xác nhận `device_id` khớp topic và đã đăng ký; `pir` boolean, `mq2` số không âm trong giới hạn cấu hình, actuator boolean, timestamp hợp lệ. Payload lỗi bị từ chối/log an toàn, không cập nhật trạng thái.

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

`accepted` chỉ xác nhận firmware đã thực thi/xác nhận trạng thái theo khả năng phần cứng; `rejected` trả lỗi. API command chờ ACK có timeout cấu hình: ACK accepted -> thành công; rejected -> lỗi; timeout -> trạng thái `timeout`, tuyệt đối không báo thành công. Cần xác nhận schema ACK với Thành viên 1.

## WebSocket dashboard

`/ws/dashboard?token=<access_token>` (dùng query token trong handshake do browser hạn chế header; production phải TLS và token ngắn hạn). Server gửi JSON event:

```json
{ "type": "alert.created", "data": { "id": 12, "severity": "CRITICAL", "status": "OPEN" } }
```

Loại event: `detection.created`, `alert.created`, `alert.resolved`, `device.status`, `device.command.updated`. WebSocket chỉ thông báo; client tải lại bản ghi qua REST khi cần dữ liệu đầy đủ.

## Mã HTTP cần thống nhất

`200/201` thành công; `400` payload/transition không hợp lệ; `401` thiếu/sai token; `403` thiếu quyền; `404` không tồn tại; `409` xung đột/trùng; `422` schema sai; `503` dependency không sẵn sàng; timeout ACK trả trạng thái ứng dụng xác định (đề xuất `202` với `status=timeout` hoặc `504`, cần chốt khi triển khai).
