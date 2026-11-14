
import os

os.environ["OPENCV_FFMPEG_CAPTURE_OPTIONS"] = "rtsp_transport;tcp"

import cv2
from fastapi import APIRouter, Depends, HTTPException
from fastapi.responses import StreamingResponse, Response
from sqlalchemy.orm import Session

from app.core.security import current_user
from app.database.connection import get_db
from app.models import Camera, User

camera_router = APIRouter(tags=["Camera Streaming"])


def generate_frames(rtsp_url: str):
    cap = cv2.VideoCapture(rtsp_url, cv2.CAP_FFMPEG)

    try:
        if not cap.isOpened():
            return

        while True:
            success, frame = cap.read()

            if not success:
                break

            success, encoded = cv2.imencode(
                ".jpg",
                frame,
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


@camera_router.get("/api/camera/{camera_id}/stream")
def camera_stream(
    camera_id: int,
    db: Session = Depends(get_db),
    user: User = Depends(current_user)
):
    camera = db.get(Camera, camera_id)

    if not camera:
        raise HTTPException(404, "Không tìm thấy camera")

    if not camera.rtsp_url:
        raise HTTPException(503, "Camera chưa có RTSP URL")

    return StreamingResponse(
        generate_frames(camera.rtsp_url),
        media_type="multipart/x-mixed-replace; boundary=frame",
        headers={"Cache-Control": "no-store"}
    )

@camera_router.get("/api/camera/{camera_id}/snapshot")
def camera_snapshot(
    camera_id: int,
    db: Session = Depends(get_db),
    user: User = Depends(current_user)
):
    camera = db.get(Camera, camera_id)

    if not camera:
        raise HTTPException(404, "Không tìm thấy camera")

    if not camera.rtsp_url:
        raise HTTPException(503, "Camera chưa cấu hình RTSP")

    cap = cv2.VideoCapture(camera.rtsp_url, cv2.CAP_FFMPEG)

    try:
        if not cap.isOpened():
            raise HTTPException(503, "Không kết nối được camera")

        ok, frame = cap.read()

        if not ok:
            raise HTTPException(503, "Không đọc được hình ảnh")

        ok, encoded = cv2.imencode(".jpg", frame)

        if not ok:
            raise HTTPException(500, "Không mã hóa được ảnh")

        return Response(
            encoded.tobytes(),
            media_type="image/jpeg",
            headers={"Cache-Control": "no-store"}
        )
    finally:
        cap.release()
