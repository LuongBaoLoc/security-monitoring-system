# SmartSecure Frontend

Frontend HTML, CSS và JavaScript thuần, được nối với API qua `/api` và WebSocket qua `/ws`. Khuyến nghị chạy bằng Docker Compose để Nginx chuyển tiếp các request đến backend. Có thể mở `index.html` qua static server, nhưng khi đó cần cấu hình reverse proxy cùng origin cho `/api` và `/ws`.

```powershell
cd frontend
python -m http.server 5500
```

Sau đó truy cập `http://localhost:5500`.

## Nội dung

- 10 màn hình điều hướng: tổng quan, giám sát camera, giám sát AI, IoT, cảnh báo, lịch sử, quản lý camera, người dùng, trạng thái hệ thống và cài đặt. Đăng nhập API bằng tài khoản khởi tạo qua biến môi trường.
- Dashboard có biểu đồ, cảnh báo mới và camera ưu tiên; trang giám sát camera có lưới camera cùng bộ lọc khu vực/trạng thái và xem chi tiết.
- Điều hướng sidebar responsive, modal thêm/sửa, hộp thoại xác nhận, toast minh họa và các điều khiển giao diện cục bộ.
- Biểu đồ, trạng thái, lịch sử và ảnh camera là dữ liệu/đồ họa minh họa được dựng trong CSS/HTML.

## Giới hạn hiện tại

Một số trang chuyên biệt vẫn chứa dữ liệu giao diện minh họa; dashboard tổng quan tải thống kê/cảnh báo từ API. Chưa có live video; các thẻ camera vẫn là ảnh dựng bằng CSS. Thư viện Lucide tải từ CDN để hiển thị icon.
