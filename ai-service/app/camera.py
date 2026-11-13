"""RTSP and local camera capture helpers for AI inference workers (Week 3-4)."""
from contextlib import contextmanager
from io import BytesIO

import cv2
from PIL import Image, ImageDraw


@contextmanager
def open_camera(source: str | int):
    capture = cv2.VideoCapture(source)
    try:
        if not capture.isOpened():
            raise RuntimeError(f"Unable to open camera stream: {source}")
        yield capture
    finally:
        capture.release()


def capture_frame_bytes(source: str | int, jpeg_quality: int = 85) -> bytes:
    """Read a single frame from an RTSP URL, video file, or webcam index and encode as JPEG."""
    with open_camera(source) as capture:
        ok, frame = capture.read()
        if not ok or frame is None:
            raise RuntimeError("Failed to read frame from camera stream")
        encode_params = [int(cv2.IMWRITE_JPEG_QUALITY), jpeg_quality]
        success, buffer = cv2.imencode(".jpg", frame, encode_params)
        if not success:
            raise RuntimeError("Failed to encode frame as JPEG")
        return buffer.tobytes()


def create_sample_test_image(width: int = 640, height: int = 480) -> bytes:
    """Create a synthetic test frame (JPEG bytes) for verifying the YOLO pipeline in Week 4."""
    img = Image.new("RGB", (width, height), color=(215, 226, 236))
    draw = ImageDraw.Draw(img)
    # Draw ground and building background
    draw.rectangle([0, int(height * 0.65), width, height], fill=(160, 174, 162))
    draw.rectangle([40, 80, 260, int(height * 0.65)], fill=(120, 138, 156))
    # Draw a simple person-like silhouette for pipeline testing
    draw.ellipse([300, 150, 340, 190], fill=(45, 55, 72))
    draw.rectangle([295, 195, 345, 300], fill=(30, 64, 175))
    draw.rectangle([298, 300, 318, 380], fill=(30, 41, 59))
    draw.rectangle([322, 300, 342, 380], fill=(30, 41, 59))
    buf = BytesIO()
    img.save(buf, format="JPEG", quality=90)
    return buf.getvalue()
