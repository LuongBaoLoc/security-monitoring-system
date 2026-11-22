# SmartSecure Web Dashboard — Giao diện Giám sát & Ứng cứu PCCC

Web Dashboard viết bằng **HTML5, CSS3 và JavaScript thuần**, phục vụ qua **Nginx** tại cổng `8080` (proxy `/api/`, `/ws/`, `/uploads/` sang Backend Rust và chặn công khai `/api/internal/`).

## Chức năng Chính

- **10 màn hình điều hành & quản trị PCCC**: Tổng quan PCCC, Giám sát Camera, Giám sát AI (Lửa & Khói kèm ảnh snapshot thật và bảng sự kiện), Thiết bị IoT ESP32 (cảm biến MQ-2, Còi 12V, Khóa từ Solenoid mở cửa thoát hiểm, Mạch Relay), Cảnh báo cháy, Lịch sử sự kiện, Quản lý Camera (hiển thị rõ `camera_id`, che mật khẩu RTSP và bảo toàn URL gốc khi sửa), Quản lý người dùng (`ADMIN`), Trạng thái hệ thống và Cài đặt ngưỡng PCCC.
- **Đồng bộ thời gian thực**: Kết nối WebSocket `/ws/dashboard` tự động nhận `detection.created`, `alert.created`, `device.status`, `device.command.updated` và tự động thăm dò thực tế `GET /api/system/health` mỗi 6 giây.
- **Trạng thái trung thực & Animation `.dot`**: Thành phần hoạt động thực tế hiện chấm xanh lá có hiệu ứng sóng lan tỏa; cảnh báo cháy/khói nhấp nháy đỏ/cam; dịch vụ chưa bật (WireGuard VPN, RTSP MediaMTX, AI Worker, ESP32) hiện chấm xám tĩnh `"Tạm thời chưa hoạt động"`.
- **Phân quyền `ADMIN` / `OPERATOR`** và chuyển đổi nhanh giao diện **Enterprise Light / Cyber Dark SOC**.

## Khởi chạy & Phát triển

Khuyến nghị chạy cùng Docker Compose (`docker compose up -d`) và truy cập **<http://localhost:8080>**. Sau khi chỉnh sửa file trong `frontend/`, nhấn **`Ctrl + F5`** trên trình duyệt để tải ngay bản mới. Khi cần trực quan hóa dài hạn nồng độ khói/gas MQ-2 và lịch sử sự cố, có thể bật thêm **Dashboard Grafana (`:3000`)** bằng `docker compose --profile monitoring up -d`.
