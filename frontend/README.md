# SmartSecure Frontend

Frontend prototype bằng HTML, CSS và JavaScript thuần. Có thể mở `index.html` trực tiếp trong trình duyệt hoặc chạy một static server tại thư mục này:

```powershell
cd frontend
python -m http.server 5500
```

Sau đó truy cập `http://localhost:5500`.

## Nội dung

- 10 màn hình sau khi truy cập hệ thống: tổng quan, giám sát camera, giám sát AI, IoT, cảnh báo, lịch sử, quản lý camera, người dùng, trạng thái hệ thống và cài đặt. Không có màn hình đăng nhập/đăng ký.
- Dashboard có biểu đồ, cảnh báo mới và camera ưu tiên; trang giám sát camera có lưới camera cùng bộ lọc khu vực/trạng thái và xem chi tiết.
- Điều hướng sidebar responsive, modal thêm/sửa, hộp thoại xác nhận, toast minh họa và các điều khiển giao diện cục bộ.
- Biểu đồ, trạng thái, lịch sử và ảnh camera là dữ liệu/đồ họa minh họa được dựng trong CSS/HTML.

## Giới hạn prototype

Không có xác thực thật, không lưu dữ liệu, không gọi REST/WebSocket, không phát luồng camera, không gửi MQTT và không điều khiển thiết bị. Thư viện Lucide tải từ CDN để hiển thị icon; cần kết nối Internet để icon xuất hiện. Các hành động trong giao diện chỉ thay đổi trạng thái minh họa trên trình duyệt.
