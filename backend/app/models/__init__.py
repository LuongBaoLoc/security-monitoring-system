from datetime import datetime, timezone

from sqlalchemy import BigInteger, Boolean, DateTime, Float, ForeignKey, Index, Integer, JSON, String, Text
from sqlalchemy.dialects.postgresql import JSONB
from sqlalchemy.orm import Mapped, mapped_column, relationship
from sqlalchemy.types import TypeDecorator

from app.database.connection import Base


BIG_ID = BigInteger().with_variant(Integer(), "sqlite")
JSON_VALUE = JSON(none_as_null=True).with_variant(JSONB(none_as_null=True), "postgresql")


class UTCDateTime(TypeDecorator):
    impl = DateTime(timezone=True)
    cache_ok = True

    def process_bind_param(self, value, _dialect):
        return self._utc(value)

    def process_result_value(self, value, _dialect):
        return self._utc(value)

    @staticmethod
    def _utc(value):
        if value is None:
            return None
        return value.astimezone(timezone.utc) if value.tzinfo else value.replace(tzinfo=timezone.utc)


def now_utc() -> datetime:
    return datetime.now(timezone.utc)


class User(Base):
    __tablename__ = "users"
    id: Mapped[int] = mapped_column(BIG_ID, primary_key=True)
    username: Mapped[str] = mapped_column(String(80), unique=True, index=True)
    password_hash: Mapped[str] = mapped_column(String(255))
    full_name: Mapped[str] = mapped_column(String(160))
    role: Mapped[str] = mapped_column(String(20), default="OPERATOR")
    is_active: Mapped[bool] = mapped_column(Boolean, default=True)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=now_utc)


class Camera(Base):
    __tablename__ = "cameras"
    id: Mapped[int] = mapped_column(BIG_ID, primary_key=True)
    name: Mapped[str] = mapped_column(String(120))
    ip_address: Mapped[str | None] = mapped_column(String(64), nullable=True)
    rtsp_url: Mapped[str] = mapped_column(Text)
    location: Mapped[str | None] = mapped_column(String(255), nullable=True)
    status: Mapped[str] = mapped_column(String(20), default="UNKNOWN", index=True)
    last_seen: Mapped[datetime | None] = mapped_column(UTCDateTime(), nullable=True)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=now_utc)
    updated_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=now_utc, onupdate=now_utc)


class Device(Base):
    __tablename__ = "devices"
    id: Mapped[int] = mapped_column(BIG_ID, primary_key=True)
    device_code: Mapped[str] = mapped_column(String(100), unique=True, index=True)
    device_name: Mapped[str] = mapped_column(String(120))
    device_type: Mapped[str] = mapped_column(String(60), default="ESP32")
    status: Mapped[str] = mapped_column(String(20), default="UNKNOWN", index=True)
    last_seen: Mapped[datetime | None] = mapped_column(UTCDateTime(), nullable=True)
    latest_pir: Mapped[bool | None] = mapped_column(Boolean, nullable=True)
    latest_mq2: Mapped[float | None] = mapped_column(Float, nullable=True)
    relay: Mapped[bool | None] = mapped_column(Boolean, nullable=True)
    buzzer: Mapped[bool | None] = mapped_column(Boolean, nullable=True)
    lock_engaged: Mapped[bool | None] = mapped_column(Boolean, nullable=True)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=now_utc)


class Sensor(Base):
    __tablename__ = "sensors"
    id: Mapped[int] = mapped_column(BIG_ID, primary_key=True)
    device_id: Mapped[int] = mapped_column(ForeignKey("devices.id"))
    sensor_type: Mapped[str] = mapped_column(String(60), index=True)
    sensor_value: Mapped[dict] = mapped_column(JSON_VALUE)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=now_utc, index=True)


class DetectionEvent(Base):
    __tablename__ = "detection_events"
    id: Mapped[int] = mapped_column(BIG_ID, primary_key=True)
    camera_id: Mapped[int] = mapped_column(ForeignKey("cameras.id"), index=True)
    object_type: Mapped[str] = mapped_column(String(80), index=True)
    confidence: Mapped[float] = mapped_column(Float)
    event_type: Mapped[str] = mapped_column(String(80))
    image_path: Mapped[str | None] = mapped_column(Text, nullable=True)
    event_timestamp: Mapped[datetime] = mapped_column(UTCDateTime(), default=now_utc, index=True)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=now_utc)
    camera: Mapped[Camera] = relationship()


class Alert(Base):
    __tablename__ = "alerts"
    id: Mapped[int] = mapped_column(BIG_ID, primary_key=True)
    detection_id: Mapped[int | None] = mapped_column(ForeignKey("detection_events.id"), nullable=True)
    alert_type: Mapped[str] = mapped_column(String(80))
    severity: Mapped[str] = mapped_column(String(20), default="WARNING", index=True)
    description: Mapped[str] = mapped_column(Text)
    status: Mapped[str] = mapped_column(String(20), default="OPEN", index=True)
    handled_by: Mapped[int | None] = mapped_column(ForeignKey("users.id"), nullable=True)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=now_utc, index=True)
    handled_at: Mapped[datetime | None] = mapped_column(UTCDateTime(), nullable=True)
    detection: Mapped[DetectionEvent | None] = relationship()


class DeviceLog(Base):
    __tablename__ = "device_logs"
    id: Mapped[int] = mapped_column(BIG_ID, primary_key=True)
    device_id: Mapped[int] = mapped_column(ForeignKey("devices.id"))
    command_id: Mapped[str] = mapped_column(String(36), unique=True)
    command: Mapped[str] = mapped_column(String(80))
    result: Mapped[str] = mapped_column(String(30), default="PENDING")
    detail: Mapped[dict | None] = mapped_column(JSON_VALUE, nullable=True)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=now_utc)
    completed_at: Mapped[datetime | None] = mapped_column(UTCDateTime(), nullable=True)


class SystemLog(Base):
    __tablename__ = "system_logs"
    id: Mapped[int] = mapped_column(BIG_ID, primary_key=True)
    user_id: Mapped[int | None] = mapped_column(ForeignKey("users.id"), nullable=True, index=True)
    action: Mapped[str] = mapped_column(String(100))
    description: Mapped[str] = mapped_column(Text)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=now_utc, index=True)
    user: Mapped[User | None] = relationship()


class RuntimeSetting(Base):
    __tablename__ = "runtime_settings"
    id: Mapped[int] = mapped_column(Integer, primary_key=True)
    values: Mapped[dict] = mapped_column(JSON_VALUE)


Index("ix_sensors_device_type_time", Sensor.device_id, Sensor.sensor_type, Sensor.created_at.desc())
Index("ix_detections_camera_time", DetectionEvent.camera_id, DetectionEvent.event_timestamp.desc())
Index("ix_alerts_detection", Alert.detection_id)
Index("ix_device_logs_result_time", DeviceLog.result, DeviceLog.created_at)
