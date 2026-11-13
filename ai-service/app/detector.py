from functools import lru_cache
from io import BytesIO
from pathlib import Path

from PIL import Image

from app.config import CONFIDENCE_THRESHOLD, MODEL_PATH


@lru_cache(maxsize=1)
def get_model():
    if not MODEL_PATH or not Path(MODEL_PATH).is_file():
        return None
    from ultralytics import YOLO
    return YOLO(MODEL_PATH)


def detect(image_bytes: bytes) -> list[dict]:
    model = get_model()
    if model is None:
        return []
    image = Image.open(BytesIO(image_bytes)).convert("RGB")
    result = model.predict(image, conf=CONFIDENCE_THRESHOLD, verbose=False)[0]
    names = result.names
    detections = []
    for box in result.boxes:
        cls_id = int(box.cls[0].item())
        label = str(names[cls_id]).lower()
        confidence = float(box.conf[0].item())
        detections.append({"object_type": label, "confidence": confidence, "event_type": label if label in {"person", "fire", "smoke", "weapon"} else "object"})
    return detections
