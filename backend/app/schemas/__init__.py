from datetime import datetime
from pydantic import BaseModel, ConfigDict, Field, field_validator


class Login(BaseModel):
    username: str = Field(min_length=1, max_length=80)
    password: str = Field(min_length=1, max_length=72)


class Register(Login):
    password: str = Field(min_length=8, max_length=72)
    full_name: str = Field(min_length=1, max_length=160)


class UserCreate(BaseModel):
    username: str = Field(min_length=1, max_length=80)
    password: str = Field(min_length=8, max_length=72)
    full_name: str = Field(min_length=1, max_length=160)
    role: str = Field(default="OPERATOR")
    is_active: bool = True

    @field_validator("role")
    @classmethod
    def valid_role(cls, value: str) -> str:
        role = value.strip().upper()
        if role not in {"ADMIN", "OPERATOR"}:
            raise ValueError("role must be ADMIN or OPERATOR")
        return role


class UserUpdate(BaseModel):
    full_name: str | None = Field(default=None, min_length=1, max_length=160)
    role: str | None = None
    is_active: bool | None = None
    password: str | None = Field(default=None, min_length=8, max_length=72)

    @field_validator("role")
    @classmethod
    def valid_role(cls, value: str | None) -> str | None:
        if value is None:
            return None
        role = value.strip().upper()
        if role not in {"ADMIN", "OPERATOR"}:
            raise ValueError("role must be ADMIN or OPERATOR")
        return role


class CameraCreate(BaseModel):
    name: str = Field(min_length=1, max_length=120)
    ip_address: str | None = None
    rtsp_url: str = Field(min_length=1)
    location: str | None = None
    status: str = "ONLINE"


class CameraUpdate(BaseModel):
    name: str = Field(min_length=1, max_length=120)
    ip_address: str | None = None
    rtsp_url: str | None = None
    location: str | None = None
    status: str = "ONLINE"


class DeviceCreate(BaseModel):
    device_code: str = Field(min_length=1, max_length=100)
    device_name: str = Field(min_length=1, max_length=120)
    device_type: str = Field(default="ESP32", min_length=1, max_length=60)
    status: str = "ONLINE"


class DetectionCreate(BaseModel):
    camera_id: int
    object_type: str = Field(min_length=1, max_length=80)
    confidence: float = Field(ge=0, le=1)
    event_type: str = Field(min_length=1, max_length=80)
    image_path: str | None = None
    timestamp: datetime | None = None

    @field_validator("image_path")
    @classmethod
    def relative_image_path(cls, value):
        normalized = value.replace("\\", "/") if value else value
        if normalized and (normalized.startswith("/") or ".." in normalized.split("/") or ":" in normalized):
            raise ValueError("image_path must be a relative path")
        return value


class SensorCreate(BaseModel):
    device_id: str
    pir: bool | None = None
    mq2: float | None = Field(default=None, ge=0, le=100000)
    relay: bool | None = None
    buzzer: bool | None = None
    lock: bool | None = None
    timestamp: datetime | None = None


class CommandCreate(BaseModel):
    action: str


class SettingsUpdate(BaseModel):
    mq2_threshold: float = Field(default=600.0, ge=50, le=10000)
    alert_confidence_threshold: float = Field(default=0.8, ge=0.1, le=1.0)
    alert_deduplication_seconds: int = Field(default=60, ge=0, le=3600)
    after_hours_alert: bool = True
    offline_device_alert: bool = True
    critical_notification: bool = True
    camera_offline_notification: bool = True
    daily_digest: bool = False
    retention_days: int = 30
    auto_save_snapshot: bool = True


class ORMModel(BaseModel):
    model_config = ConfigDict(from_attributes=True)
