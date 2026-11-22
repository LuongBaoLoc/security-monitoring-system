# Hạ tầng Mạng WireGuard VPN & Phân đoạn VLAN — Hệ thống PCCC

Thư mục này chứa tài liệu cấu hình **WireGuard Server Container** và kiến trúc **Phân đoạn VLAN** dùng để bảo vệ hạ tầng giám sát và báo động Phòng cháy Chữa cháy (**SmartSecure PCCC**).

---

## 1. Kiến trúc Phân đoạn VLAN Đề xuất

Để ngăn thiết bị biên (Camera IP hoặc mạch ESP32) bị lợi dụng tấn công ngược vào máy chủ quản trị hoặc Internet, mạng được chia thành **3 VLAN độc lập**:

| VLAN ID | Tên phân đoạn | Dải mạng gợi ý | Thiết bị / Dịch vụ | Quy tắc Tường lửa (ACL) |
| :---: | :--- | :--- | :--- | :--- |
| **VLAN 10** | **Camera & IoT Hiện trường** | `192.168.10.0/24` | Camera IP RTSP, Webcam USB gateway, ESP32 (MQ-2, Còi 12V, Solenoid, Relay) | Chỉ cho phép kết nối ra cổng `8554/TCP` (MediaMTX) và `1883/TCP` (Mosquitto) ở VLAN 20. **Chặn Internet và chặn truy cập VLAN 30**. |
| **VLAN 20** | **Edge AI & Backend Core** | `192.168.20.0/24` | Server Docker (MediaMTX, AI YOLOv8 Lửa/Khói, Backend Rust, PostgreSQL, Mosquitto, Grafana) | Chỉ nhận RTSP/MQTT từ VLAN 10 và nhận HTTP/WS quản trị từ VLAN 30 / đường hầm WireGuard. Chặn `/api/internal/*` từ ngoài Docker network. |
| **VLAN 30** | **Quản trị & Trực ban VPN** | `10.13.13.0/24` (WireGuard) | Máy tính/Điện thoại của `ADMIN` và trực ban PCCC `OPERATOR` | Truy cập Web Dashboard (`:8080`) và Grafana (`:3000`) **chỉ qua đường hầm mã hóa WireGuard VPN (`UDP :51820`)**. |

---

## 2. Khởi chạy WireGuard Server Container qua Docker Compose

Trong `docker-compose.yml`, dịch vụ `wireguard` nằm trong profile `vpn` (không tự bật khi chạy mặc định để tiết kiệm tài nguyên trên máy phát triển):

```powershell
# Bật riêng WireGuard VPN Container
docker compose --profile vpn up -d

# Hoặc bật toàn bộ stack mở rộng (RTSP MediaMTX + WireGuard VPN + Grafana)
docker compose --profile full up -d
```

Sau khi khởi chạy:
1. File cấu hình client (peer) và mã QR sẽ tự động sinh tại `wireguard/config/peer_admin/` và `wireguard/config/peer_operator1/`.
2. Xem mã QR trên terminal để quét bằng ứng dụng WireGuard trên điện thoại/máy tính:
   ```powershell
   docker compose exec wireguard /app/show-peer admin
   ```
3. Đặt `WIREGUARD_ACTIVE=true` trong `.env` (hoặc cấu hình giao diện mạng `wg0` trên host Linux) rồi khởi động lại `backend`:
   ```powershell
   docker compose up -d backend
   ```
   Trang **Trạng thái hệ thống** và góc dưới Sidebar trên Web Dashboard sẽ tự động phát hiện đường hầm hoạt động và chuyển chấm trạng thái **WireGuard VPN** sang màu xanh có hiệu ứng sóng lan tỏa.

---

## 3. Lưu ý Bảo mật

- **Không bao giờ commit** thư mục `wireguard/config/` (chứa Private Key của Server và các Peer) lên Git.
- Khi triển khai thực tế, chỉ mở duy nhất cổng `51820/UDP` trên Router/Firewall biên và đóng các cổng `8080`, `8000`, `5432`, `1883`, `3000` khỏi Internet công cộng.
