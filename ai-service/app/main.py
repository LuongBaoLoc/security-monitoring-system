from fastapi import FastAPI, File, Header, HTTPException, UploadFile
from PIL import UnidentifiedImageError
import requests
from pathlib import Path

from app.config import MAX_IMAGE_BYTES, MODEL_PATH
from app.detector import detect
from app.event_handler import send_detection

app = FastAPI(title="Security Monitoring AI Service", version="1.0.0")


@app.get("/health")
def health():
    return {"status": "ok", "model_configured": bool(MODEL_PATH and Path(MODEL_PATH).is_file())}


@app.post("/detect")
async def detect_frame(camera_id: int, file: UploadFile = File(...), authorization: str | None = Header(default=None)):
    from app.config import AI_SERVICE_TOKEN
    if AI_SERVICE_TOKEN and authorization != f"Bearer {AI_SERVICE_TOKEN}":
        raise HTTPException(401, "Invalid AI service token")
    if not file.content_type or not file.content_type.startswith("image/"):
        raise HTTPException(415, "Upload must be an image")
    contents = await file.read(MAX_IMAGE_BYTES + 1)
    if len(contents) > MAX_IMAGE_BYTES:
        raise HTTPException(413, "Image exceeds configured size limit")
    if not MODEL_PATH:
        raise HTTPException(503, "No YOLO model configured. Set MODEL_PATH to a model file.")
    try:
        detections = detect(contents)
    except (UnidentifiedImageError, OSError):
        raise HTTPException(400, "Invalid image")
    forwarded = []
    for item in detections:
        try:
            forwarded.append(send_detection(camera_id, item))
        except requests.RequestException as exc:
            raise HTTPException(502, "Could not submit detection to backend") from exc
    return {"camera_id": camera_id, "count": len(detections), "detections": forwarded}
