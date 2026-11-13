import requests

from app.config import AI_SERVICE_TOKEN, BACKEND_URL


def send_detection(camera_id: int, detection: dict) -> dict:
    headers = {"Authorization": f"Bearer {AI_SERVICE_TOKEN}"} if AI_SERVICE_TOKEN else {}
    response = requests.post(f"{BACKEND_URL.rstrip('/')}/api/detections", json={"camera_id": camera_id, **detection}, headers=headers, timeout=10)
    response.raise_for_status()
    return response.json()
