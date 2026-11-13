# Thiết kế Cơ sở Dữ liệu — PostgreSQL (Tuần 2 & Tuần 4)

## 1. Sơ đồ Quan hệ Thực thể (ERD)

```mermaid
erDiagram
    USERS ||--o{ ALERTS : "handled_by"
    USERS ||--o{ SYSTEM_LOGS : "user_id"
    CAMERAS ||--o{ DETECTION_EVENTS : "camera_id"
    DETECTION_EVENTS ||--o{ ALERTS : "detection_id"
    DEVICES ||--o{ SENSORS : "device_id"
    DEVICES ||--o{ DEVICE_LOGS : "device_id"

    USERS {
        bigint id PK
        varchar username UK
        varchar password_hash
        varchar full_name
        varchar role "ADMIN | OPERATOR"
        boolean is_active
        timestamptz created_at
    }

    CAMERAS {
        bigint id PK
        varchar name
        inet ip_address
        text rtsp_url
        varchar location
        varchar status "ONLINE | OFFLINE | UNKNOWN"
        timestamptz last_seen
        timestamptz created_at
        timestamptz updated_at
    }

    DEVICES {
        bigint id PK
        varchar device_code UK "Khớp topic MQTT"
        varchar device_name
        varchar device_type "ESP32"
        varchar status "ONLINE | OFFLINE | UNKNOWN"
        timestamptz last_seen
        boolean latest_pir
        float latest_mq2
        boolean relay
        boolean buzzer
        boolean lock_engaged
        timestamptz created_at
    }

    SENSORS {
        bigint id PK
        bigint device_id FK
        varchar sensor_type "pir | mq2 | relay | buzzer | lock"
        jsonb sensor_value
        timestamptz created_at
    }

    DETECTION_EVENTS {
        bigint id PK
        bigint camera_id FK
        varchar object_type
        real confidence "0.0 -> 1.0"
        varchar event_type
        text image_path
        timestamptz event_timestamp
        timestamptz created_at
    }

    ALERTS {
        bigint id PK
        bigint detection_id FK "Nullable"
        varchar alert_type
        varchar severity "INFO | WARNING | CRITICAL"
        text description
        varchar status "OPEN | RESOLVED | ARCHIVED"
        bigint handled_by FK "Nullable -> users.id"
        timestamptz created_at
        timestamptz handled_at
    }

    DEVICE_LOGS {
        bigint id PK
        bigint device_id FK
        uuid command_id UK
        varchar command
        varchar result "PENDING | ACCEPTED | REJECTED | TIMEOUT | ERROR"
        jsonb detail
        timestamptz created_at
        timestamptz completed_at
    }

    SYSTEM_LOGS {
        bigint id PK
        bigint user_id FK "Nullable -> users.id"
        varchar action
        text description
        timestamptz created_at
    }
```

---

## 2. Chi tiết các Bảng và Ràng buộc Toàn vẹn (Tuần 4)

1. **`users`**: Lưu tài khoản người dùng. `username` là duy nhất (`UNIQUE`); `password_hash` lưu chuỗi băm `bcrypt`; `role` giới hạn trong `('ADMIN', 'OPERATOR')`.
2. **`cameras`**: Lưu thông tin kết nối camera IP. Không cho phép xóa camera nếu đã phát sinh lịch sử trong `detection_events` (`ON DELETE RESTRICT` & kiểm tra ở tầng API trả `409 Conflict`).
3. **`devices`**: Lưu danh sách thiết bị IoT. `device_code` (`UNIQUE`) dùng làm định danh trong topic MQTT `security/device/{device_id}/...`. Các cột `latest_pir`, `latest_mq2`, `relay`, `buzzer`, `lock_engaged` được denormalize để truy vấn nhanh trạng thái hiện tại trên Dashboard.
4. **`sensors`**: Lưu chuỗi thời gian (time-series) của từng loại cảm biến. Đánh chỉ mục kết hợp `(device_id, sensor_type, created_at DESC)`.
5. **`detection_events`**: Lưu toàn bộ sự kiện do AI gửi về, ràng buộc `0 <= confidence <= 1`.
6. **`alerts`**: Lưu cảnh báo an ninh (có thể gắn với `detection_id` hoặc cảnh báo thiết bị/hệ thống độc lập khi `detection_id IS NULL`). Lưu người xử lý (`handled_by`) và thời điểm xử lý (`handled_at`).
7. **`device_logs`**: Lưu nhật ký điều khiển thiết bị IoT với `command_id` duy nhất để đối chiếu bản tin `ACK` từ ESP32.
8. **`system_logs`**: Lưu nhật ký kiểm toán hệ thống (đăng nhập, thêm/sửa/xóa camera, xử lý cảnh báo, nhận sự kiện AI).

---

## 3. Khởi tạo Schema và Dữ liệu Mẫu (Tuần 4)

- **File SQL khởi tạo**: [`database/init.sql`](../database/init.sql) tạo đầy đủ 8 bảng và các B-tree index tối ưu truy vấn theo thời gian.
- **File dữ liệu mẫu SQL**: [`database/seed.sql`](../database/seed.sql) cung cấp dữ liệu mẫu cho 4 camera, 3 thiết bị ESP32, lịch sử cảm biến, 5 sự kiện nhận diện AI, 4 cảnh báo, nhật ký điều khiển và nhật ký hệ thống.
- **Tự động khởi tạo khi chạy Backend**: Khi `SEED_DEMO_DATA=true` (mặc định trong môi trường phát triển), hàm `seed_demo_data_if_empty()` trong [`backend/app/services/__init__.py`](../backend/app/services/__init__.py) tự động nạp bộ dữ liệu mẫu nếu CSDL đang trống, giúp nhóm review ngay giao diện Tổng quan với dữ liệu thật từ API.
