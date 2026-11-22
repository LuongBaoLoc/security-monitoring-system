# AI Service YOLOv8

AI Service Rust chạy ONNX Runtime trên CPU. Model PCCC gửi sự kiện `fire` và `smoke`. Model an ninh nhận diện người, ô tô, xe máy, xe buýt và xe tải. Hai model dùng chung API hiện có ở `POST /detect?camera_id=...`, rồi gửi kết quả tới `POST /api/detections` của backend.

## Trọng số

Lưu hai tệp sau trong thư mục `models/` ở gốc dự án.

| Tệp cục bộ | Model |
|---|---|
| `models/fire-smoke.onnx` | [Fire & Smoke R3-E6, tệp ONNX FP32 768](https://github.com/Supakorn289/fire-smoke-detection-r3e6/releases/tag/v1.0.0-r3e6) |
| `models/yolov8n.onnx` | [Trọng số COCO từ Ultralytics Assets](https://github.com/ultralytics/assets/releases/download/v8.4.0/yolov8n.onnx) |

`FIRE_MODEL_PATH` và `SECURITY_MODEL_PATH` cho phép chọn tệp khác theo thứ tự lớp của hai model trên. Dùng bản export YOLOv8 detect FP32 với đầu ra thô; nếu chạy một model, đặt đường dẫn của model còn lại thành chuỗi rỗng. `GET /health` hiển thị model đã tải qua trường `models`.

## Chạy thử

Docker Compose tự cung cấp ONNX Runtime, FFmpeg và kết nối backend. Từ thư mục gốc, chạy:

```powershell
docker compose up --build -d
```

Khi chạy trực tiếp, cài gói `onnxruntime==1.24.4` và đặt `ORT_DYLIB_PATH` tới thư viện bên trong `onnxruntime/capi`, chẳng hạn `onnxruntime.dll` trên Windows. [ONNX Runtime 1.24.4](https://pypi.org/project/onnxruntime/1.24.4/)

```powershell
cargo run -p security-monitoring-ai-service --bin security-monitoring-ai-service
cargo run -p security-monitoring-ai-service --bin test_yolo_sample -- --image .\anh-mau.jpg
```

Đăng ký camera qua dashboard trước khi gửi ảnh. Thay `1` bằng ID thật và dùng cùng `AI_SERVICE_TOKEN` cho AI Service và backend.

```powershell
curl.exe -H ("Authorization: Bearer " + $env:AI_SERVICE_TOKEN) -F "file=@anh-mau.jpg;type=image/jpeg" "http://localhost:8001/detect?camera_id=1"
```

## Theo dõi camera RTSP

Đặt `AI_CAMERA_ID` bằng ID camera đã đăng ký và `AI_RTSP_URL` bằng địa chỉ RTSP mà máy chạy AI truy cập được. Dùng biến môi trường cục bộ cho URL có thông tin xác thực. Khi cả hai biến được đặt, dịch vụ lấy mẫu một camera theo `AI_FRAME_INTERVAL_SECONDS` (mặc định 5 giây) bằng FFmpeg và gửi sự kiện có điểm cao nhất của mỗi lớp trong khung hình.

`CONFIDENCE_THRESHOLD` lọc dự đoán trước khi gửi. Ngưỡng cảnh báo và khoảng chống trùng tiếp tục được cấu hình trong backend qua dashboard. Cảnh báo camera hỗ trợ phát hiện sớm bên cạnh hệ thống báo cháy chuyên dụng.
