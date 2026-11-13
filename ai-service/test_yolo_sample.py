"""Script thử nghiệm đọc frame và chạy inference YOLOv8 trên ảnh mẫu (Tuần 3 - Tuần 4).

Cách chạy:
    cd ai-service
    python test_yolo_sample.py
    python test_yolo_sample.py --image path/to/sample.jpg
    python test_yolo_sample.py --rtsp rtsp://192.168.1.21:554/stream1
"""
import argparse
import json
import time
from pathlib import Path

from app.camera import capture_frame_bytes, create_sample_test_image
from app.config import CONFIDENCE_THRESHOLD, MODEL_PATH
from app.detector import detect, get_model


def main() -> None:
    parser = argparse.ArgumentParser(description="Test đọc frame camera và chạy YOLOv8 trên ảnh mẫu")
    parser.add_argument("--image", type=str, help="Đường dẫn ảnh mẫu (.jpg/.png)")
    parser.add_argument("--rtsp", type=str, help="Đường dẫn RTSP hoặc đường dẫn video để thử đọc frame")
    parser.add_argument("--save-sample", type=str, default="sample_frame.jpg", help="Lưu frame mẫu ra file để kiểm tra")
    args = parser.parse_args()

    if args.rtsp:
        print(f"[1/3] Đang đọc frame từ nguồn stream: {args.rtsp} ...")
        frame_bytes = capture_frame_bytes(args.rtsp)
        source_desc = f"stream:{args.rtsp}"
    elif args.image:
        img_path = Path(args.image)
        print(f"[1/3] Đang đọc ảnh mẫu từ file: {img_path} ...")
        frame_bytes = img_path.read_bytes()
        source_desc = f"file:{img_path}"
    else:
        print("[1/3] Không truyền --image/--rtsp, đang tạo ảnh mẫu thử nghiệm (640x480) ...")
        frame_bytes = create_sample_test_image()
        Path(args.save_sample).write_bytes(frame_bytes)
        source_desc = f"generated:{args.save_sample}"

    print(f"      -> Đã nạp frame thành công ({len(frame_bytes)} bytes, nguồn: {source_desc}).")

    print(f"[2/3] Kiểm tra cấu hình model YOLO (MODEL_PATH={MODEL_PATH or 'chưa đặt'}, CONF={CONFIDENCE_THRESHOLD}) ...")
    start_t = time.perf_counter()
    model = get_model()
    if model is None:
        print("      -> Chưa có file trọng số .pt tại MODEL_PATH. Chạy chế độ mô phỏng đầu ra chuẩn contract Tuần 4:")
        detections = [
            {
                "object_type": "person",
                "confidence": 0.942,
                "event_type": "person",
            }
        ]
        mode = "simulated_contract"
    else:
        detections = detect(frame_bytes)
        mode = "ultralytics_yolov8"
    elapsed_ms = (time.perf_counter() - start_t) * 1000.0

    report = {
        "status": "ok",
        "mode": mode,
        "source": source_desc,
        "frame_size_bytes": len(frame_bytes),
        "inference_time_ms": round(elapsed_ms, 2),
        "detections_count": len(detections),
        "detections": detections,
    }
    print("[3/3] Kết quả chạy thử YOLO:")
    print(json.dumps(report, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
