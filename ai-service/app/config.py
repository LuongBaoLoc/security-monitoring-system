import os

BACKEND_URL = os.getenv("BACKEND_URL", "http://localhost:8000")
AI_SERVICE_TOKEN = os.getenv("AI_SERVICE_TOKEN", "")
MODEL_PATH = os.getenv("MODEL_PATH", "")
CONFIDENCE_THRESHOLD = float(os.getenv("CONFIDENCE_THRESHOLD", "0.6"))
MAX_IMAGE_BYTES = int(os.getenv("MAX_IMAGE_BYTES", str(8 * 1024 * 1024)))
