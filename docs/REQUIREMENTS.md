# Đề cương & Đặc tả Yêu cầu Hệ thống PCCC (Tuần 1)

**Tên đề tài chính thức:** *Hạ tầng Giám sát & Báo động An ninh tích hợp Edge AI (YOLOv8) và Mạng VPN WireGuard qua Docker*  
**Định hướng ứng dụng chuyên sâu:** **Phòng cháy Chữa cháy thông minh (PCCC — Phát hiện Lửa `fire` & Khói `smoke`)** cho **Tòa nhà, Khu công nghiệp, Nhà kho và Bãi xe thông minh**.

---

## 1. Mục tiêu và Phạm vi Đề tài

### 1.1. Mục tiêu
Xây dựng hạ tầng giám sát, phát hiện sớm hỏa hoạn và điều phối ứng cứu khẩn cấp tập trung dựa trên 4 trụ cột:
1. **Edge AI (YOLOv8 Nano/Small chạy local)** phân tích luồng video thời gian thực từ Camera IP RTSP / Webcam USB để nhận diện sớm **Lửa (`fire`)** và **Khói (`smoke`)**, xuất ảnh bằng chứng (snapshot) và truyền sự kiện về máy chủ trung tâm.
2. **Hạ tầng Mạng Bảo mật & Đóng gói Docker**: Phân đoạn **VLAN** tách biệt lớp Camera/IoT hiện trường với lớp xử lý trung tâm, kết nối quản trị từ xa qua đường hầm mã hóa **WireGuard VPN**, tiếp nhận luồng video qua **RTSP Simple Server (MediaMTX)** và mở rộng trực quan hóa dữ liệu trên **Grafana**.
3. **IoT Ứng cứu PCCC (ESP32 + MQTT)**: Đọc liên tục nồng độ khói/khí dễ cháy từ **Cảm biến MQ-2** (kèm cảm biến lửa/tín hiệu số), đồng thời điều khiển từ xa có xác nhận hai chiều (`ACK`) đối với **Còi báo động 12V**, **Khóa từ Solenoid (mở cửa thoát hiểm khẩn cấp)** và **Mạch Relay kích hoạt** (bơm chữa cháy / van phun sương / kích còi 12V).
4. **Backend Rust (Axum + SQLx) & Web Dashboard PCCC**: Quản trị tập trung camera, thiết bị IoT, sự kiện cháy/khói, cảnh báo khẩn cấp, phân quyền `ADMIN` / `OPERATOR`, tự động kiểm tra trạng thái sống/chết thực tế của từng dịch vụ và cập nhật tức thì qua WebSocket.

### 1.2. Phạm vi thực hiện
- **Trong phạm vi**:
  - **Backend API + WebSocket (`backend/`)**: Viết bằng **Rust (`Axum`, `SQLx`, `Tokio`, `Rumqttc`)**, cấu trúc **9 thư mục tiếng Việt**, hỗ trợ **PostgreSQL 16** (môi trường Docker) và **SQLite** (chạy thử cục bộ).
  - **AI Service (`ai-service/`)**: Viết bằng **Rust (`Axum`, `tract-onnx`, `image`)**, chạy mô hình **YOLOv8 Nano / Small (.onnx)** tại biên (Jetson Nano / PC GPU Nvidia GTX 1650+ hoặc CPU) chuyên 2 lớp **Lửa (`fire`)** và **Khói (`smoke`)**, đọc luồng RTSP/Webcam qua `ffmpeg` có timeout và ẩn mật khẩu.
  - **ESP32 Firmware (`iot-service/esp32/`)**: Viết bằng **Rust (`esp-idf-svc` + `esp-idf-hal`)** kết nối **Eclipse Mosquitto MQTT Broker** thu thập **MQ-2** và điều khiển **Relay**, **Còi 12V**, **Khóa từ Solenoid** có phản hồi `ACK`.
  - **Web Dashboard (`frontend/`)**: HTML5/CSS3/JS thuần phục vụ qua Nginx, hỗ trợ chế độ Sáng/Tối (SOC Dark Mode), hiển thị ảnh snapshot cháy/khói thực tế, phân quyền `ADMIN`/`OPERATOR`, tự động kiểm tra sức khỏe hạ tầng.
  - **Hạ tầng Docker (`docker-compose.yml`, `wireguard/`)**: Chạy gọn stack lõi mặc định và hỗ trợ bật mở rộng **WireGuard Server Container**, **RTSP Simple Server (MediaMTX)** và **Dashboard Grafana** qua Docker Compose `profiles`.
- **Ngoài phạm vi / Giới hạn an toàn**:
  - Không tự động đóng/mở cơ cấu chấp hành vật lý công suất lớn (bơm chữa cháy / nhả khóa Solenoid) chỉ từ một khung hình AI đơn lẻ khi chưa có xác nhận của trực ban hoặc quy tắc phối hợp đa cảm biến (AI + MQ-2) nhằm tránh báo động giả gây thiệt hại tài sản.
  - Phân đoạn VLAN thực tế phụ thuộc thiết bị mạng tầng 2/3 (Managed Switch / Router VLAN); trong phòng lab được mô phỏng và tài liệu hóa bằng phân tách Docker networks + WireGuard VPN.

---

## 2. Khảo sát theo Vai trò 3 Thành viên (Tuần 1)

### 2.1. Thành viên 1 — Backend Rust, Cơ sở dữ liệu, MQTT, WireGuard VPN & Docker
- **Công nghệ Backend**: Chọn **Rust (`Axum 0.7` + `SQLx 0.8` + `Tokio`)** để đạt độ ổn định cao, tiêu thụ RAM thấp khi triển khai trên thiết bị biên (Edge Server / Jetson / Mini PC), an toàn bộ nhớ và xử lý đồng thời nhiều luồng WebSocket/MQTT.
- **Cơ sở dữ liệu**: Chọn **PostgreSQL 16** lưu trữ 8 bảng dữ liệu bền vững (`users`, `cameras`, `devices`, `sensors`, `detection_events`, `alerts`, `device_logs`, `system_logs`) và phục vụ trực tiếp làm nguồn dữ liệu cho **Grafana**.
- **Hạ tầng Mạng & Bảo mật**: Thiết kế phân đoạn VLAN (`VLAN 10` Camera/IoT, `VLAN 20` Edge AI/Backend, `VLAN 30` Quản trị VPN), cấu hình **WireGuard Server Container**, **RTSP Simple Server (MediaMTX)** và tách endpoint nội bộ `GET /api/internal/cameras/mappings` bảo vệ bằng `AI_SERVICE_TOKEN`.
- **Các nhóm API cốt lõi**: `/api/auth/*`, `/api/users/*`, `/api/cameras/*`, `/api/internal/cameras/mappings`, `/api/devices/*`, `/api/sensors/*`, `/api/detections/*` (kèm `/snapshot`), `/api/alerts/*`, `/api/statistics/*`, `/api/logs/*`, `/api/system/*`, `/ws/dashboard`.

### 2.2. Thành viên 2 — Giao diện Web Dashboard PCCC, Phân quyền & Luồng Thao tác
- **Đối tượng sử dụng (Actors)**:
  - **Quản trị viên (`ADMIN`)**: Toàn quyền quản trị hạ tầng PCCC (thêm/sửa/xóa camera khu vực cháy, quản lý tài khoản trực ban, cấu hình ngưỡng khí gas MQ-2 & độ tin cậy YOLOv8, theo dõi trạng thái VPN/RTSP/AI/ESP32, xử lý cảnh báo và điều khiển thiết bị ứng cứu).
  - **Nhân viên trực ban PCCC (`OPERATOR`)**: Theo dõi màn hình Tổng quan, Giám sát Camera, Giám sát AI (Lửa/Khói), xử lý/xác nhận cảnh báo cháy (`Resolve`) và thao tác nhanh các thiết bị ứng cứu (bật Còi 12V, kích hoạt Relay bơm/phun sương, nhả Khóa từ Solenoid mở cửa thoát hiểm). Toàn bộ mục quản trị được ẩn tự động và chặn `403` từ Backend.
- **Luồng thao tác trực ban khi xảy ra sự cố cháy/khói**:
  1. Đăng nhập JWT → Mở kết nối WebSocket `/ws/dashboard` → Theo dõi trạng thái thực tế của 6 thành phần hạ tầng.
  2. Khi YOLOv8 phát hiện **Lửa (`fire`)** hoặc **Khói (`smoke`)** (hoặc MQ-2 vượt ngưỡng ppm), Dashboard bật cảnh báo realtime (`detection.created` / `alert.created`), hiển thị ngay ảnh snapshot và khu vực xảy ra cháy.
  3. Trực ban mở chi tiết sự kiện xác minh hình ảnh → chuyển sang **Thiết bị IoT** để kích hoạt **Còi báo động 12V**, **Mạch Relay chữa cháy** và **nhả Khóa từ Solenoid mở cửa thoát hiểm** (chờ xác nhận `ACK`) → đánh dấu đã xử lý cảnh báo và xuất báo cáo sự cố.

### 2.3. Thành viên 3 — Edge AI (YOLOv8 Lửa & Khói), Camera RTSP & IoT ESP32 PCCC
- **Mô hình AI Local**: Chọn **YOLOv8 Nano (`yolov8n`) / Small (`yolov8s`)** huấn luyện chuyên biệt 2 lớp **`fire` (Lửa)** và **`smoke` (Khói)**, xuất sang định dạng **ONNX (`.onnx`)** để chạy suy luận trực tiếp trong **AI Service Rust (`tract-onnx`)** trên **Jetson Nano**, **PC GPU Nvidia GTX 1650+** hoặc **CPU** mà không phụ thuộc môi trường Python cồng kềnh.
- **Luồng hình ảnh**: Camera IP chuẩn RTSP hoặc Webcam USB đẩy luồng vào **RTSP Simple Server (MediaMTX)**; AI Service trích xuất khung hình bằng `ffmpeg` có giới hạn timeout và che mật khẩu RTSP.
- **Phần cứng IoT ESP32 PCCC**:
  - **ESP32-WROOM-32 DevKit V1** (Firmware Rust `esp-idf-svc` + `esp-idf-hal`).
  - **Cảm biến khói/khí dễ cháy MQ-2** (ngõ Analog ADC1 GPIO 34) đo nồng độ khói/gas (ppm).
  - **Cảm biến lửa hồng ngoại (Flame Sensor)** (ngõ Digital GPIO 27) phát hiện bức xạ tia lửa tại chỗ.
  - **Còi báo động 12V** (kích qua Transistor/Relay tại GPIO 25) phát báo động âm thanh công suất lớn.
  - **Khóa từ Solenoid 12V** (kích qua Relay tại GPIO 33) ở chế độ Fail-Safe giúp **nhả khóa mở cửa thoát hiểm khẩn cấp** khi có lệnh `unlock`.
  - **Mạch Relay kích hoạt** (GPIO 26) điều khiển máy bơm chữa cháy mini / van phun sương / đèn báo cháy khẩn cấp.

---

## 3. Yêu cầu Chức năng và Phi chức năng

### 3.1. Yêu cầu Chức năng (Functional Requirements)
| Mã | Nhóm chức năng | Mô tả yêu cầu |
|---|---|---|
| **FR-01** | Xác thực & Phân quyền RBAC | Đăng nhập bằng `username`/`password`, cấp JWT có thời hạn; phân quyền rõ ràng `ADMIN` (toàn quyền, thêm/sửa/khóa/xóa tài khoản) và `OPERATOR` (giám sát, xử lý báo cháy, điều khiển thiết bị ứng cứu). |
| **FR-02** | Quản lý Camera & Bảo mật RTSP | Thêm, sửa, xóa, tìm kiếm/lọc (`search`, `location`, `status`) camera giám sát PCCC theo khu vực (nhà kho, xưởng, tầng hầm, bãi xe); kiểm tra chuẩn `rtsp://`/`rtsps://`; che mật khẩu RTSP khi trả về Frontend; bảo toàn URL gốc khi sửa mà để trống; cấp mapping gốc cho AI Service qua `/api/internal/cameras/mappings` có `AI_SERVICE_TOKEN`. |
| **FR-03** | Phát hiện Lửa & Khói (YOLOv8) | Đọc frame từ Camera IP RTSP / MediaMTX / Webcam USB (có timeout & ẩn mật khẩu log); suy luận YOLOv8 ONNX (`fire`, `smoke`) với lọc confidence + NMS; tải ảnh snapshot lên `/api/detections/snapshot` (chống path traversal) và ghi nhận sự kiện vào `POST /api/detections`. |
| **FR-04** | Cảnh báo Cháy & Chống trùng | Tự động sinh `Alert` khi phát hiện **Lửa (`fire` → `CRITICAL`)**, **Khói (`smoke` → `WARNING` hoặc nâng `CRITICAL` khi vượt ngưỡng cao)**, hoặc **MQ-2 vượt ngưỡng khói/gas (ppm)**; chống tạo trùng cảnh báo cùng loại trên cùng camera trong khoảng cooldown (`alert_deduplication_seconds`). |
| **FR-05** | Giám sát Cảm biến PCCC (ESP32) | Nhận telemetry khói/gas `mq2`, tín hiệu cảm biến lửa/số và trạng thái `relay`, `buzzer`, `lock` qua MQTT (`security/device/{device_id}/sensor` & `/status`), lưu lịch sử vào bảng `sensors` và hiển thị thời gian thực. |
| **FR-06** | Điều khiển Ứng cứu PCCC có ACK | Gửi lệnh có `command_id` xuống ESP32 qua MQTT: `relay_on`/`relay_off` (Bơm/Phun sương), `buzzer_on`/`buzzer_off` (Còi báo động 12V), `unlock`/`lock` (Khóa từ Solenoid mở cửa thoát hiểm). Chỉ chuyển trạng thái `ACCEPTED`/`REJECTED` sau khi nhận bản tin `ack`. |
| **FR-07** | Dashboard PCCC, Giám sát Hạ tầng & Grafana | Hiển thị 6 thẻ thống kê PCCC, biểu đồ cháy/khói theo ngày, ảnh snapshot thực tế + modal chi tiết sự kiện, nhật ký kiểm toán, tự động dò trạng thái sống/chết thực tế (Backend, PostgreSQL, WireGuard VPN, MQTT/ESP32, RTSP, AI Worker) và hỗ trợ mở rộng biểu đồ Grafana. |

### 3.2. Yêu cầu Phi chức năng (Non-Functional Requirements)
- **An toàn & Bảo mật Mạng**: Phân đoạn VLAN tách thiết bị biên khỏi mạng quản trị; truy cập từ xa qua **WireGuard VPN** mã hóa; mật khẩu băm `bcrypt`; JWT Bearer token; `AI_SERVICE_TOKEN` so sánh constant-time; chặn `/api/internal/*` từ Nginx công khai; kiểm tra magic bytes & chống Path Traversal khi lưu ảnh snapshot.
- **Thời gian thực (Real-time)**: Độ trễ từ lúc AI phát hiện Lửa/Khói hoặc MQ-2 vượt ngưỡng đến khi hiển thị cảnh báo trên Web Dashboard qua WebSocket `< 1 giây`.
- **Tính trung thực trạng thái**: Mọi thành phần chưa kết nối thực tế (WireGuard VPN, RTSP MediaMTX, AI Worker, ESP32) phải hiển thị `"Tạm thời chưa hoạt động"` màu xám tĩnh, tuyệt đối không báo xanh giả.
- **Đóng gói & Khả chuyển**: Toàn bộ dịch vụ lõi và mở rộng đóng gói hoàn chỉnh trong `docker-compose.yml` với multi-stage Rust Dockerfiles.
