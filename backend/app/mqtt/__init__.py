"""Bridge telemetry and acknowledgements between Mosquitto and PostgreSQL."""
import asyncio
import json
import logging
from datetime import timedelta
from typing import Literal

import paho.mqtt.client as mqtt
from pydantic import AwareDatetime, BaseModel, ValidationError
from sqlalchemy import select, update

from app.core.config import settings
from app.database.connection import SessionLocal
from app.models import Device, DeviceLog, now_utc
from app.schemas import SensorCreate
from app.services import ACTUATORS, device_status_event, store_telemetry
from app.websocket import broadcast

logger = logging.getLogger(__name__)
client: mqtt.Client | None = None
event_loop: asyncio.AbstractEventLoop | None = None


class DeviceStatusMessage(BaseModel):
    status: Literal["online", "offline"]
    timestamp: AwareDatetime | None = None


class DeviceAck(BaseModel):
    command_id: str
    action: str
    status: Literal["accepted", "rejected"]
    timestamp: AwareDatetime | None = None


def _dispatch(event_type, data):
    loop = event_loop
    if loop and loop.is_running():
        asyncio.run_coroutine_threadsafe(broadcast(event_type, data), loop)


def _on_connect(mqtt_client, _userdata, _flags, reason_code, _properties):
    if reason_code.is_failure:
        logger.warning("MQTT connection rejected: %s", reason_code)
        return
    mqtt_client.subscribe([(f"security/device/+/{kind}", 1) for kind in ("sensor", "status", "ack")])


def _on_message(_client, _userdata, message):
    parts = message.topic.split("/")
    if len(parts) != 4 or parts[:2] != ["security", "device"] or len(message.payload) > 8192:
        return
    device_code, kind = parts[2], parts[3]
    try:
        payload = json.loads(message.payload)
        if not isinstance(payload, dict) or payload.get("device_id", device_code) != device_code:
            return
        if kind == "sensor":
            data = SensorCreate.model_validate({**payload, "device_id": device_code})
        elif kind == "status":
            data = DeviceStatusMessage.model_validate(payload)
        elif kind == "ack" and not message.retain:
            data = DeviceAck.model_validate(payload)
        else:
            return
    except (ValueError, ValidationError):
        logger.warning("Ignoring invalid MQTT message on %s", message.topic)
        return

    try:
        with SessionLocal.begin() as db:
            device = db.scalar(select(Device).where(Device.device_code == device_code).with_for_update())
            if device is None:
                return
            alert = None
            command_event = None
            if isinstance(data, SensorCreate):
                alert = store_telemetry(db, device, data)
                db.flush()
            elif isinstance(data, DeviceStatusMessage):
                timestamp = data.timestamp or now_utc()
                if device.last_seen and timestamp < device.last_seen:
                    return
                device.last_seen, device.status = timestamp, data.status.upper()
            else:
                received = now_utc()
                result = db.execute(
                    update(DeviceLog)
                    .where(
                        DeviceLog.command_id == data.command_id,
                        DeviceLog.device_id == device.id,
                        DeviceLog.command == data.action,
                        DeviceLog.result == "PENDING",
                        DeviceLog.created_at >= received - timedelta(seconds=settings.mqtt_command_timeout_seconds),
                    )
                    .values(result=data.status.upper(), completed_at=received, detail=data.model_dump(mode="json"))
                )
                if not result.rowcount:
                    return
                if data.status == "accepted":
                    column, value = ACTUATORS[data.action]
                    setattr(device, column, value)
                device.last_seen, device.status = received, "ONLINE"
                command_event = {"command_id": data.command_id, "status": data.status}

        _dispatch("device.status", device_status_event(device))
        if alert:
            _dispatch("alert.created", {"id": alert.id, "severity": alert.severity, "status": alert.status})
        if command_event:
            _dispatch("device.command.updated", command_event)
    except Exception:
        logger.exception("Unable to persist MQTT message from %s", device_code)


def is_connected() -> bool:
    return client is not None and client.is_connected()


def publish_command(device_code: str, payload: dict) -> bool:
    mqtt_client = client
    if mqtt_client is None or not mqtt_client.is_connected():
        return False
    # A disconnected client must not replay actuator commands after reconnecting.
    info = mqtt_client.publish(f"security/device/{device_code}/command", json.dumps(payload), qos=0, retain=False)
    return info.rc == mqtt.MQTT_ERR_SUCCESS


def expire_commands() -> list[str]:
    now = now_utc()
    with SessionLocal.begin() as db:
        return list(db.scalars(
            update(DeviceLog)
            .where(
                DeviceLog.result == "PENDING",
                DeviceLog.created_at < now - timedelta(seconds=settings.mqtt_command_timeout_seconds),
            )
            .values(result="TIMEOUT", completed_at=now)
            .returning(DeviceLog.command_id)
        ))


def start_mqtt(loop):
    global client, event_loop
    event_loop = loop
    if not settings.mqtt_enabled:
        return
    client = mqtt.Client(mqtt.CallbackAPIVersion.VERSION2)
    if settings.mqtt_username:
        client.username_pw_set(settings.mqtt_username, settings.mqtt_password)
    client.on_connect, client.on_message = _on_connect, _on_message
    client.connect_async(settings.mqtt_host, settings.mqtt_port, keepalive=30)
    client.loop_start()


def stop_mqtt():
    global client, event_loop
    if client:
        client.disconnect()
        client.loop_stop()
        client = None
    event_loop = None
