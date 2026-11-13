---
name: smartsecure-web
description: >-
  Hoàn thiện, vận hành và kiểm thử toàn bộ chức năng trên Web Dashboard SmartSecure
  (Security Monitoring System), bao gồm tự động kiểm tra và cập nhật trạng thái thực tế
  của từng dịch vụ/thiết bị (có thì tự hiện hoạt động kèm hiệu ứng animation chấm sáng lan tỏa,
  thiếu/tắt thì hiện tạm thời chưa hoạt động), phân quyền người dùng (ADMIN / OPERATOR),
  và chức năng Admin thêm/quản lý tài khoản.
---

# SmartSecure Full Web Functions, Auto Health Status, Animations & RBAC Skill

Sử dụng skill này khi người dùng gọi `/smartsecure-web` hoặc yêu cầu **tự động cập nhật trạng thái hệ thống theo thực tế**, **tạo hiệu ứng animation cho các chấm/nút trạng thái (`.dot`) và nút tương tác**, cùng **Phân quyền người dùng (`ADMIN` / `OPERATOR`), Admin thêm tài khoản**, và **hoàn thiện toàn bộ chức năng Web Dashboard**.

---

## 0. Cơ chế Tự động Kiểm tra Trạng thái & Hiệu ứng Animation Chấm Trạng thái (`.dot`)

### 0.1. Hiệu ứng Animation cho Chấm Trạng thái (`.dot`) và Nút bấm
1. **Chấm trạng thái đang hoạt động (`.dot` xanh lá — `online` / `active`)**:
   - Phải có hiệu ứng **nhấp nháy sóng lan tỏa (Pulse / Ripple Halo Animation)** mượt mà liên tục bằng CSS `@keyframes` (`::before` / `::after` hoặc `box-shadow` lan tỏa từ `0px` ra `6px-8px` với độ mờ giảm dần `opacity: 0.7 -> 0` trong `1.8s - 2s infinite ease-out`).
   - Áp dụng cho tất cả các chấm `.dot` đang ở trạng thái **Hoạt động / Trực tuyến** trên toàn giao diện:
     - Các thẻ dịch vụ đang hoạt động ở trang **Trạng thái hệ thống (`system`)** (`#ss-sys-api`, `#ss-sys-db`, v.v. khi online).
     - Nhãn `.system-pill .dot` trên thanh Header và đầu các trang khi hệ thống/thiết bị trực tuyến.
     - Thẻ `.vpn-card .dot` khi WireGuard VPN thực sự đang bật.
     - Các badge `.status.online` của Camera và Thiết bị IoT đang trực tuyến.
2. **Chấm trạng thái cảnh báo / nghiêm trọng (`warning` / `critical` / `.camera-live`)**:
   - Có hiệu ứng pulse màu cam (`#f59e0b`) hoặc đỏ (`#ef4444`) nhanh hơn (`1.2s infinite`) để thu hút sự chú ý.
3. **Chấm trạng thái "Tạm thời chưa hoạt động" / Ngoại tuyến (`offline` / `inactive`)**:
   - Đổi sang **màu xám tĩnh (`#94a3b8`)**, **tắt hoàn toàn animation (`animation: none; box-shadow: none`)** để nhìn vào phân biệt được ngay mục nào đang chạy thật (có sóng xanh chuyển động) và mục nào đang tạm nghỉ/chưa hoạt động (chấm xám đứng yên).
4. **Animation cho các nút bấm & công tắc (`.btn`, `.toggle`, `.nav button`)**:
   - Khi hover/click có hiệu ứng chuyển động mượt (`transform: translateY(-1px)`, `active: scale(0.97)`), nút `"Làm mới"` / `"Kiểm tra lại"` có icon `<i data-lucide="refresh-cw">` xoay tròn (`spin` 360 độ) khi đang tải dữ liệu.

### 0.2. Nguyên tắc Tự động Kiểm tra & Ẩn/Hiện theo Thực tế (Dynamic Auto-Status)
- **Không hardcode cứng trạng thái**: Backend (`GET /api/system/health` + `/api/statistics/overview`) tự động dò thực tế từng thành phần và đồng bộ lên Frontend (qua WebSocket + Polling định kỳ 5–10 giây + nút `"Kiểm tra lại"`):
  1. **Backend API & PostgreSQL**: Kiểm tra `SELECT 1`. Đang chạy -> Bật chấm xanh có animation pulse `Hoạt động tốt`. Lỗi -> Chấm đỏ/xám.
  2. **MQTT & Mạch ESP32**: Kiểm tra kết nối socket tới MQTT Broker và số thiết bị `ONLINE`. Có kết nối -> Bật chấm xanh có animation pulse và hiện thông số cảm biến (`PIR`, `MQ-2`, `Relay/Lock`). Chưa có -> Chấm xám tĩnh `Tạm thời chưa hoạt động`.
  3. **Luồng Camera RTSP**: Có camera `ONLINE` -> Bật chấm xanh có animation pulse. Chưa có -> Chấm xám tĩnh `Tạm thời chưa hoạt động`.
  4. **AI Worker (YOLOv8)**: Ping `http://ai-service:8001/health` (hoặc `localhost:8001/health`). Phản hồi 200 -> Bật chấm xanh có animation pulse. Chưa bật -> Chấm xám tĩnh `Tạm thời chưa hoạt động`.
  5. **WireGuard VPN**: Kiểm tra interface `wg0` thực tế. Đang chạy -> Bật chấm xanh có animation pulse. Chưa cấu hình -> Chấm xám tĩnh `Tạm thời chưa hoạt động`.

---

## 1. Ma trận Phân quyền Người dùng (RBAC: `ADMIN` vs `OPERATOR`)

| Trang / Chức năng (`data-view`) | Quyền `ADMIN` (Quản trị viên) | Quyền `OPERATOR` (Nhân viên vận hành) | Quy tắc Tự động Ẩn/Hiện theo Quyền |
| :--- | :--- | :--- | :--- |
| **1. Tổng quan (`overview`)** | Xem toàn bộ thống kê & biểu đồ | Xem toàn bộ thống kê & biểu đồ | Hiển thị cho cả 2 vai trò |
| **2. Giám sát Camera (`monitor`)** | Xem danh sách, tìm kiếm, lọc | Xem danh sách, tìm kiếm, lọc | Hiển thị cho cả 2 vai trò |
| **3. Giám sát AI (`ai`)** | Xem, lọc theo đối tượng, mức độ, từ khóa | Xem, lọc theo đối tượng, mức độ, từ khóa | Hiển thị cho cả 2 vai trò |
| **4. Thiết bị IoT (`iot`)** | Xem cảm biến, Điều khiển, **Thêm/Xóa thiết bị** | Xem cảm biến & Điều khiển (`Relay`, `Còi`, `Khóa`) | Tự động **ẩn** nút `Thêm thiết bị` và `Xóa` nếu là `OPERATOR` |
| **5. Cảnh báo an ninh (`alerts`)** | Xem, lọc, **Xử lý cảnh báo (`Resolve`)** | Xem, lọc, **Xử lý cảnh báo (`Resolve`)** | Cả 2 vai trò đều thấy nút `Xử lý` trên cảnh báo `OPEN` |
| **6. Lịch sử sự kiện (`history`)** | Xem timeline, phân bố & lọc bảng sự kiện | Xem timeline, phân bố & lọc bảng sự kiện | Hiển thị cho cả 2 vai trò |
| **7. Quản lý Camera (`cameras`)** | **Thêm / Sửa / Xóa Camera** | Chỉ xem danh sách | Tự động **ẩn** nút `Thêm Camera`, `Sửa`, `Xóa` nếu không phải `ADMIN` |
| **8. Quản lý người dùng (`users`)** | **Xem danh sách, Thêm tài khoản, Đổi vai trò, Khóa/Mở khóa** | **Tự động ẩn mục menu & trang Quản lý người dùng** | Chỉ hiển thị menu `users` và nút `Thêm tài khoản` khi `role === 'ADMIN'` |
| **9. Trạng thái hệ thống (`system`)** | Xem & kiểm tra lại toàn bộ dịch vụ | Xem trạng thái dịch vụ | Hiển thị cho cả 2 vai trò |
| **10. Cài đặt hệ thống (`settings`)** | Xem & cập nhật ngưỡng cảnh báo | Chỉ xem thông số cấu hình | Tự động ẩn nút lưu cấu hình nếu không phải `ADMIN` |

---

## 2. Chức năng Admin Thêm & Quản lý Tài khoản (`data-page="users"`)

- **Backend**:
  - `GET /api/users`: Danh sách người dùng.
  - `POST /api/users` (`Depends(admin_user)`): Tạo người dùng mới với `username`, `password`, `full_name`, `role` (`ADMIN` / `OPERATOR`), `is_active`.
  - `PUT /api/users/{user_id}` (`Depends(admin_user)`): Đổi vai trò (`ADMIN` <-> `OPERATOR`) hoặc Khóa/Mở khóa (`is_active`).
- **Frontend**:
  - Nút `Thêm tài khoản` (`data-modal="user"`) mở modal nhập Họ tên, Tên đăng nhập, Mật khẩu, Vai trò -> Gọi `POST /api/users` -> Cập nhật bảng `#ss-users-tbody`.
  - Có nút **Đăng xuất** trên góc phải / sidebar để đổi tài khoản kiểm tra phân quyền.
