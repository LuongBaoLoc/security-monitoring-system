
import os
from pathlib import Path

os.environ["OPENCV_FFMPEG_CAPTURE_OPTIONS"] = "rtsp_transport;tcp"

import cv2
from dotenv import load_dotenv
from fastapi import FastAPI, HTTPException
from fastapi.responses import StreamingResponse
from fastapi.middleware.cors import CORSMiddleware

ROOT = Path(__file__).resolve().parents[2]
load_dotenv(ROOT / ".env")

app = FastAPI(title="SmartSecure Camera API")

app.add_middleware(
    CORSMiddleware,
    allow_origins=[
        "http://localhost:5500",
        "http://127.0.0.1:5500"
    ],
    allow_methods=["GET"],
    allow_headers=["*"]
)


@app.get("/health")
def health():
    return {"status": "ok", "service": "camera"}


@app.get("/api/camera/status")
def camera_status():
    return {
        "camera_id": "CAM-01",
        "configured": bool(os.getenv("CAMERA_RTSP_URL"))
    }


def generate_frames():
    url = os.getenv("CAMERA_RTSP_URL", "").strip()
    cap = cv2.VideoCapture(url, cv2.CAP_FFMPEG)

    try:
        if not cap.isOpened():
            print("Không thể kết nối camera RTSP")
            return

        while True:
            success, frame = cap.read()

            if not success:
                print("Mất tín hiệu camera")
                break

            success, encoded = cv2.imencode(
                ".jpg", frame,
                [cv2.IMWRITE_JPEG_QUALITY, 75]
            )

            if success:
                yield (
                    b"--frame\r\n"
                    b"Content-Type: image/jpeg\r\n\r\n"
                    + encoded.tobytes()
                    + b"\r\n"
                )

    finally:
        cap.release()


@app.get("/api/camera/stream")
def camera_stream():
    if not os.getenv("CAMERA_RTSP_URL"):
        raise HTTPException(
            status_code=503,
            detail="Chưa cấu hình camera RTSP"
        )

    return StreamingResponse(
        generate_frames(),
        media_type="multipart/x-mixed-replace; boundary=frame",
        headers={"Cache-Control": "no-store"}
    )
