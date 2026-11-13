# Thiết kế Giao diện, Wireframe & Quy ước Frontend (Tuần 2 – Tuần 4)

Tài liệu này tổng hợp kết quả công việc của **Thành viên 2 (Frontend)** từ Tuần 2 đến Tuần 4: phác thảo wireframe các màn hình chính, quy ước thiết kế (Design System) và trạng thái hoàn thiện màn hình **Tổng quan (Overview Dashboard)**.

---

## 1. Quy ước Thiết kế & Component (Tuần 3)

### 1.1. Bảng màu (Color Palette)
- **Nền ứng dụng (`--bg`)**: `#f5f7fb` (Xám xanh sáng giúp dịu mắt khi giám sát lâu).
- **Bề mặt thẻ/panel (`--card`)**: `#ffffff` với viền `#e6ebf2` và bo góc `12px`.
- **Màu nhấn chính (`--primary`)**: `#2563eb` (Xanh dương điều hướng & nút tác vụ chính).
- **Màu ngữ nghĩa theo mức độ an ninh**:
  - **Online / Resolved / An toàn**: `#16a34a` (Nền `#dcfce7`)
  - **Warning / Chú ý / Khí gas**: `#d97706` (Nền `#fef3c7`)
  - **Critical / Offline / Xâm nhập**: `#dc2626` (Nền `#fee2e2`)
  - **Info / Hệ thống**: `#2563eb` (Nền `#dbeafe`)

### 1.2. Bố cục Đáp ứng (Responsive Layout)
- **Desktop (`> 1024px`)**: Sidebar cố định bên trái (`232px`), vùng làm việc chính (`workspace`) gồm `topbar` trên cùng và lưới nội dung多cột (`.stats` 6 cột, `.grid-main` 2 cột tỷ lệ `1.55fr : 1fr`).
- **Tablet (`<= 1024px`)**: Lưới `.stats` thu về 3 cột, `.grid-main` và `.grid-equal` xếp chồng 1 cột.
- **Mobile (`<= 760px`)**: Sidebar ẩn sang dạng ngăn kéo trượt (`off-canvas drawer`) mở bằng nút Hamburger `#ss-menu`; bảng dữ liệu hỗ trợ cuộn ngang mượt (`.table-wrap`).

---

## 2. Wireframe các Màn hình Chính (Tuần 2)

### 2.1. Màn hình Tổng quan (`data-page="overview"`)
```text
+------------------+---------------------------------------------------------------+
| SMARTSECURE      | [Menu] SmartSecure / Tổng quan    [Ổn định] [Tìm kiếm] [User] |
|                  +---------------------------------------------------------------+
| ĐIỀU HÀNH        | TỔNG QUAN HỆ THỐNG                      [Cập nhật] [Làm mới]  |
| * Tổng quan      | +--------+ +--------+ +--------+ +--------+ +--------+ +----+ |
|   Giám sát Cam   | |Tổng Cam| |Cam Bật | |TB IoT  | |SK AI   | |Cảnh báo| |Crit| |
|   Giám sát AI    | +--------+ +--------+ +--------+ +--------+ +--------+ +----+ |
|   Thiết bị IoT   | +-------------------------------------+ +-------------------+ |
|   Cảnh báo (3)   | | Biểu đồ cột: Sự kiện & Cảnh báo 7d  | | Biểu đồ tròn: AI  | |
|   Lịch sử        | +-------------------------------------+ +-------------------+ |
|                  | +-------------------------------------+ +-------------------+ |
| QUẢN TRỊ         | | Bảng: Cảnh báo mới nhất (#ALT-...)  | | Trạng thái TB     | |
|   Quản lý Cam    | +-------------------------------------+ +-------------------+ |
|   Người dùng     | +-----------------------------------------------------------+ |
|   Trạng thái HT  | | Nhật ký hoạt động gần đây & Xem nhanh 3 Camera ưu tiên    | |
+------------------+---------------------------------------------------------------+
```

### 2.2. Màn hình Giám sát Camera & Quản lý Camera (`monitor` / `cameras`)
```text
+----------------------------------------------------------------------------------+
| GIÁM SÁT CAMERA                                     [10/12 trực tuyến] [Làm mới] |
| [Tìm kiếm tên/vị trí...] [Khu vực: Tất cả v] [Trạng thái: Tất cả v]              |
| +-----------------------------------+  +-----------------------------------+     |
| | [LIVE] Khung hình + Bounding Box  |  | [LIVE] Khung hình + Bounding Box  |     |
| | CAM-01 · Cổng chính      [Online] |  | CAM-02 · Sảnh lễ tân     [Online] |     |
| | [Xem chi tiết]                    |  | [Xem chi tiết]                    |     |
| +-----------------------------------+  +-----------------------------------+     |
+----------------------------------------------------------------------------------+
```

### 2.3. Màn hình Cảnh báo An ninh (`alerts`)
```text
+----------------------------------------------------------------------------------+
| QUẢN LÝ CẢNH BÁO                                                 [Xuất báo cáo]  |
| [Tổng hôm nay: 18]   [Chưa xử lý: 3]   [Đang xử lý: 2]   [Đã giải quyết: 13]     |
| [Tìm mã/nội dung...] [Mức độ: Tất cả v] [Trạng thái: Tất cả v] [Đặt lại]         |
| +------------------------------------------------------------------------------+ |
| | [x] Mã        | Nội dung                 | Nguồn   | Mức độ   | Trạng thái   | |
| | [ ] #ALT-0001 | Phát hiện người xâm nhập | CAM-01  | CRITICAL | Chưa xử lý   | |
| +------------------------------------------------------------------------------+ |
+----------------------------------------------------------------------------------+
```

### 2.4. Màn hình Thiết bị IoT (`iot`) & Lịch sử Sự kiện (`history`)
```text
+---------------------------------------+------------------------------------------+
| GIÁM SÁT CẢM BIẾN (ESP32_001)         | ĐIỀU KHIỂN THIẾT BỊ (Chờ ACK)            |
| [PIR: Bình thường] [MQ-2: 350 ppm]    | - Relay Đèn khu vực A       [Bật/Tắt]    |
| [Nhiệt độ: 27.4°C]                    | - Còi báo động              [Bật/Tắt]    |
|                                       | - Khóa điện cửa chính       [Mở khóa]    |
+---------------------------------------+------------------------------------------+
| BẢNG THIẾT BỊ ĐÃ ĐĂNG KÝ / DÒNG THỜI GIAN SỰ KIỆN TẬP TRUNG                      |
+----------------------------------------------------------------------------------+
```

---

## 3. Kết nối Dữ liệu Mẫu trên Trang Tổng quan (Tuần 4)

Trong [`frontend/js/api.js`](../frontend/js/api.js), màn hình **Tổng quan** đã được kết nối đồng bộ với Backend API:
- `GET /api/statistics/overview` -> Cập nhật 6 thẻ thống kê, chú thích chân thẻ và số lượng cảnh báo mở trên thanh điều hướng.
- `GET /api/statistics/detections?days=7` -> Cập nhật tổng số đối tượng và tỷ lệ phần trăm trong biểu đồ phân loại AI.
- `GET /api/alerts?limit=5` -> Render bảng cảnh báo mới nhất với đầy đủ huy hiệu màu `CRITICAL` / `WARNING` / `INFO` và trạng thái xử lý.
- `GET /api/cameras` & `GET /api/devices` -> Hiển thị danh sách trạng thái trực tuyến/ngoại tuyến của Camera và ESP32.
- `GET /api/logs/system?limit=5` -> Hiển thị danh sách hoạt động gần đây của hệ thống.
