# Phân tích kiến trúc Module 3

## Phạm vi

Module 3 là lớp điều phối và trình bày: Backend API, PostgreSQL, web dashboard, tích hợp AI/IoT, cùng observability. AI inference và firmware ESP32 thuộc thành viên khác; giao tiếp qua contract ổn định, không import code nội bộ của nhau.

## Kiến trúc logic

```text
Browser dashboard --HTTPS/REST + WebSocket--> FastAPI
AI service --------HTTPS POST /detections----> FastAPI
ESP32 <-------------MQTT broker-------------> MQTT adapter của backend
FastAPI ------------SQLAlchemy---------------> PostgreSQL
Browser <-----------HLS/WebRTC--------------- MediaMTX (tùy chọn)
```

Backend được chia theo trách nhiệm: `routers` xử lý giao thức HTTP, `schemas` xác thực đầu vào/đầu ra, `services` chứa nghiệp vụ, `models` ánh xạ dữ liệu, `database` quản lý session, `mqtt` xử lý telemetry/command/ACK và `websocket` phân phối sự kiện dashboard. Không đặt toàn bộ nghiệp vụ trong `main.py`.

## Luồng và ranh giới tin cậy

- Mật khẩu lưu dưới dạng hash; token có thời hạn và secret chỉ lấy từ biến môi trường. Mọi route quản trị/điều khiển phải kiểm tra role phía server.
- RTSP URL có thể chứa thông tin xác thực: chỉ trả metadata đã che credential; không ghi URL thô vào log hoặc dashboard.
- Telemetry và detection là dữ liệu không tin cậy: kiểm tra schema, kích thước, phạm vi số và timestamp trước khi ghi.
- Việc phát hiện người/vật không tự bật relay/còi. Quy tắc cảnh báo phải cấu hình riêng; hành động điều khiển vật lý cần người có quyền và ACK.
- Lệnh MQTT dùng `command_id` tương quan ACK, timeout thành `timeout/unknown` chứ không báo thành công giả.
- Ảnh sự kiện lưu trong vùng upload riêng, giới hạn loại/kích thước; API chỉ phát đường dẫn hoặc URL đã kiểm soát.

## Quyết định dữ liệu

- PostgreSQL là nguồn dữ liệu chuẩn; `device_code` là định danh MQTT ổn định, còn `devices.id` là khóa nội bộ.
- `detection_events` lưu mọi sự kiện AI; `alerts` là bản ghi nghiệp vụ riêng, có thể không tạo nếu policy không khớp hoặc bị gộp chống lặp.
- `sensors` giữ lịch sử telemetry theo từng loại; trạng thái mới nhất của thiết bị có thể được denormalize trong `devices` để truy vấn dashboard nhanh.
- `device_logs` ghi lệnh và ACK/error; `system_logs` ghi hành động người dùng có ý nghĩa kiểm toán.
- Thời gian lưu UTC (`TIMESTAMPTZ`); client trình bày theo múi giờ địa phương.
- Schema production tiến hóa bằng Alembic migration; `init.sql`/`seed.sql` phục vụ bootstrap/demo cục bộ.

## Các điểm phải chốt khi tích hợp

1. Broker host/port, TLS hoặc VPN, thông tin xác thực và quyền ACL cho từng topic.
2. Payload ESP32: kiểu timestamp, ngưỡng MQ-2/đơn vị, cấu trúc trạng thái cảm biến và ACK/error.
3. Cách AI xác thực (service token), quy ước camera ID, event type, timezone và nơi lưu ảnh.
4. MediaMTX stream path và giao thức browser (WebRTC/HLS); RTSP không phát trực tiếp trong thẻ video.
5. Policy cảnh báo, thời lượng gộp sự kiện, thời gian chờ ACK và quyền Operator/Admin.

Contract mặc định đề xuất được ghi trong `API_DOCUMENTATION.md`; cần Thành viên 1/2 xác nhận các điểm trên trước khi coi là tương thích thiết bị thật.
