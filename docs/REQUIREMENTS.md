# Đề cương & Đặc tả Yêu cầu Hệ thống (Tuần 1)

**Đề tài:** Xây dựng hệ thống giám sát và báo động an ninh thông minh (*Security Monitoring System — SmartSecure*)

---

## 1. Mục tiêu và Phạm vi Đề tài

### 1.1. Mục tiêu
Xây dựng nền tảng giám sát an ninh tập trung kết hợp:
- **Thị giác máy tính (AI/YOLOv8)** để phát hiện sớm người, phương tiện và hành vi xâm nhập từ camera.
- **Internet vạn vật (IoT/ESP32 + MQTT)** để thu thập dữ liệu cảm biến môi trường/chuyển động (PIR, MQ-2) và điều khiển thiết bị chấp hành (Relay đèn, Còi báo động, Khóa điện) có xác nhận hai chiều (`ACK`).
- **Web Dashboard & Backend API (FastAPI + PostgreSQL + WebSocket)** theo mô hình phân lớp MVC để quản lý tập trung, cảnh báo theo thời gian thực và lưu vết kiểm toán.

### 1.2. Phạm vi thực hiện
- **Trong phạm vi**:
  - Backend REST API + WebSocket (`FastAPI`, `SQLAlchemy`, `PostgreSQL`/`SQLite`).
  - Giao diện quản trị & giám sát (`HTML5/CSS3/JS` kết nối REST API & WebSocket).
  - Dịch vụ nhận diện ảnh/frame bằng `YOLOv8` (`Ultralytics`) gửi sự kiện sang Backend.
  - Firmware `ESP32` giao tiếp qua `Mosquitto MQTT Broker` (telemetry, status, command, ack).
  - Đóng gói môi trường tích hợp bằng `Docker Compose`.
- **Ngoài phạm vi (giới hạn đồ án)**:
  - Không tự động kích hoạt còi/khóa vật lý chỉ dựa trên một khung hình AI đơn lẻ khi chưa có xác nhận hoặc quy tắc rõ ràng.
  - Luồng video trực tiếp RTSP/WebRTC qua MediaMTX ở mức cấu hình mở rộng; giao diện dùng khung hình minh họa và ảnh sự kiện.

---

## 2. Khảo sát theo Vai trò 3 Thành viên (Tuần 1)

### 2.1. Thành viên 1 — Khảo sát Backend & Cơ sở dữ liệu
- **Framework**: Chọn **FastAPI** (Python 3.12) vì hỗ trợ bất đồng bộ (`async/await`) cho WebSocket, tự động sinh tài liệu OpenAPI (`/docs`) và kiểm tra dữ liệu chặt chẽ với **Pydantic v2**.
- **CSDL**: Chọn **PostgreSQL 16** kết hợp **SQLAlchemy 2.0 ORM** để đảm bảo tính toàn vẹn tham chiếu (FK), hỗ trợ kiểu `TIMESTAMPTZ` (UTC) và `JSONB` cho dữ liệu cảm biến/log.
- **Danh sách nhóm API dự kiến**:
  1. `/api/auth/*`: Đăng nhập JWT, đăng ký (khi bật), lấy thông tin phiên (`/me`).
  2. `/api/cameras/*`: Quản lý danh sách camera (ẩn thông tin nhạy cảm trong RTSP URL).
  3. `/api/devices/*` & `/api/sensors/*`: Quản lý thiết bị ESP32, lịch sử cảm biến và gửi lệnh điều khiển.
  4. `/api/detections/*`: Tiếp nhận sự kiện nhận diện từ AI service và tra cứu lịch sử.
  5. `/api/alerts/*`: Quản lý cảnh báo an ninh và đánh dấu xử lý (`resolve`).
  6. `/api/statistics/*` & `/api/logs/*`: Thống kê tổng quan, biểu đồ theo ngày/loại đối tượng và nhật ký hệ thống.
  7. `/ws/dashboard`: Kênh đẩy thông báo thời gian thực cho trình duyệt.

### 2.2. Thành viên 2 — Khảo sát Người dùng, Dashboard & Luồng thao tác
- **Đối tượng sử dụng (Actors)**:
  - **Quản trị viên (`ADMIN`)**: Toàn quyền cấu hình hệ thống, thêm/sửa/xóa camera, quản lý người dùng, xem toàn bộ thống kê, xử lý cảnh báo và gửi lệnh điều khiển thiết bị.
  - **Nhân viên vận hành (`OPERATOR`)**: Theo dõi màn hình tổng quan, giám sát camera/AI/IoT, xác nhận và xử lý cảnh báo (`resolve`), điều khiển thiết bị khi có sự cố.
- **Luồng thao tác chính trên Dashboard**:
  1. Người dùng đăng nhập -> Nhận JWT token -> Tải trang **Tổng quan (Overview)** và mở kết nối **WebSocket**.
  2. Khi có cảnh báo mới (`alert.created`) hoặc thay đổi trạng thái thiết bị (`device.status`), giao diện tự động cập nhật thẻ thống kê và bảng cảnh báo.
  3. Nhân viên chuyển sang các tab chuyên biệt (**Giám sát Camera**, **Giám sát AI**, **Thiết bị IoT**, **Cảnh báo**, **Lịch sử**) để kiểm tra chi tiết và xử lý.

### 2.3. Thành viên 3 — Khảo sát Camera, YOLOv8, ESP32 & Cảm biến
- **Mô hình AI**: Chọn **YOLOv8 (`yolov8n.pt`)** của Ultralytics vì tốc độ suy luận nhanh trên cả CPU/GPU, phù hợp phát hiện các lớp đối tượng an ninh cơ bản (`person`, `car`, `truck`, `motorcycle`...) và dễ mở rộng thêm lớp `fire`/`smoke`/`weapon`.
- **Vi điều khiển & Cảm biến**:
  - **ESP32-WROOM-32**: Tích hợp sẵn Wi-Fi 2.4GHz, nhiều chân GPIO và ADC 12-bit, thư viện `PubSubClient` và `ArduinoJson` ổn định.
  - **PIR HC-SR501**: Phát hiện chuyển động hồng ngoại thụ động trong phạm vi 3–7m.
  - **Cảm biến khí MQ-2**: Phát hiện khói và khí dễ cháy (LPG, propane, hydrogen) qua tín hiệu Analog.
  - **Cơ cấu chấp hành**: Module Relay 5V (điều khiển đèn và khóa điện từ) và Còi Buzzer 5V.

---

## 3. Yêu cầu Chức năng và Phi chức năng

### 3.1. Yêu cầu Chức năng (Functional Requirements)
| Mã | Nhóm chức năng | Mô tả yêu cầu |
|---|---|---|
| **FR-01** | Xác thực & Phân quyền | Đăng nhập bằng `username`/`password`, cấp JWT có thời hạn; phân biệt quyền `ADMIN` và `OPERATOR`. |
| **FR-02** | Quản lý Camera | Thêm, sửa, xóa, xem danh sách camera; tự động ẩn mật khẩu trong chuỗi `rtsp_url` khi trả về API. |
| **FR-03** | Tiếp nhận Sự kiện AI | Nhận sự kiện từ AI Service (`camera_id`, `object_type`, `confidence`, `event_type`, `image_path`), kiểm tra hợp lệ và lưu CSDL. |
| **FR-04** | Sinh Cảnh báo & Chống trùng | Tự động tạo `Alert` khi `confidence >= ngưỡng` cho các sự kiện nguy hiểm; chống tạo trùng cảnh báo cùng loại trên cùng camera trong khoảng thời gian cấu hình (`alert_deduplication_seconds`). |
| **FR-05** | Giám sát Cảm biến IoT | Nhận dữ liệu `pir`, `mq2`, `relay`, `buzzer`, `lock` từ ESP32 qua MQTT (hoặc HTTP fallback), cập nhật trạng thái `ONLINE` và lưu lịch sử `sensors`. |
| **FR-06** | Điều khiển Thiết bị có ACK | Phát lệnh điều khiển xuống ESP32 kèm `command_id`; lưu trạng thái `PENDING` trong `device_logs` và chỉ chuyển sang `ACCEPTED`/`REJECTED` khi nhận được bản tin `ack`. |
| **FR-07** | Dashboard & Thống kê | Hiển thị 6 chỉ số tổng quan, biểu đồ cảnh báo/nhận diện, danh sách cảnh báo mới nhất, trạng thái thiết bị và nhật ký hoạt động thời gian thực. |

### 3.2. Yêu cầu Phi chức năng (Non-Functional Requirements)
- **Bảo mật (Security)**: Mật khẩu băm bằng `bcrypt`; API bảo vệ bằng JWT Bearer token; AI Service xác thực bằng `AI_SERVICE_TOKEN`; kiểm tra chống Path Traversal cho `image_path`.
- **Hiệu năng & Thời gian thực (Real-time)**: Độ trễ đẩy sự kiện qua WebSocket dưới 1 giây sau khi Backend ghi nhận cảnh báo hoặc thay đổi trạng thái thiết bị.
- **Tính sẵn sàng & Đóng gói (Portability)**: Toàn bộ hệ thống (PostgreSQL, Mosquitto, Backend, Frontend Nginx, AI Service) khởi chạy đồng nhất qua `docker compose up --build`.
