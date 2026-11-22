# Thiết kế Giao diện, Wireframe & Quy ước Frontend PCCC (Tuần 2 – Tuần 4)

Tài liệu tổng hợp thiết kế giao diện của **Thành viên 2 (Frontend)** cho **Web Dashboard PCCC SmartSecure**, phục vụ giám sát phát hiện **Lửa (`fire`) & Khói (`smoke`)**, cảm biến **MQ-2**, điều khiển ứng cứu **Còi 12V / Khóa từ Solenoid thoát hiểm / Mạch Relay** và theo dõi tình trạng **WireGuard VPN / RTSP / Edge AI**.

---

## 1. Quy ước Thiết kế & Component (Design System)

### 1.1. Bảng màu PCCC & Chế độ Sáng / Tối (SOC Dark Mode)
- Hỗ trợ chuyển đổi tức thì giữa **Enterprise Light** (`--ss-bg: #f5f7fb`, `--ss-surface: #ffffff`) và **Cyber Dark SOC** (`data-theme="dark"`) dành cho phòng trực ban PCCC ban đêm.
- Thanh điều hướng trái cố định (Executive Dark Sidebar `252px`) kèm thẻ tình trạng **WireGuard VPN** ở góc dưới.
- **Màu ngữ nghĩa theo cấp độ PCCC**:
  - **An toàn / Đang hoạt động / Đã xử lý (`ONLINE` / `RESOLVED`)**: `#10b981` — chấm trạng thái `.dot` xanh lá có hiệu ứng **sóng lan tỏa (pulse/ripple halo)** mượt mà.
  - **Cảnh báo sớm Khói (`smoke`) / MQ-2 tăng (`WARNING`)**: `#f59e0b` — chấm cam nhấp nháy cảnh báo (`1.25s`).
  - **Báo cháy Khẩn cấp Lửa (`fire`) / MQ-2 vượt ngưỡng nguy hiểm (`CRITICAL`)**: `#ef4444` — chấm đỏ nhấp nháy khẩn cấp (`1.15s`).
  - **Tạm thời chưa hoạt động / Ngoại tuyến (`OFFLINE`)**: `#94a3b8` — chấm **màu xám tĩnh** (tắt hoàn toàn animation), tuyệt đối không báo xanh giả khi chưa kết nối phần cứng/dịch vụ.

### 1.2. Bố cục Đáp ứng (Responsive Layout)
- **Desktop (`> 1200px`)**: Sidebar cố định `252px`, lưới `.stats` 6 cột, `.grid-main` tỷ lệ `1.75fr : 1fr`, bảng dữ liệu thẳng hàng chuẩn Enterprise.
- **Tablet (`<= 1200px`)**: `.stats` và `.service-grid` chuyển sang 3 cột.
- **Mobile (`<= 820px`)**: Sidebar chuyển thành ngăn kéo trượt (`off-canvas drawer`) qua nút `#ss-menu`; bảng dữ liệu cuộn ngang mượt (`.table-wrap`).

---

## 2. Wireframe các Màn hình Trọng tâm PCCC

### 2.1. Tổng quan PCCC (`data-page="overview"`)
```text
+------------------+---------------------------------------------------------------+
| SMARTSECURE PCCC | [Menu] SmartSecure / Tong quan   [Trang thai] [Tim] [Theme]   |
|                  +---------------------------------------------------------------+
| DIEU HANH        | TONG QUAN GIAM SAT PCCC                   [Cap nhat] [Lam moi]|
| * Tong quan      | +--------+ +--------+ +--------+ +--------+ +--------+ +----+ |
|   Giam sat Cam   | |Tong Cam| |Cam Bat | |ESP32   | |Lua/Khoi| |Canh bao| |Crit| |
|   Giam sat AI    | +--------+ +--------+ +--------+ +--------+ +--------+ +----+ |
|   Thiet bi IoT   | +-------------------------------------+ +-------------------+ |
|   Canh bao (2)   | | Bieu do cot: Phat hien & Canh bao   | | Ti le Lua / Khoi  | |
|   Lich su        | +-------------------------------------+ +-------------------+ |
|                  | +-------------------------------------+ +-------------------+ |
| QUAN TRI         | | Canh bao chay moi nhat (#ALT-...)   | | Tinh trang ESP32  | |
|   Quan ly Cam    | +-------------------------------------+ +-------------------+ |
|   Nguoi dung*    | +-----------------------------------------------------------+ |
|   Trang thai HT  | | Nhat ky hoat dong & Xem nhanh Camera khu vuc trong diem   | |
|   Cai dat        | +-----------------------------------------------------------+ |
| [WireGuard VPN]  | (*Muc Quan ly nguoi dung tu dong an voi quyen OPERATOR)       |
+------------------+---------------------------------------------------------------+
```

### 2.2. Giám sát AI Lửa & Khói (`data-page="ai"`)
```text
+----------------------------------------------------------------------------------+
| GIAM SAT AI (YOLOv8 LUA & KHOI)          [AI Worker: Trang thai] [Gui su kien]   |
| [Tat ca] [Lua (Fire)] [Khoi (Smoke)] [Khac] [Camera v] [Su kien v] [Tin cay v]   |
| +---------------------------+  +---------------------------+  +----------------+ |
| | [Anh Snapshot That]  #12  |  | [Anh Snapshot That]  #11  |  | [Fallback Img] | |
| | [fire · 94.2%]            |  | [smoke · 87.5%]           |  | [smoke · 79%]  | |
| | CAM-01 · Nha kho Hoa chat |  | CAM-03 · Tang ham Bai xe  |  | CAM-02 · Xuong | |
| | [=== 94.2% ===] Chi tiet  |  | [=== 87.5% ===] Chi tiet  |  | [== 79% ==]    | |
| +---------------------------+  +---------------------------+  +----------------+ |
| BANG SU KIEN AI (ID | Snapshot | Camera | Doi tuong | Loai | Tin cay | Chi tiet) |
+----------------------------------------------------------------------------------+
```

### 2.3. Quản lý Camera PCCC (`data-page="cameras"`) & Bảo toàn RTSP
```text
+----------------------------------------------------------------------------------+
| QUAN LY CAMERA PCCC                                          [+ Them Camera*]    |
| [Tim ten/vi tri/IP...] [Khu vuc: Tat ca v] [Trang thai: Tat ca v]                |
| +------------------------------------------------------------------------------+ |
| | ID  | Camera                 | Vi tri          | RTSP (Da che MK)  | Thao tac| |
| | #1  | CAM-01 · Nha kho HC    | Nha kho A       | rtsp://192...     | Sua Xoa | |
| | #3  | CAM-03 · Tang ham B1   | Tang ham Bai xe | rtsp://192...     | Sua Xoa | |
| +------------------------------------------------------------------------------+ |
| Modal Sua Camera: Hien ro camera_id #1, de trong o RTSP de giu nguyen URL goc    |
+----------------------------------------------------------------------------------+
```

### 2.4. Thiết bị IoT Ứng cứu PCCC (`data-page="iot"`)
```text
+---------------------------------------+------------------------------------------+
| GIAM SAT CAM BIEN (ESP32_KHO_01)      | DIEU KHIEN UNG CUU PCCC (Cho ACK)        |
| - Cam bien Lua / Hong ngoai: An toan  | - Mach Relay (Bom / Phun suong) [Bat/Tat]|
| - Cam bien Khoi/Gas MQ-2:   320 ppm   | - Coi bao dong 12V              [Bat/Tat]|
| - Khoa tu Solenoid Thoat hiem: Khoa   | - Khoa tu Solenoid Thoat hiem   [Mo/Khoa]|
+---------------------------------------+------------------------------------------+
| BANG THIET BI ESP32 DA DANG KY (An/Khoa dieu khien khi mach ESP32 dang Offline)  |
+----------------------------------------------------------------------------------+
```

### 2.5. Trạng thái Hạ tầng (`data-page="system"`) & Mở rộng Grafana
- Tự động thăm dò thực tế `GET /api/system/health` mỗi 6 giây cho 6 thành phần: **Backend API (Rust)**, **PostgreSQL**, **WireGuard VPN**, **MQTT & Mạch ESP32**, **Luồng Camera RTSP (MediaMTX)** và **AI Worker (YOLOv8 Lửa & Khói)**.
- Nút **Kiểm tra lại** có hiệu ứng xoay icon mượt; các dịch vụ đang bật hiển thị chấm xanh lan tỏa, dịch vụ chưa bật hiển thị chấm xám tĩnh `"Tạm thời chưa hoạt động"`.
- Khi bật profile `monitoring` trong Docker Compose, trực ban/quản trị có thể mở thêm **Dashboard Grafana (`:3000`)** để phân tích xu hướng nồng độ khói/gas MQ-2 và biểu đồ sự cố PCCC dài hạn.
