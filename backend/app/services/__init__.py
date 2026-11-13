"""Nghiệp vụ dùng chung giữa các router và adapter."""
from datetime import timedelta
from urllib.parse import urlsplit, urlunsplit

from sqlalchemy import func, select
from sqlalchemy.orm import Session

from app.models import Alert, Camera, DetectionEvent, Device, DeviceLog, Sensor, SystemLog, User, now_utc


def redact_rtsp_url(rtsp_url: str) -> str:
    """Che thông tin xác thực trong RTSP URL trước khi trả về qua API."""
    parts = urlsplit(rtsp_url)
    host = parts.hostname or ""
    if ":" in host and not host.startswith("["):
        host = f"[{host}]"
    if parts.port:
        host = f"{host}:{parts.port}"
    return urlunsplit((parts.scheme, host, parts.path, "", "")) if parts.scheme else ""


def camera_json(camera: Camera) -> dict:
    return {
        "id": camera.id,
        "name": camera.name,
        "ip_address": camera.ip_address,
        "rtsp_url": redact_rtsp_url(camera.rtsp_url),
        "location": camera.location,
        "status": camera.status,
        "last_seen": camera.last_seen,
        "created_at": camera.created_at,
        "updated_at": camera.updated_at,
    }


def record_system_log(db: Session, action: str, description: str, user_id: int | None = None) -> SystemLog:
    entry = SystemLog(user_id=user_id, action=action, description=description, created_at=now_utc())
    db.add(entry)
    return entry


def seed_demo_data_if_empty(db: Session) -> None:
    """Khởi tạo dữ liệu mẫu ban đầu (Tuần 4) nếu bảng cameras và devices đang trống."""
    has_camera = db.scalar(select(func.count(Camera.id))) or 0
    has_device = db.scalar(select(func.count(Device.id))) or 0
    if has_camera > 0 or has_device > 0:
        return

    now = now_utc()
    admin = db.scalar(select(User).where(User.role == "ADMIN"))
    admin_id = admin.id if admin else None

    cameras = [
        Camera(
            name="CAM-01 · Cổng chính",
            ip_address="192.168.1.21",
            rtsp_url="rtsp://demo:secret@192.168.1.21:554/stream1",
            location="Cổng phía Đông",
            status="ONLINE",
            last_seen=now - timedelta(minutes=1),
            created_at=now - timedelta(days=7),
            updated_at=now - timedelta(minutes=1),
        ),
        Camera(
            name="CAM-02 · Sảnh lễ tân",
            ip_address="192.168.1.22",
            rtsp_url="rtsp://demo:secret@192.168.1.22:554/stream1",
            location="Tầng trệt",
            status="ONLINE",
            last_seen=now - timedelta(minutes=2),
            created_at=now - timedelta(days=7),
            updated_at=now - timedelta(minutes=2),
        ),
        Camera(
            name="CAM-03 · Hành lang A",
            ip_address="192.168.1.23",
            rtsp_url="rtsp://demo:secret@192.168.1.23:554/stream1",
            location="Tầng 1",
            status="ONLINE",
            last_seen=now - timedelta(minutes=3),
            created_at=now - timedelta(days=6),
            updated_at=now - timedelta(minutes=3),
        ),
        Camera(
            name="CAM-08 · Kho hàng",
            ip_address="192.168.1.28",
            rtsp_url="rtsp://demo:secret@192.168.1.28:554/stream1",
            location="Khu nhà kho",
            status="OFFLINE",
            last_seen=now - timedelta(hours=2),
            created_at=now - timedelta(days=5),
            updated_at=now - timedelta(hours=2),
        ),
    ]
    db.add_all(cameras)
    db.flush()

    devices = [
        Device(
            device_code="ESP32_001",
            device_name="ESP32 khu vực A · Tầng 1",
            device_type="ESP32",
            status="ONLINE",
            last_seen=now - timedelta(seconds=30),
            latest_pir=False,
            latest_mq2=350.0,
            relay=False,
            buzzer=False,
            lock_engaged=True,
            created_at=now - timedelta(days=7),
        ),
        Device(
            device_code="ESP32_002",
            device_name="ESP32 khu vực B · Kho hàng",
            device_type="ESP32",
            status="OFFLINE",
            last_seen=now - timedelta(hours=5),
            latest_pir=False,
            latest_mq2=280.0,
            relay=False,
            buzzer=False,
            lock_engaged=True,
            created_at=now - timedelta(days=6),
        ),
    ]
    db.add_all(devices)
    db.flush()

    sensors = [
        Sensor(device_id=devices[0].id, sensor_type="pir", sensor_value={"value": False}, created_at=now - timedelta(minutes=5)),
        Sensor(device_id=devices[0].id, sensor_type="mq2", sensor_value={"value": 350.0}, created_at=now - timedelta(minutes=5)),
        Sensor(device_id=devices[0].id, sensor_type="lock", sensor_value={"value": True}, created_at=now - timedelta(minutes=5)),
        Sensor(device_id=devices[1].id, sensor_type="mq2", sensor_value={"value": 280.0}, created_at=now - timedelta(hours=5)),
    ]
    db.add_all(sensors)

    detections = [
        DetectionEvent(
            camera_id=cameras[0].id,
            object_type="person",
            confidence=0.968,
            event_type="intrusion",
            image_path="uploads/events/event_001.jpg",
            event_timestamp=now - timedelta(minutes=15),
        ),
        DetectionEvent(
            camera_id=cameras[2].id,
            object_type="person",
            confidence=0.921,
            event_type="person",
            image_path="uploads/events/event_002.jpg",
            event_timestamp=now - timedelta(minutes=35),
        ),
        DetectionEvent(
            camera_id=cameras[0].id,
            object_type="car",
            confidence=0.887,
            event_type="object",
            image_path="uploads/events/event_003.jpg",
            event_timestamp=now - timedelta(hours=1, minutes=10),
        ),
        DetectionEvent(
            camera_id=cameras[1].id,
            object_type="person",
            confidence=0.854,
            event_type="person",
            image_path="uploads/events/event_004.jpg",
            event_timestamp=now - timedelta(days=1, hours=2),
        ),
        DetectionEvent(
            camera_id=cameras[0].id,
            object_type="truck",
            confidence=0.831,
            event_type="object",
            image_path="uploads/events/event_005.jpg",
            event_timestamp=now - timedelta(days=2, hours=3),
        ),
    ]
    db.add_all(detections)
    db.flush()

    alerts = [
        Alert(
            detection_id=detections[0].id,
            alert_type="intrusion",
            severity="CRITICAL",
            description="Phát hiện người xâm nhập tại CAM-01 · Cổng chính",
            status="OPEN",
            created_at=now - timedelta(minutes=15),
        ),
        Alert(
            detection_id=detections[1].id,
            alert_type="person",
            severity="WARNING",
            description="Chuyển động tại hành lang CAM-03 · Hành lang A",
            status="OPEN",
            created_at=now - timedelta(minutes=35),
        ),
        Alert(
            detection_id=None,
            alert_type="camera_offline",
            severity="WARNING",
            description="Camera mất kết nối tại CAM-08 · Kho hàng",
            status="OPEN",
            created_at=now - timedelta(hours=2),
        ),
        Alert(
            detection_id=detections[2].id,
            alert_type="vehicle",
            severity="INFO",
            description="Nhận diện phương tiện tại CAM-01 · Cổng chính",
            status="RESOLVED",
            handled_by=admin_id,
            created_at=now - timedelta(hours=1, minutes=10),
            handled_at=now - timedelta(minutes=50),
        ),
    ]
    db.add_all(alerts)

    db.add(
        DeviceLog(
            device_id=devices[0].id,
            command_id="00000000-0000-4000-8000-000000000001",
            command="buzzer_off",
            result="ACCEPTED",
            detail={"status": "accepted", "device_id": "ESP32_001"},
            created_at=now - timedelta(minutes=45),
            completed_at=now - timedelta(minutes=45) + timedelta(seconds=1),
        )
    )

    db.add_all(
        [
            SystemLog(
                user_id=None,
                action="AI_DETECTION",
                description="Phát hiện người tại CAM-01 · Cổng chính · Độ tin cậy 96.8%",
                created_at=now - timedelta(minutes=15),
            ),
            SystemLog(
                user_id=None,
                action="SENSOR_TELEMETRY",
                description="Cập nhật cảm biến MQ-2 (350 ppm) từ thiết bị ESP32_001",
                created_at=now - timedelta(minutes=30),
            ),
            SystemLog(
                user_id=admin_id,
                action="ALERT_RESOLVED",
                description="Đã xử lý cảnh báo nhận diện phương tiện tại CAM-01",
                created_at=now - timedelta(minutes=50),
            ),
        ]
    )
    db.commit()
