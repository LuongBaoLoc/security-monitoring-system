# Security Monitoring System

Nền tảng giám sát an ninh tích hợp AI, IoT, backend API và dashboard. Phạm vi tài liệu hiện tại là các bước nền: phân tích kiến trúc, thống nhất database/API contract, và tạo cấu trúc backend ban đầu. Không tuyên bố AI hoặc thiết bị phần cứng đã tích hợp/chạy thành công.

## Thành phần và ranh giới

- **Backend (FastAPI)**: REST API, xác thực/phân quyền, persistence, tiếp nhận sự kiện AI, điều phối lệnh thiết bị và WebSocket dashboard.
- **PostgreSQL**: dữ liệu bền vững (người dùng, camera, thiết bị, sự kiện, cảnh báo, nhật ký). Migration là nguồn thay đổi schema sau khởi tạo; `database/init.sql` dành cho môi trường mới.
- **Frontend**: dashboard trình duyệt gọi REST/WebSocket. Không kết nối trực tiếp PostgreSQL hoặc MQTT broker.
- **AI service**: gửi sự kiện phát hiện đến REST API; backend không phụ thuộc nội bộ YOLO.
- **ESP32**: publish/subscribe qua MQTT broker theo contract; backend là nơi xác thực quyền lệnh và phát lệnh.
- **Docker Compose**: môi trường tích hợp cục bộ cho dịch vụ. Chưa xác minh các dịch vụ ngoại vi thực sự kết nối.

## Luồng dữ liệu chính

1. Trình duyệt đăng nhập REST, nhận access token và dùng token cho API/WebSocket.
2. AI gửi detection qua `POST /api/detections`; backend kiểm tra camera, lưu event, đánh giá chính sách cảnh báo đã cấu hình, chống trùng, rồi phát event dashboard.
3. ESP32 gửi telemetry vào `security/device/{device_id}/sensor` và trạng thái vào `security/device/{device_id}/status`; backend kiểm tra payload và lưu bản ghi gần nhất/lịch sử.
4. Operator có quyền gửi lệnh; backend publish command có `command_id`, chờ ACK tương ứng ở topic `.../ack`. Chỉ sau ACK hợp lệ mới báo lệnh hoàn tất.
5. Dashboard lấy dữ liệu ban đầu qua REST và nhận cập nhật thời gian thực qua `/ws/dashboard`.

## Cấu trúc bước nền

```text
backend/app/{core,database,models,schemas,routers,services,mqtt,websocket}/
database/{init.sql,seed.sql}
docs/{ARCHITECTURE.md,API_DOCUMENTATION.md,DATABASE_DOCUMENTATION.md}
```

## Chạy

Các file chạy hiện có trong repo ban đầu đều rỗng; hướng dẫn triển khai đầy đủ sẽ được bổ sung cùng các giai đoạn triển khai backend/dashboard. Hợp đồng tích hợp hiện tại được ghi ở `docs/API_DOCUMENTATION.md` và `docs/DATABASE_DOCUMENTATION.md`.
