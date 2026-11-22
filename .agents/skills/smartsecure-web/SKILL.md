---
name: smartsecure-web
description: >-
  Hoàn thiện, đồng bộ từ Tuần 1 đến Tuần 6+ cho Đồ án "Hạ tầng Giám sát & Báo động An ninh
  tích hợp Edge AI (YOLOv8) và Mạng VPN WireGuard qua Docker" định hướng chuyên sâu
  Phòng cháy Chữa cháy (PCCC — YOLOv8 nhận diện Lửa & Khói theo nghiệp vụ TCVN 5738:2021 /
  quy trình phòng trực trung tâm PCCC). Bao gồm Backend Rust (Axum + SQLx, 9 thư mục tiếng Việt),
  AI Service Rust (YOLOv8 fire/smoke), hạ tầng Docker (WireGuard VPN, VLAN, RTSP Simple Server /
  MediaMTX, Grafana), IoT ESP32 (MQ-2, Còi 12V, Khóa từ Solenoid mở cửa thoát hiểm, Mạch Relay),
  cùng Web Dashboard phân quyền (ADMIN/OPERATOR) và tự động kiểm tra trạng thái thực tế kèm animation.
---

# Skill Đồ Án PCCC — Hạ tầng Giám sát & Báo động tích hợp Edge AI (YOLOv8) & WireGuard VPN

Sử dụng skill này khi người dùng gọi `/smartsecure-web` hoặc yêu cầu rà soát, đồng bộ tài liệu/code từ **Tuần 1 đến Tuần 6+** theo đúng **nghiệp vụ Phòng cháy Chữa cháy (PCCC) thực tế** và bám sát **bảng đặc tả đề tài chính thức**.

---

## 0. Nghiệp vụ Cốt lõi của Hệ thống PCCC (Chuẩn TCVN 5738:2021 / TCVN 7568-14:2025 & Phòng trực Trung tâm)

Mọi thiết kế tài liệu, CSDL, API, AI và Giao diện đều phải phản ánh đúng 5 nghiệp vụ PCCC sau:

1. **Quản lý theo Vùng bảo vệ (Fire Zones / Khu vực)**:
   - Mỗi khu vực giám sát (VD: *Zone 1 - Kho hàng*, *Zone 2 - Hầm gửi xe B1*, *Zone 3 - Nhà xưởng A*, *Zone 4 - Phòng kỹ thuật điện*) gắn liền **Camera IP/RTSP (AI YOLOv8)** và **Thiết bị IoT ESP32 (MQ-2 + Relay/Còi/Khóa)** để định vị chính xác điểm phát sinh cháy.
2. **Phát hiện Đa tầng & Xác nhận chéo 2 tín hiệu chống báo cháy giả (Cross-Zone / Sensor Fusion)**:
   - Theo nguyên tắc liên động PCCC chuẩn TCVN, hạn chế tối đa kích hoạt chữa cháy giả từ 1 tín hiệu đơn lẻ (như bụi xưởng, hơi nước, tia lửa hàn, khói thuốc):
     - **Cấp 1 — Tiền cảnh báo sớm (`WARNING` / Pre-Alarm)**: Chỉ có 1 nguồn đơn lẻ phát hiện sớm (`smoke` độ tin cậy trung bình hoặc `MQ-2` tăng nhẹ quanh ngưỡng). Hiển thị ngay ảnh snapshot lên Dashboard để nhân viên trực kiểm tra qua Camera.
     - **Cấp 2 — Báo cháy xác thực (`CRITICAL` / Fire Alarm)**: AI YOLOv8 phát hiện **Lửa (`fire`)** với độ tin cậy cao / ổn định đa khung hình, HOẶC **xác nhận chéo** AI phát hiện `smoke`/`fire` kết hợp **MQ-2 vượt ngưỡng nguy hiểm** tại cùng khu vực.
   - Áp dụng **chống trùng cảnh báo (`deduplication` 60s)** để không spam cùng 1 loại sự cố trên cùng 1 camera.
3. **Quy trình 5 bước của Phòng trực Trung tâm PCCC (FACP / SOC Workflow)**:
   - **Bước 1 — Tiếp nhận (`OPEN`)**: Đẩy cảnh báo realtime qua WebSocket kèm ảnh snapshot có bounding box (đỏ = Lửa, vàng cam = Khói), tên Zone, Camera, độ tin cậy, nồng độ MQ-2.
   - **Bước 2 — Xác minh qua Camera & Phân loại**: Nhân viên trực (`OPERATOR`) kiểm tra ảnh/luồng camera hiện trường, có thể bấm tắt chuông tạm (`Silence`) và phân loại kết quả xử lý:
     - **Cháy thật (`CONFIRMED_FIRE`)** -> Kích hoạt ngay kịch bản liên động ứng cứu cháy.
     - **Báo cháy giả (`FALSE_ALARM`)** -> Ghi chú nguyên nhân (*bụi, hơi nước, tia lửa hàn, khói thuốc...*) và **Reset** hệ thống về trạng thái thường trực (`Normal`).
   - **Bước 3 — Điều khiển Liên động PCCC có xác nhận `ACK` 2 chiều**:
     - **Còi báo động 12V (`buzzer_on` / `buzzer_off`)**: Hú còi sơ tán khẩn cấp / tắt còi (`Silence`).
     - **Khóa từ Solenoid (`unlock` / `lock`)**: Mở khóa **cửa thoát hiểm** khẩn cấp (nguyên lý Fail-Safe khi có cháy).
     - **Mạch Relay kích hoạt (`relay_on` / `relay_off`)**: Bật bơm chữa cháy / van phun sương / quạt hút khói hoặc ngắt điện khu vực cháy.
     - *Bắt buộc*: Mọi lệnh gửi qua MQTT phải có `command_id` và chỉ đánh dấu thành công sau khi mạch ESP32 trả về `ACK`.
   - **Bước 4 — Khôi phục (`RESOLVED`)**: Đóng cảnh báo, lưu người xử lý (`handled_by`), thời gian (`handled_at`) và đưa thiết bị về trạng thái giám sát bình thường.
   - **Bước 5 — Sổ nhật ký vận hành PCCC (Audit Log)**: Tự động lưu vết đầy đủ sự kiện phát hiện, ảnh bằng chứng, người trực xử lý, phân loại cháy thật/báo giả, lệnh liên động và kết quả `ACK`.
4. **Giám sát Lỗi thiết bị & Đường truyền (`FAULT` Monitoring)**:
   - Trong PCCC, Camera mất tín hiệu hoặc ESP32 mất kết nối là **lỗi giám sát (`FAULT`)** cần hiển thị trung thực ngay lập tức.
   - Cơ chế **Tự động kiểm tra trạng thái thực tế (`/api/system/health`) + Animation `.dot`**: đang hoạt động = chấm xanh sóng lan tỏa; đang báo cháy = pulse đỏ/cam nhịp nhanh; chưa bật/mất kết nối = chấm xám tĩnh `"Tạm thời chưa hoạt động"`.
5. **Phân quyền & Hạ tầng Mạng bảo mật (RBAC + VLAN + WireGuard VPN)**:
   - **Phân đoạn VLAN** (VLAN Camera/IoT, VLAN Edge AI/Backend, VLAN Quản trị qua đường hầm mã hóa **WireGuard VPN Container**).
   - **Container Docker**: WireGuard Server, RTSP Simple Server (MediaMTX), Stream Processor YOLOv8 (`fire`/`smoke`), Backend Rust Axum, PostgreSQL, Mosquitto MQTT, Web Dashboard, cấu hình mở rộng Grafana.
   - **`ADMIN` (Quản trị / Đội trưởng kỹ thuật PCCC)**: Toàn quyền quản trị, thêm/sửa/khóa tài khoản ở trang `users`, quản lý Camera RTSP (che mật khẩu), thiết bị IoT, cài đặt ngưỡng PCCC.
   - **`OPERATOR` (Nhân viên trực trung tâm PCCC)**: Giám sát 24/7, xác minh sự cố cháy/báo giả, xử lý cảnh báo (`Resolve`), điều khiển liên động (Còi 12V, Solenoid thoát hiểm, Relay); tự động ẩn nút quản trị và chặn `403` ở API quản trị.

---

## 1. Cấu trúc Thư mục Backend Rust bằng Tiếng Việt — Giữ nguyên không phá vỡ (`backend/src/`)

```text
backend/src/
├── main.rs                         # Điểm khởi chạy chính, mount `/api`, `/ws/dashboard`, `/uploads`
├── cau_hinh/mod.rs                 # 1. Cấu hình hệ thống & biến môi trường (.env)
├── bao_mat/mod.rs                  # 2. Bảo mật, JWT Token, bcrypt & Phân quyền ADMIN/OPERATOR
├── co_so_du_lieu/                  # 3. Kết nối PostgreSQL/SQLite & khởi tạo bảng
│   ├── mod.rs
│   ├── ket_noi.rs
│   └── khoi_tao_bang.rs
├── mo_hinh_du_lieu/mod.rs          # 4. Mô hình bảng dữ liệu trong Database
├── khuon_mau_du_lieu/mod.rs        # 5. Khuôn mẫu JSON Request/Response
├── dinh_tuyen_api/                 # 6. Định tuyến API tách riêng từng chức năng
│   ├── mod.rs
│   ├── xac_thuc.rs                 # /api/auth/*
│   ├── quan_ly_nguoi_dung.rs       # /api/users/* (Admin thêm/sửa/khóa tài khoản)
│   ├── quan_ly_camera.rs           # /api/cameras/* & /api/internal/cameras/mappings (che mật khẩu RTSP)
│   ├── thiet_bi_va_cam_bien.rs     # /api/devices/* & /api/sensors/* (MQ-2, Còi 12V, Solenoid, Relay)
│   ├── su_kien_ai.rs               # /api/detections/* & upload snapshot sự kiện lửa/khói
│   ├── canh_bao_an_ninh.rs         # /api/alerts/* (Cảnh báo cháy/khói, xác nhận cháy thật/báo giả & Resolve)
│   ├── thong_ke_va_nhat_ky.rs      # /api/statistics/* & /api/logs/* (Sổ nhật ký vận hành PCCC)
│   └── he_thong_va_cai_dat.rs      # /api/system/health (tự động dò Fault/Online) & /api/settings
├── dich_vu_nghiep_vu/mod.rs        # 7. Bootstrap Admin, seed dữ liệu mẫu PCCC, redact RTSP, ghi log
├── giao_tiep_mqtt/mod.rs           # 8. Giao tiếp MQTT với ESP32 (telemetry, status, command, ack)
└── ket_noi_websocket/mod.rs        # 9. Kênh WebSocket `/ws/dashboard` đẩy cảnh báo cháy thời gian thực
```

---

## 2. Yêu cầu Đồng bộ Tuần 1 → Tuần 6

- **Tuần 1–3**: Đồng bộ toàn bộ `README.md`, `docs/*`, `docker-compose.yml`, `.env.example`, `wireguard/README.md` theo đúng đề tài PCCC, tiêu chuẩn nghiệp vụ báo cháy và bảng công nghệ chính thức.
- **Tuần 4**: Đồng bộ `database/init.sql`, `database/seed.sql`, `dich_vu_nghiep_vu/mod.rs`, `iot-service/esp32/src/main.rs` và giao diện Tổng quan sang dữ liệu/thuật ngữ PCCC.
- **Tuần 5**: Xác thực JWT, phân quyền `ADMIN`/`OPERATOR`, Admin thêm tài khoản, CRUD Camera validate & che mật khẩu RTSP, endpoint nội bộ `/api/internal/cameras/mappings`, `ai-service/src/camera.rs` ẩn mật khẩu RTSP và thêm timeout `ffmpeg`.
- **Tuần 6**: `ai-service/src/detector.rs` chạy YOLOv8 ONNX chuyên 2 lớp `fire` & `smoke` (`YOLO_CLASSES=fire,smoke`, letterbox 640x640, confidence, NMS), vẽ khung đỏ (Lửa)/vàng cam (Khói), upload snapshot vào `uploads/events/`, Backend mount `/uploads`, lọc server-side `GET /api/detections`, Frontend hiển thị ảnh snapshot thật + bảng & modal chi tiết sự kiện AI.
