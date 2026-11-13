# AI Detection Service (Thành viên 3)

Dịch vụ AI nhận khung hình tại `POST /detect?camera_id=...`, chạy mô hình YOLOv8 (Ultralytics), sau đó gửi các sự kiện phát hiện về Backend qua `POST /api/detections`.

## Cấu trúc thư mục

- `app/main.py`: FastAPI endpoint `/health` và `/detect`.
- `app/camera.py`: Hàm mở luồng RTSP/camera (`open_camera`), chụp 1 khung hình JPEG (`capture_frame_bytes`) và sinh ảnh mẫu thử nghiệm (`create_sample_test_image`).
- `app/detector.py`: Nạp model YOLOv8 và trích xuất danh sách đối tượng (`object_type`, `confidence`, `event_type`).
- `app/event_handler.py`: Gửi JSON sự kiện phát hiện sang Backend kèm `AI_SERVICE_TOKEN`.
- `test_yolo_sample.py`: Script kiểm thử đọc frame và chạy inference YOLO trên ảnh mẫu (đầu ra Tuần 3 – Tuần 4).

## Cài đặt & chạy thử YOLO trên ảnh mẫu (Tuần 3 – Tuần 4)

```powershell
cd ai-service
python -m venv .venv
.\.venv\Scripts\Activate.ps1
pip install -r requirements.txt

# Chạy thử với ảnh mẫu tự sinh (hoặc truyền --image / --rtsp)
python test_yolo_sample.py
```

Khi đặt file trọng số YOLOv8 (ví dụ `yolov8n.pt`) vào thư mục `models/` và đặt biến môi trường `MODEL_PATH=../models/yolov8n.pt`, script sẽ chạy inference thật bằng `ultralytics` và in thời gian xử lý (`inference_time_ms`) cùng danh sách bounding box/nhãn phát hiện.
