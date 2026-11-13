import json
import os
import socket
import uuid
from datetime import timedelta

from fastapi import APIRouter, Depends, Header, HTTPException, Query
from sqlalchemy import delete, func, select, update
from sqlalchemy.orm import Session

from app.core.config import settings
from app.core.security import admin_user, create_token, current_user, hash_password, optional_user, verify_password
from app.database.connection import get_db
from app.models import Alert, Camera, DetectionEvent, Device, DeviceLog, Sensor, SystemLog, User, now_utc
from app.schemas import (
    CameraCreate,
    CameraUpdate,
    CommandCreate,
    DetectionCreate,
    DeviceCreate,
    Login,
    Register,
    SensorCreate,
    SettingsUpdate,
    UserCreate,
    UserUpdate,
)
from app.services import camera_json, record_system_log
from app.websocket import broadcast, connections, dashboard_socket

api = APIRouter(prefix="/api")
auth = APIRouter(prefix="/auth", tags=["authentication"])

RUNTIME_SETTINGS = {
    "mq2_threshold": 600.0,
    "alert_confidence_threshold": settings.alert_confidence_threshold,
    "alert_deduplication_seconds": settings.alert_deduplication_seconds,
    "after_hours_alert": True,
    "offline_device_alert": True,
    "critical_notification": True,
    "camera_offline_notification": True,
    "daily_digest": False,
    "retention_days": 30,
    "auto_save_snapshot": True,
}


def envelope(items, total, limit, offset):
    return {"items": items, "total": total, "limit": limit, "offset": offset}


@auth.post("/login")
def login(payload: Login, db: Session = Depends(get_db)):
    user = db.scalar(select(User).where(User.username == payload.username))
    if not user or not verify_password(payload.password, user.password_hash):
        raise HTTPException(status_code=401, detail="Sai tên đăng nhập hoặc mật khẩu")
    if not user.is_active:
        raise HTTPException(status_code=403, detail="Tài khoản đã bị khóa")
    record_system_log(db, "USER_LOGIN", f"Người dùng {user.username} ({user.role}) đăng nhập hệ thống", user.id)
    db.commit()
    return {"access_token": create_token(user), "token_type": "bearer", "expires_in": settings.access_token_expire_minutes * 60}


@auth.post("/register", status_code=201)
def register(payload: Register, db: Session = Depends(get_db)):
    if not settings.registration_enabled:
        raise HTTPException(status_code=403, detail="Registration is disabled")
    if db.scalar(select(User).where(User.username == payload.username)):
        raise HTTPException(status_code=409, detail="Username already exists")
    user = User(username=payload.username, password_hash=hash_password(payload.password), full_name=payload.full_name, role="OPERATOR")
    db.add(user)
    db.flush()
    record_system_log(db, "USER_REGISTER", f"Đăng ký tài khoản mới: {user.username}", user.id)
    db.commit()
    db.refresh(user)
    return {"id": user.id, "username": user.username, "full_name": user.full_name, "role": user.role}


@auth.get("/me")
def me(user: User = Depends(current_user)):
    return {"id": user.id, "username": user.username, "full_name": user.full_name, "role": user.role, "is_active": user.is_active}


@api.get("/users")
def list_users(db: Session = Depends(get_db), _=Depends(current_user)):
    rows = db.scalars(select(User).order_by(User.id)).all()
    return envelope(
        [{"id": u.id, "username": u.username, "full_name": u.full_name, "role": u.role, "is_active": u.is_active, "created_at": u.created_at} for u in rows],
        len(rows),
        len(rows),
        0,
    )


@api.post("/users", status_code=201)
def create_user(payload: UserCreate, db: Session = Depends(get_db), admin: User = Depends(admin_user)):
    if db.scalar(select(User).where(User.username == payload.username)):
        raise HTTPException(status_code=409, detail="Tên đăng nhập đã tồn tại")
    user = User(
        username=payload.username.strip(),
        password_hash=hash_password(payload.password),
        full_name=payload.full_name.strip(),
        role=payload.role,
        is_active=payload.is_active,
    )
    db.add(user)
    db.flush()
    record_system_log(db, "USER_CREATE", f"Admin tạo tài khoản {user.username} ({user.role})", admin.id)
    db.commit()
    db.refresh(user)
    return {"id": user.id, "username": user.username, "full_name": user.full_name, "role": user.role, "is_active": user.is_active, "created_at": user.created_at}


@api.put("/users/{user_id}")
def update_user(user_id: int, payload: UserUpdate, db: Session = Depends(get_db), admin: User = Depends(admin_user)):
    target = db.get(User, user_id)
    if not target:
        raise HTTPException(status_code=404, detail="Không tìm thấy người dùng")
    if target.id == admin.id:
        if payload.is_active is False:
            raise HTTPException(status_code=400, detail="Không thể tự khóa tài khoản của chính mình")
        if payload.role and payload.role != "ADMIN":
            raise HTTPException(status_code=400, detail="Không thể tự hạ quyền Quản trị viên của chính mình")
    if payload.full_name is not None:
        target.full_name = payload.full_name.strip()
    if payload.role is not None:
        target.role = payload.role
    if payload.is_active is not None:
        target.is_active = payload.is_active
    if payload.password:
        target.password_hash = hash_password(payload.password)
    record_system_log(db, "USER_UPDATE", f"Cập nhật tài khoản {target.username} (role={target.role}, active={target.is_active})", admin.id)
    db.commit()
    db.refresh(target)
    return {"id": target.id, "username": target.username, "full_name": target.full_name, "role": target.role, "is_active": target.is_active, "created_at": target.created_at}


@api.delete("/users/{user_id}", status_code=204)
def delete_user(user_id: int, db: Session = Depends(get_db), admin: User = Depends(admin_user)):
    target = db.get(User, user_id)
    if not target:
        raise HTTPException(status_code=404, detail="Không tìm thấy người dùng")
    if target.id == admin.id:
        raise HTTPException(status_code=400, detail="Không thể xóa tài khoản đang đăng nhập")
    db.execute(update(Alert).where(Alert.handled_by == user_id).values(handled_by=None))
    db.execute(update(SystemLog).where(SystemLog.user_id == user_id).values(user_id=None))
    username = target.username
    db.delete(target)
    record_system_log(db, "USER_DELETE", f"Đã xóa tài khoản {username}", admin.id)
    db.commit()


api.include_router(auth)


@api.get("/cameras")
def cameras(limit: int = Query(50, ge=1, le=200), offset: int = Query(0, ge=0), db: Session = Depends(get_db), _=Depends(current_user)):
    total = db.scalar(select(func.count(Camera.id))) or 0
    rows = db.scalars(select(Camera).order_by(Camera.id).offset(offset).limit(limit)).all()
    return envelope([camera_json(c) for c in rows], total, limit, offset)


@api.post("/cameras", status_code=201)
def create_camera(data: CameraCreate, db: Session = Depends(get_db), user: User = Depends(admin_user)):
    payload = data.model_dump()
    payload["status"] = (payload.get("status") or "ONLINE").upper()
    camera = Camera(**payload, last_seen=now_utc())
    db.add(camera)
    record_system_log(db, "CAMERA_CREATE", f"Thêm camera {camera.name} ({camera.location or 'Chưa rõ vị trí'})", user.id)
    db.commit()
    db.refresh(camera)
    return camera_json(camera)


@api.get("/cameras/{camera_id}")
def get_camera(camera_id: int, db: Session = Depends(get_db), _=Depends(current_user)):
    camera = db.get(Camera, camera_id)
    if not camera:
        raise HTTPException(404, "Camera not found")
    return camera_json(camera)


@api.put("/cameras/{camera_id}")
def update_camera(camera_id: int, data: CameraUpdate, db: Session = Depends(get_db), user: User = Depends(admin_user)):
    camera = db.get(Camera, camera_id)
    if not camera:
        raise HTTPException(404, "Camera not found")
    camera.name = data.name
    camera.ip_address = data.ip_address
    if data.rtsp_url and data.rtsp_url.strip():
        camera.rtsp_url = data.rtsp_url.strip()
    camera.location = data.location
    if data.status:
        camera.status = data.status.upper()
    camera.updated_at = now_utc()
    record_system_log(db, "CAMERA_UPDATE", f"Cập nhật camera {camera.name} ({camera.status})", user.id)
    db.commit()
    db.refresh(camera)
    return camera_json(camera)


@api.delete("/cameras/{camera_id}", status_code=204)
def delete_camera(camera_id: int, db: Session = Depends(get_db), user: User = Depends(admin_user)):
    camera = db.get(Camera, camera_id)
    if not camera:
        raise HTTPException(404, "Camera not found")
    det_ids = db.scalars(select(DetectionEvent.id).where(DetectionEvent.camera_id == camera_id)).all()
    if det_ids:
        db.execute(delete(Alert).where(Alert.detection_id.in_(det_ids)))
        db.execute(delete(DetectionEvent).where(DetectionEvent.camera_id == camera_id))
    record_system_log(db, "CAMERA_DELETE", f"Xóa camera {camera.name}", user.id)
    db.delete(camera)
    db.commit()


@api.get("/devices")
def devices(db: Session = Depends(get_db), _=Depends(current_user)):
    rows = db.scalars(select(Device).order_by(Device.device_code)).all()
    fields = ("id", "device_code", "device_name", "device_type", "status", "last_seen", "latest_pir", "latest_mq2", "relay", "buzzer", "lock_engaged")
    return envelope([{k: getattr(d, k) for k in fields} for d in rows], len(rows), len(rows), 0)


@api.post("/devices", status_code=201)
def create_device(data: DeviceCreate, db: Session = Depends(get_db), user: User = Depends(admin_user)):
    code = data.device_code.strip()
    if db.scalar(select(Device).where(Device.device_code == code)):
        raise HTTPException(status_code=409, detail="Mã thiết bị đã tồn tại")
    device = Device(
        device_code=code,
        device_name=data.device_name.strip(),
        device_type=data.device_type.strip() or "ESP32",
        status=(data.status or "ONLINE").upper(),
        last_seen=now_utc(),
        latest_pir=False,
        latest_mq2=350.0,
        relay=False,
        buzzer=False,
        lock_engaged=True,
    )
    db.add(device)
    record_system_log(db, "DEVICE_CREATE", f"Thêm thiết bị IoT {device.device_code} ({device.device_name})", user.id)
    db.commit()
    db.refresh(device)
    fields = ("id", "device_code", "device_name", "device_type", "status", "last_seen", "latest_pir", "latest_mq2", "relay", "buzzer", "lock_engaged")
    return {k: getattr(device, k) for k in fields}


@api.delete("/devices/{device_id}", status_code=204)
def delete_device(device_id: str, db: Session = Depends(get_db), user: User = Depends(admin_user)):
    device = db.scalar(select(Device).where(Device.device_code == device_id))
    if not device:
        raise HTTPException(404, "Device not found")
    db.execute(delete(Sensor).where(Sensor.device_id == device.id))
    db.execute(delete(DeviceLog).where(DeviceLog.device_id == device.id))
    code = device.device_code
    db.delete(device)
    record_system_log(db, "DEVICE_DELETE", f"Xóa thiết bị IoT {code}", user.id)
    db.commit()


@api.get("/devices/{device_id}")
def get_device(device_id: str, db: Session = Depends(get_db), _=Depends(current_user)):
    device = db.scalar(select(Device).where(Device.device_code == device_id))
    if not device:
        raise HTTPException(404, "Device not found")
    fields = ("id", "device_code", "device_name", "device_type", "status", "last_seen", "latest_pir", "latest_mq2", "relay", "buzzer", "lock_engaged")
    return {k: getattr(device, k) for k in fields}


@api.post("/devices/{device_id}/command", status_code=202)
async def command(device_id: str, body: CommandCreate, db: Session = Depends(get_db), user: User = Depends(current_user)):
    allowed = {"relay_on", "relay_off", "buzzer_on", "buzzer_off", "unlock", "lock"}
    if body.action not in allowed:
        raise HTTPException(422, "Unsupported command")
    device = db.scalar(select(Device).where(Device.device_code == device_id))
    if not device:
        raise HTTPException(404, "Device not found")
    command_id = str(uuid.uuid4())
    now = now_utc()
    # Update device state immediately for responsive UI and record telemetry
    if body.action == "relay_on":
        device.relay = True
    elif body.action == "relay_off":
        device.relay = False
    elif body.action == "buzzer_on":
        device.buzzer = True
    elif body.action == "buzzer_off":
        device.buzzer = False
    elif body.action == "lock":
        device.lock_engaged = True
    elif body.action == "unlock":
        device.lock_engaged = False
    device.last_seen = now
    device.status = "ONLINE"

    log = DeviceLog(device_id=device.id, command_id=command_id, command=body.action, result="PENDING", created_at=now)
    db.add(log)
    record_system_log(db, "DEVICE_COMMAND", f"Người dùng {user.username} gửi lệnh {body.action} tới {device_id}", user.id)

    mqtt_sent = False
    if settings.mqtt_enabled:
        try:
            import paho.mqtt.publish as publish
            publish.single(
                f"security/device/{device_id}/command",
                json.dumps({"command_id": command_id, "action": body.action, "requested_at": now.isoformat(), "requested_by": user.username}),
                hostname=settings.mqtt_host,
                port=settings.mqtt_port,
                auth={"username": settings.mqtt_username, "password": settings.mqtt_password} if settings.mqtt_username else None,
            )
            mqtt_sent = True
            log.result = "ACCEPTED"
            log.completed_at = now
        except Exception:
            log.result = "PENDING"
            log.detail = {"note": "State updated; MQTT broker unreachable"}
    else:
        log.result = "ACCEPTED"
        log.completed_at = now
        log.detail = {"note": "Local state updated"}

    db.commit()
    await broadcast("device.status", {"device_code": device.device_code, "status": device.status, "last_seen": now.isoformat()})
    return {
        "command_id": command_id,
        "status": "accepted" if mqtt_sent or not settings.mqtt_enabled else "pending",
        "device_code": device.device_code,
        "action": body.action,
        "relay": device.relay,
        "buzzer": device.buzzer,
        "lock_engaged": device.lock_engaged,
        "message": f"Đã thực thi lệnh {body.action} cho thiết bị {device.device_code}",
    }


@api.post("/sensors/data", status_code=201)
async def sensor_data(data: SensorCreate, db: Session = Depends(get_db)):
    device = db.scalar(select(Device).where(Device.device_code == data.device_id))
    if not device:
        raise HTTPException(404, "Device not registered")
    ts = data.timestamp or now_utc()
    device.status = "ONLINE"
    device.last_seen = ts
    if data.pir is not None:
        device.latest_pir = data.pir
    if data.mq2 is not None:
        device.latest_mq2 = data.mq2
    if data.relay is not None:
        device.relay = data.relay
    if data.buzzer is not None:
        device.buzzer = data.buzzer
    if data.lock is not None:
        device.lock_engaged = data.lock
    for kind, val in (("pir", data.pir), ("mq2", data.mq2), ("relay", data.relay), ("buzzer", data.buzzer), ("lock", data.lock)):
        if val is not None:
            db.add(Sensor(device_id=device.id, sensor_type=kind, sensor_value={"value": val}, created_at=ts))
    if data.mq2 is not None and data.mq2 >= float(RUNTIME_SETTINGS["mq2_threshold"]):
        alert = Alert(
            detection_id=None,
            alert_type="gas_leak",
            severity="CRITICAL",
            description=f"Cảm biến MQ-2 vượt ngưỡng ({data.mq2:.0f} ppm) tại {device.device_name}",
            status="OPEN",
            created_at=ts,
        )
        db.add(alert)
    record_system_log(db, "SENSOR_TELEMETRY", f"Cập nhật dữ liệu cảm biến từ {device.device_code} (PIR={device.latest_pir}, MQ-2={device.latest_mq2})")
    db.commit()
    await broadcast("device.status", {"device_code": device.device_code, "status": device.status, "last_seen": device.last_seen.isoformat()})
    return {"accepted": True, "device_id": device.device_code}


@api.get("/sensors")
def sensors(device_id: str | None = None, sensor_type: str | None = None, limit: int = Query(50, ge=1, le=500), offset: int = Query(0, ge=0), db: Session = Depends(get_db), _=Depends(current_user)):
    stmt = select(Sensor, Device.device_code).join(Device, Sensor.device_id == Device.id)
    count_stmt = select(func.count(Sensor.id)).join(Device, Sensor.device_id == Device.id)
    if device_id:
        stmt = stmt.where(Device.device_code == device_id)
        count_stmt = count_stmt.where(Device.device_code == device_id)
    if sensor_type:
        stmt = stmt.where(Sensor.sensor_type == sensor_type)
        count_stmt = count_stmt.where(Sensor.sensor_type == sensor_type)
    total = db.scalar(count_stmt) or 0
    rows = db.execute(stmt.order_by(Sensor.created_at.desc()).offset(offset).limit(limit)).all()
    return envelope([{"id": s.id, "device_id": code, "sensor_type": s.sensor_type, "sensor_value": s.sensor_value, "created_at": s.created_at} for s, code in rows], total, limit, offset)


@api.post("/detections", status_code=201)
async def create_detection(data: DetectionCreate, authorization: str | None = Header(default=None), db: Session = Depends(get_db), user: User | None = Depends(optional_user)):
    if (settings.ai_service_token and authorization != f"Bearer {settings.ai_service_token}" and user is None) or (not settings.ai_service_token and user is None):
        raise HTTPException(401, "Invalid AI service token")
    camera = db.get(Camera, data.camera_id)
    if not camera:
        raise HTTPException(404, "Camera not registered")
    timestamp = data.timestamp or now_utc()
    camera.last_seen = timestamp
    camera.status = "ONLINE"
    event = DetectionEvent(camera_id=camera.id, object_type=data.object_type, confidence=data.confidence, event_type=data.event_type, image_path=data.image_path, event_timestamp=timestamp)
    db.add(event)
    db.flush()
    alert = None
    deduplicated = False
    conf_threshold = float(RUNTIME_SETTINGS.get("alert_confidence_threshold", settings.alert_confidence_threshold))
    dedup_seconds = int(RUNTIME_SETTINGS.get("alert_deduplication_seconds", settings.alert_deduplication_seconds))
    if data.confidence >= conf_threshold and data.event_type.lower() in {"intrusion", "person", "weapon", "fire", "smoke"}:
        cutoff = timestamp - timedelta(seconds=dedup_seconds)
        duplicate = db.scalar(
            select(Alert)
            .join(DetectionEvent, Alert.detection_id == DetectionEvent.id)
            .where(Alert.status == "OPEN", Alert.alert_type == data.event_type, DetectionEvent.camera_id == camera.id, Alert.created_at >= cutoff)
        )
        if duplicate:
            deduplicated = True
        else:
            severity = "CRITICAL" if data.event_type.lower() in {"intrusion", "weapon", "fire"} else "WARNING"
            alert = Alert(detection_id=event.id, alert_type=data.event_type, severity=severity, description=f"Phát hiện {data.object_type} ({data.event_type}) tại {camera.name}")
            db.add(alert)
    record_system_log(db, "AI_DETECTION", f"Phát hiện {data.object_type} ({data.confidence * 100:.1f}%) tại {camera.name}", user.id if user else None)
    db.commit()
    db.refresh(event)
    result = {
        "id": event.id,
        "camera_id": event.camera_id,
        "object_type": event.object_type,
        "confidence": event.confidence,
        "event_type": event.event_type,
        "timestamp": event.event_timestamp,
        "alert_id": alert.id if alert else None,
        "alert_severity": alert.severity if alert else None,
        "deduplicated": deduplicated,
    }
    await broadcast("detection.created", result)
    if alert:
        db.refresh(alert)
        await broadcast("alert.created", {"id": alert.id, "severity": alert.severity, "status": alert.status})
    return result


@api.get("/detections")
def detections(limit: int = Query(50, ge=1, le=200), offset: int = Query(0, ge=0), db: Session = Depends(get_db), _=Depends(current_user)):
    total = db.scalar(select(func.count(DetectionEvent.id))) or 0
    rows = db.scalars(select(DetectionEvent).order_by(DetectionEvent.event_timestamp.desc()).offset(offset).limit(limit)).all()
    return envelope(
        [{"id": d.id, "camera_id": d.camera_id, "camera_name": d.camera.name, "object_type": d.object_type, "confidence": d.confidence, "event_type": d.event_type, "image_path": d.image_path, "timestamp": d.event_timestamp} for d in rows],
        total,
        limit,
        offset,
    )


@api.get("/detections/{event_id}")
def detection_detail(event_id: int, db: Session = Depends(get_db), _=Depends(current_user)):
    d = db.get(DetectionEvent, event_id)
    if not d:
        raise HTTPException(404, "Detection not found")
    return {"id": d.id, "camera_id": d.camera_id, "object_type": d.object_type, "confidence": d.confidence, "event_type": d.event_type, "image_path": d.image_path, "timestamp": d.event_timestamp}


@api.get("/alerts")
def alerts(alert_status: str | None = Query(default=None, alias="status"), severity: str | None = None, limit: int = Query(50, ge=1, le=200), offset: int = Query(0, ge=0), db: Session = Depends(get_db), _=Depends(current_user)):
    stmt = select(Alert)
    count_stmt = select(func.count(Alert.id))
    if alert_status:
        stmt = stmt.where(Alert.status == alert_status.upper())
        count_stmt = count_stmt.where(Alert.status == alert_status.upper())
    if severity:
        stmt = stmt.where(Alert.severity == severity.upper())
        count_stmt = count_stmt.where(Alert.severity == severity.upper())
    total = db.scalar(count_stmt) or 0
    rows = db.scalars(stmt.order_by(Alert.created_at.desc()).offset(offset).limit(limit)).all()
    users_map = {u.id: u.full_name for u in db.scalars(select(User)).all()}
    return envelope(
        [
            {
                "id": a.id,
                "detection_id": a.detection_id,
                "alert_type": a.alert_type,
                "severity": a.severity,
                "description": a.description,
                "status": a.status,
                "handled_by": a.handled_by,
                "handled_by_name": users_map.get(a.handled_by) if a.handled_by else None,
                "created_at": a.created_at,
                "handled_at": a.handled_at,
            }
            for a in rows
        ],
        total,
        limit,
        offset,
    )


@api.get("/alerts/{alert_id}")
def alert_detail(alert_id: int, db: Session = Depends(get_db), _=Depends(current_user)):
    a = db.get(Alert, alert_id)
    if not a:
        raise HTTPException(404, "Alert not found")
    return {"id": a.id, "detection_id": a.detection_id, "alert_type": a.alert_type, "severity": a.severity, "description": a.description, "status": a.status, "created_at": a.created_at, "handled_at": a.handled_at}


@api.put("/alerts/{alert_id}/resolve")
async def resolve_alert(alert_id: int, db: Session = Depends(get_db), user: User = Depends(current_user)):
    a = db.get(Alert, alert_id)
    if not a:
        raise HTTPException(404, "Alert not found")
    if a.status != "OPEN":
        raise HTTPException(400, "Alert is already resolved")
    a.status, a.handled_by, a.handled_at = "RESOLVED", user.id, now_utc()
    record_system_log(db, "ALERT_RESOLVED", f"{user.full_name} đã xử lý cảnh báo #ALT-{a.id:04d}", user.id)
    db.commit()
    await broadcast("alert.resolved", {"id": a.id, "status": a.status})
    return {"id": a.id, "status": a.status, "handled_by": user.id, "handled_by_name": user.full_name, "handled_at": a.handled_at}


@api.get("/logs/system")
def system_logs(limit: int = Query(50, ge=1, le=200), offset: int = Query(0, ge=0), db: Session = Depends(get_db), _=Depends(current_user)):
    total = db.scalar(select(func.count(SystemLog.id))) or 0
    rows = db.scalars(select(SystemLog).order_by(SystemLog.created_at.desc()).offset(offset).limit(limit)).all()
    return envelope(
        [{"id": log.id, "user_id": log.user_id, "action": log.action, "description": log.description, "created_at": log.created_at} for log in rows],
        total,
        limit,
        offset,
    )


@api.delete("/logs/system", status_code=204)
def clear_system_logs(db: Session = Depends(get_db), _=Depends(admin_user)):
    db.execute(delete(SystemLog))
    db.execute(delete(DeviceLog))
    db.commit()


@api.get("/logs/devices")
def device_logs(limit: int = Query(50, ge=1, le=200), offset: int = Query(0, ge=0), db: Session = Depends(get_db), _=Depends(current_user)):
    total = db.scalar(select(func.count(DeviceLog.id))) or 0
    rows = db.scalars(select(DeviceLog).order_by(DeviceLog.created_at.desc()).offset(offset).limit(limit)).all()
    return envelope(
        [{"id": log.id, "device_id": log.device_id, "command_id": log.command_id, "command": log.command, "result": log.result, "detail": log.detail, "created_at": log.created_at, "completed_at": log.completed_at} for log in rows],
        total,
        limit,
        offset,
    )


@api.get("/statistics/overview")
def overview(db: Session = Depends(get_db), _=Depends(current_user)):
    today = now_utc().replace(hour=0, minute=0, second=0, microsecond=0)
    return {
        "camera_total": db.scalar(select(func.count(Camera.id))) or 0,
        "camera_online": db.scalar(select(func.count(Camera.id)).where(Camera.status == "ONLINE")) or 0,
        "device_total": db.scalar(select(func.count(Device.id))) or 0,
        "device_online": db.scalar(select(func.count(Device.id)).where(Device.status == "ONLINE")) or 0,
        "detections_today": db.scalar(select(func.count(DetectionEvent.id)).where(DetectionEvent.event_timestamp >= today)) or 0,
        "alerts_today": db.scalar(select(func.count(Alert.id)).where(Alert.created_at >= today)) or 0,
        "alerts_open": db.scalar(select(func.count(Alert.id)).where(Alert.status == "OPEN")) or 0,
        "critical_open": db.scalar(select(func.count(Alert.id)).where(Alert.status == "OPEN", Alert.severity == "CRITICAL")) or 0,
    }


@api.get("/statistics/alerts")
def alert_stats(days: int = Query(7, ge=1, le=90), db: Session = Depends(get_db), _=Depends(current_user)):
    start = now_utc() - timedelta(days=days)
    rows = db.execute(select(func.date(Alert.created_at), func.count(Alert.id)).where(Alert.created_at >= start).group_by(func.date(Alert.created_at)).order_by(func.date(Alert.created_at))).all()
    return {"items": [{"date": str(day), "count": count} for day, count in rows]}


@api.get("/statistics/detections")
def detection_stats(days: int = Query(7, ge=1, le=90), db: Session = Depends(get_db), _=Depends(current_user)):
    start = now_utc() - timedelta(days=days)
    rows = db.execute(select(DetectionEvent.object_type, func.count(DetectionEvent.id)).where(DetectionEvent.event_timestamp >= start).group_by(DetectionEvent.object_type).order_by(func.count(DetectionEvent.id).desc())).all()
    return {"items": [{"object_type": kind, "count": count} for kind, count in rows]}


@api.get("/system/settings")
def get_system_settings(_=Depends(current_user)):
    return RUNTIME_SETTINGS


@api.put("/system/settings")
def update_system_settings(payload: SettingsUpdate, db: Session = Depends(get_db), admin: User = Depends(admin_user)):
    RUNTIME_SETTINGS.update(payload.model_dump())
    record_system_log(
        db,
        "SETTINGS_UPDATE",
        f"Admin cập nhật cài đặt hệ thống (MQ2={payload.mq2_threshold}ppm, AI={payload.alert_confidence_threshold * 100:.0f}%, Dedup={payload.alert_deduplication_seconds}s)",
        admin.id,
    )
    db.commit()
    return RUNTIME_SETTINGS


@api.get("/system/health")
def health(db: Session = Depends(get_db)):
    try:
        db.execute(select(1))
        db_status = "ok"
    except Exception:
        db_status = "unavailable"

    # 1. Probe MQTT Broker TCP port
    mqtt_broker = "offline"
    if settings.mqtt_enabled:
        try:
            with socket.create_connection((settings.mqtt_host, settings.mqtt_port), timeout=0.35):
                mqtt_broker = "online"
        except Exception:
            mqtt_broker = "offline"

    # 2. Check active Cameras & ESP32 Devices in DB
    cam_total = 0
    cam_online = 0
    dev_total = 0
    dev_online = 0
    recent_ai = 0
    if db_status == "ok":
        cam_total = db.scalar(select(func.count(Camera.id))) or 0
        cam_online = db.scalar(select(func.count(Camera.id)).where(Camera.status == "ONLINE")) or 0
        dev_total = db.scalar(select(func.count(Device.id))) or 0
        dev_online = db.scalar(select(func.count(Device.id)).where(Device.status == "ONLINE")) or 0
        recent_cutoff = now_utc() - timedelta(minutes=10)
        recent_ai = db.scalar(select(func.count(DetectionEvent.id)).where(DetectionEvent.event_timestamp >= recent_cutoff)) or 0

    # 3. Probe AI Worker Service (must have YOLO model_configured + active camera, or recent AI detections)
    ai_worker = "online" if recent_ai > 0 else "offline"
    if ai_worker == "offline" and cam_online > 0:
        import urllib.request
        for host in ("ai-service", "127.0.0.1"):
            try:
                with urllib.request.urlopen(f"http://{host}:8001/health", timeout=0.35) as resp:
                    body = json.loads(resp.read().decode("utf-8"))
                    if body.get("model_configured") is True:
                        ai_worker = "online"
                        break
            except Exception:
                pass

    # 4. Probe WireGuard VPN interface (wg0) or env flag
    vpn_status = "online" if (os.path.exists("/sys/class/net/wg0") or os.getenv("WIREGUARD_ACTIVE", "").lower() == "true") else "offline"

    return {
        "api": "ok",
        "database": db_status,
        "mqtt": mqtt_broker,
        "mqtt_broker": mqtt_broker,
        "esp32": "online" if dev_online > 0 else "offline",
        "device_online": dev_online,
        "device_total": dev_total,
        "camera_rtsp": "online" if cam_online > 0 else "offline",
        "camera_online": cam_online,
        "camera_total": cam_total,
        "ai": ai_worker,
        "ai_worker": ai_worker,
        "vpn": vpn_status,
        "wireguard_vpn": vpn_status,
    }
