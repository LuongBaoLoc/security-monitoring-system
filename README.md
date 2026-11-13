# Security Monitoring System

Nền tảng giám sát an ninh tích hợp backend API, dashboard, AI inference endpoint, PostgreSQL và MQTT/ESP32 mẫu. Các module hiện có tạo được luồng nền để thử nghiệm; model YOLO, camera thật, firmware/hardware, TLS và phân quyền MQTT phải được cấu hình riêng trước khi sử dụng thực tế.

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
4. Operator có quyền gửi lệnh; backend publish command có `command_id` và trả `pending`. MQTT adapter cập nhật log khi nhận ACK ở topic `.../ack`; API chưa có endpoint truy vấn trạng thái lệnh.
5. Dashboard lấy dữ liệu ban đầu qua REST và nhận cập nhật thời gian thực qua `/ws/dashboard`.

## Cấu trúc bước nền

```text
backend/app/{core,database,models,schemas,routers,services,mqtt,websocket}/
database/{init.sql,seed.sql}
docs/{ARCHITECTURE.md,API_DOCUMENTATION.md,DATABASE_DOCUMENTATION.md}
```

## Chạy nhanh bằng Docker Compose

1. Cài Docker Desktop, sao chép `.env.example` thành `.env`, rồi thay `POSTGRES_PASSWORD`, `SECRET_KEY`, `AI_SERVICE_TOKEN` và `BOOTSTRAP_ADMIN_PASSWORD` bằng giá trị riêng.
2. Tại thư mục dự án chạy `docker compose up --build`.
3. Mở dashboard tại <http://localhost:8080>; API docs tại <http://localhost:8000/docs>.
4. Đăng nhập bằng `BOOTSTRAP_ADMIN_USERNAME` và `BOOTSTRAP_ADMIN_PASSWORD` đã đặt trong `.env`.

Frontend tĩnh được Nginx phục vụ và chuyển tiếp `/api` cùng `/ws` đến FastAPI. PostgreSQL được khởi tạo tự động, schema hiện tạo từ SQLAlchemy khi backend khởi động. `database/init.sql` là tài liệu/bootstrap SQL tham chiếu. Mosquitto trong compose cho môi trường demo; cấu hình hiện cho phép kết nối ẩn danh và chỉ nên bind trong mạng phát triển tin cậy. Không đưa broker cấu hình này ra Internet.

## Chạy backend trực tiếp

Python 3.12 trở lên: tạo virtualenv trong `backend`, cài `pip install -r requirements.txt`, đặt `DATABASE_URL` (mặc định SQLite), `SECRET_KEY`, `BOOTSTRAP_ADMIN_USERNAME` và `BOOTSTRAP_ADMIN_PASSWORD`, sau đó chạy `uvicorn app.main:app --reload` từ thư mục `backend`. UI có thể phục vụ bằng bất kỳ static web server nào.

## Tích hợp

- AI gửi JSON theo `docs/API_DOCUMENTATION.md` đến `POST /api/detections` với `Authorization: Bearer $AI_SERVICE_TOKEN`.
- AI endpoint nhận frame tại `http://localhost:8001/detect?camera_id=1` (multipart field `file`); đặt file model YOLO vào `models/` và giữ `MODEL_PATH=/models/yolov8n.pt` hoặc sửa đường dẫn tương ứng.
- Thiết bị có thể gửi telemetry dự phòng qua `POST /api/sensors/data`; firmware mẫu sử dụng MQTT.
- YOLO endpoint nhận từng frame nhưng chưa có worker đọc camera liên tục; luồng RTSP/WebRTC thật chưa được cấu hình. Xác thực MQTT theo thiết bị, phân quyền chi tiết và migration Alembic chưa được cấu hình. Dashboard giữ ảnh xem trước minh họa cho đến khi MediaMTX/luồng camera được nối.
