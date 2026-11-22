from typing import Annotated, Literal

from pydantic import AfterValidator, AwareDatetime, BaseModel, Field, field_validator


def validate_password_length(value: str) -> str:
    if len(value.encode("utf-8")) > 72:
        raise ValueError("Password must be at most 72 UTF-8 bytes")
    return value


Password = Annotated[str, AfterValidator(validate_password_length)]
DeviceCode = Annotated[str, Field(min_length=1, max_length=100, pattern=r"^[^/+#\x00]+$")]
DeviceStatus = Literal["ONLINE", "OFFLINE", "UNKNOWN"]


class Login(BaseModel):
    username: str = Field(min_length=1, max_length=80)
    password: Password = Field(min_length=1, max_length=72)


class Register(Login):
    password: Password = Field(min_length=8, max_length=72)
    full_name: str = Field(min_length=1, max_length=160)


class UserCreate(BaseModel):
    username: str = Field(min_length=1, max_length=80)
    password: Password = Field(min_length=8, max_length=72)
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
    password: Password | None = Field(default=None, min_length=8, max_length=72)

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
    ip_address: str | None = Field(default=None, max_length=64)
    rtsp_url: str = Field(min_length=1)
    location: str | None = Field(default=None, max_length=255)
    status: DeviceStatus = "ONLINE"


class CameraUpdate(BaseModel):
    name: str = Field(min_length=1, max_length=120)
    ip_address: str | None = Field(default=None, max_length=64)
    rtsp_url: str | None = None
    location: str | None = Field(default=None, max_length=255)
    status: DeviceStatus = "ONLINE"


class DeviceCreate(BaseModel):
    device_code: DeviceCode
    device_name: str = Field(min_length=1, max_length=120)
    device_type: str = Field(default="ESP32", min_length=1, max_length=60)
    status: DeviceStatus = "ONLINE"

    @field_validator("device_code", mode="before")
    @classmethod
    def trim_code(cls, value):
        return value.strip() if isinstance(value, str) else value


class DetectionCreate(BaseModel):
    camera_id: int
    object_type: str = Field(min_length=1, max_length=80)
    confidence: float = Field(ge=0, le=1)
    event_type: str = Field(min_length=1, max_length=80)
    image_path: str | None = None
    timestamp: AwareDatetime | None = None

    @field_validator("image_path")
    @classmethod
    def relative_image_path(cls, value):
        normalized = value.replace("\\", "/") if value else value
        if normalized and (normalized.startswith("/") or ".." in normalized.split("/") or ":" in normalized):
            raise ValueError("image_path must be a relative path")
        return value


class SensorCreate(BaseModel):
    device_id: DeviceCode
    pir: bool | None = Field(default=None, strict=True)
    mq2: float | None = Field(default=None, strict=True, ge=0, le=100000)
    relay: bool | None = Field(default=None, strict=True)
    buzzer: bool | None = Field(default=None, strict=True)
    lock: bool | None = Field(default=None, strict=True)
    timestamp: AwareDatetime | None = None


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
    retention_days: int = Field(default=30, ge=1)
    auto_save_snapshot: bool = True
