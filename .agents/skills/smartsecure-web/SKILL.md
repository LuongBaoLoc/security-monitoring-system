---
name: smartsecure-web
description: >-
  Hoàn thiện, tái cấu trúc thư mục Backend Rust (Axum + SQLx) với tên thư mục và file
  bằng tiếng Việt rõ ràng (phần nào ra phần đó, dễ tìm, dễ nhìn), vận hành và kiểm thử
  toàn bộ chức năng trên Web Dashboard SmartSecure, bao gồm tự động kiểm tra trạng thái
  thực tế kèm animation, phân quyền người dùng (ADMIN / OPERATOR), và Admin thêm/quản lý tài khoản.
---

# SmartSecure Rust Backend (Thư mục Tiếng Việt), Full Web Functions & RBAC Skill

Sử dụng skill này khi người dùng gọi `/smartsecure-web` hoặc yêu cầu **sắp xếp cấu trúc thư mục Backend Rust (`backend/src`) bằng tên tiếng Việt rõ ràng, phần nào ra phần đó**, **tự động cập nhật trạng thái hệ thống kèm animation**, **Phân quyền (`ADMIN` / `OPERATOR`) & Admin thêm tài khoản**, hoặc **hoàn thiện toàn bộ chức năng Web Dashboard**.

---

## 0. Cấu trúc Thư mục Backend Rust bằng Tiếng Việt — Phần nào ra phần đó (`backend/src/`)

Khi tổ chức mã nguồn Rust (`Axum` + `SQLx` + `rumqttc`), sử dụng **tên thư mục và tên file bằng tiếng Việt không dấu (`snake_case` đúng chuẩn trình biên dịch Rust)** để nhìn vào biết ngay file nào phụ trách chức năng gì, tuyệt đối không để file nằm lộn xộn:

```text
backend/
├── Cargo.toml
└── src/
    ├── main.rs                         # Điểm khởi chạy chính của server Backend
    ├── cau_hinh/                       # 1. Cấu hình hệ thống & đọc biến môi trường (.env)
    │   └── mod.rs                      # Struct Settings (port, database_url, mqtt, secret_key...)
    ├── bao_mat/                        # 2. Bảo mật, Mã hóa mật khẩu, JWT & Kiểm tra phân quyền
    │   └── mod.rs                      # hash_password, verify_password, create_token, current_user, admin_user
    ├── co_so_du_lieu/                  # 3. Cơ sở dữ liệu (PostgreSQL / SQLite)
    │   ├── mod.rs
    │   ├── ket_noi.rs                  # Quản lý Pool kết nối DB & AppState dùng chung
    │   └── khoi_tao_bang.rs            # Tự động tạo các bảng SQL (init_schema)
    ├── mo_hinh_du_lieu/                # 4. Mô hình bảng dữ liệu trong Database (SQLx Models)
    │   └── mod.rs                      # User, Camera, Device, Sensor, DetectionEvent, Alert, Logs, RuntimeSettings
    ├── khuon_mau_du_lieu/              # 5. Khuôn mẫu dữ liệu JSON gửi lên / trả về (Request/Response Schemas)
    │   └── mod.rs                      # Login, UserCreate, CameraCreate, CommandCreate, DetectionCreate...
    ├── dinh_tuyen_api/                 # 6. Định tuyến API — Tách riêng từng trang/chức năng trên Web
    │   ├── mod.rs                      # Gộp tất cả các nhánh API vào `/api` (`api_router()`)
    │   ├── xac_thuc.rs                 # Đăng nhập, Đăng ký, Lấy thông tin phiên (`/api/auth/*`)
    │   ├── quan_ly_nguoi_dung.rs       # Danh sách user, Admin thêm/sửa/khóa tài khoản (`/api/users/*`)
    │   ├── quan_ly_camera.rs           # Xem, Thêm, Sửa, Xóa Camera (`/api/cameras/*`)
    │   ├── thiet_bi_va_cam_bien.rs     # Thiết bị IoT ESP32, gửi lệnh điều khiển & cảm biến (`/api/devices/*`, `/api/sensors/*`)
    │   ├── su_kien_ai.rs               # Tiếp nhận & tra cứu sự kiện nhận diện AI YOLOv8 (`/api/detections/*`)
    │   ├── canh_bao_an_ninh.rs         # Danh sách cảnh báo & xử lý cảnh báo (`/api/alerts/*`)
    │   ├── thong_ke_va_nhat_ky.rs      # Số liệu trang Tổng quan, biểu đồ & lịch sử nhật ký (`/api/statistics/*`, `/api/logs/*`)
    │   └── he_thong_va_cai_dat.rs      # Tự động kiểm tra trạng thái hệ thống & cài đặt (`/api/system/health`, `/api/settings`)
    ├── dich_vu_nghiep_vu/              # 7. Dịch vụ xử lý nghiệp vụ nền
    │   └── mod.rs                      # Tạo Admin mặc định (bootstrap), tạo dữ liệu mẫu (seed), ghi log hệ thống
    ├── giao_tiep_mqtt/                 # 8. Giao tiếp MQTT với mạch ESP32
    │   └── mod.rs                      # Lắng nghe telemetry/status/ack & phát lệnh điều khiển qua MQTT Broker
    └── ket_noi_websocket/              # 9. Kết nối WebSocket thời gian thực
        └── mod.rs                      # Kênh `/ws/dashboard` đẩy thông báo trực tiếp lên giao diện Web
```

### Quy tắc bắt buộc khi sắp xếp thư mục Tiếng Việt:
1. Dùng tiếng Việt không dấu chuẩn `snake_case` để trình biên dịch `cargo` hoạt động ổn định 100% trên mọi hệ điều hành và Docker mà không gặp lỗi bảng mã Unicode.
2. Trong thư mục `dinh_tuyen_api/`, **mỗi chức năng trên Web phải nằm ở 1 file riêng biệt** (`xac_thuc.rs`, `quan_ly_nguoi_dung.rs`, `quan_ly_camera.rs`, `thiet_bi_va_cam_bien.rs`, `su_kien_ai.rs`, `canh_bao_an_ninh.rs`, `thong_ke_va_nhat_ky.rs`, `he_thong_va_cai_dat.rs`).
3. Xóa toàn bộ các file `.rs` tiếng Anh nằm phẳng cũ ở ngoài `backend/src/` (`config.rs`, `db.rs`, `models.rs`, `mqtt.rs`, `routers.rs`, `schemas.rs`, `security.rs`, `services.rs`, `websocket.rs`) sau khi đã chuyển code vào các thư mục tiếng Việt tương ứng; ở ngoài `backend/src/` chỉ giữ duy nhất file `main.rs` và 9 thư mục tiếng Việt trên.
4. Chạy `cargo check` để đảm bảo toàn bộ `mod` và `use crate::...` biên dịch thành công 100%.

---

## 1. Cơ chế Tự động Kiểm tra Trạng thái & Hiệu ứng Animation Chấm Trạng thái (`.dot`)

- **Đang hoạt động (`.dot` xanh lá — `online` / `active`)**: Có hiệu ứng **nhấp nháy sóng lan tỏa (Pulse / Ripple Halo Animation)** mượt mà liên tục bằng CSS `@keyframes`.
- **Cảnh báo / Nghiêm trọng (`warning` / `critical`)**: Hiệu ứng pulse màu cam/đỏ nhanh hơn (`1.2s infinite`).
- **Tạm thời chưa hoạt động / Ngoại tuyến (`offline` / `inactive`)**: Đổi sang **màu xám tĩnh (`#94a3b8`)**, tắt hoàn toàn animation (`animation: none; box-shadow: none`).
- **Tự động kiểm tra thực tế (`dinh_tuyen_api/he_thong_va_cai_dat.rs`)**: Thành phần nào đã có / đang chạy thì tự bật chấm xanh có animation và hiển thị thông số; thành phần nào thiếu / đang tắt thì tự hiện chấm xám tĩnh `"Tạm thời chưa hoạt động"`.

---

## 2. Phân quyền Người dùng (`ADMIN` vs `OPERATOR`) & Admin Thêm Tài khoản (`dinh_tuyen_api/quan_ly_nguoi_dung.rs`)

- **`ADMIN`**: Toàn quyền trên 10 trang; tại trang **Quản lý người dùng (`users`)** có nút **Thêm tài khoản** (`POST /api/users`), đổi vai trò (`ADMIN` <-> `OPERATOR`) và Khóa/Mở khóa tài khoản (`PUT /api/users/:id`).
- **`OPERATOR`**: Được xem các trang giám sát, lọc dữ liệu, xử lý cảnh báo (`PUT /api/alerts/:id/resolve`) và điều khiển thiết bị IoT, nhưng tự động ẩn các nút/mục quản trị và chặn `403 Forbidden` ở Backend.
