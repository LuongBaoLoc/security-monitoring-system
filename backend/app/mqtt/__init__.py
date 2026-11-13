"""MQTT telemetry and command acknowledgement adapter."""
import asyncio
import json
import logging
from datetime import datetime, timezone

import paho.mqtt.client as mqtt
from sqlalchemy import select

from app.core.config import settings
from app.database.connection import SessionLocal
from app.models import Device, DeviceLog, Sensor

logger = logging.getLogger(__name__)
client = None
event_loop = None


def _timestamp(value):
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
        return parsed if parsed.tzinfo else parsed.replace(tzinfo=timezone.utc)
    except (ValueError, AttributeError):
        return datetime.now(timezone.utc)


def _dispatch(event_type, data):
    if event_loop and event_loop.is_running():
        from app.routers import broadcast
        asyncio.run_coroutine_threadsafe(broadcast(event_type, data), event_loop)


def _on_connect(mqtt_client, _userdata, _flags, reason_code, _properties=None):
    if reason_code != 0:
        logger.error("MQTT connection rejected: %s", reason_code)
        return
    mqtt_client.subscribe("security/device/+/sensor")
    mqtt_client.subscribe("security/device/+/status")
    mqtt_client.subscribe("security/device/+/ack")


def _on_message(_client, _userdata, message):
    try:
        parts = message.topic.split("/")
        if len(parts) != 4 or parts[:2] != ["security", "device"]:
            return
        device_code, kind = parts[2], parts[3]
        payload = json.loads(message.payload.decode("utf-8"))
        if payload.get("device_id", device_code) != device_code:
            logger.warning("Ignoring MQTT payload with mismatched device id")
            return
        with SessionLocal() as db:
            device = db.scalar(select(Device).where(Device.device_code == device_code))
            if not device:
                logger.warning("Ignoring MQTT message from unregistered device %s", device_code)
                return
            now = _timestamp(payload.get("timestamp"))
            device.last_seen, device.status = now, "ONLINE"
            if kind == "status":
                if str(payload.get("status", "online")).lower() in {"offline", "online"}:
                    device.status = str(payload.get("status", "online")).upper()
            elif kind == "sensor":
                pir, mq2 = payload.get("pir"), payload.get("mq2")
                if pir is not None and not isinstance(pir, bool): return
                if mq2 is not None and (not isinstance(mq2, (int, float)) or mq2 < 0 or mq2 > 100000): return
                device.latest_pir, device.latest_mq2 = pir, mq2
                for key, column in (("relay", "relay"), ("buzzer", "buzzer"), ("lock", "lock_engaged")):
                    value = payload.get(key)
                    if value is not None and not isinstance(value, bool): return
                    if value is not None: setattr(device, column, value)
                for key in ("pir", "mq2", "relay", "buzzer", "lock"):
                    if key in payload:
                        db.add(Sensor(device_id=device.id, sensor_type=key, sensor_value={"value": payload[key]}, created_at=now))
            elif kind == "ack":
                command_id = payload.get("command_id")
                log = db.scalar(select(DeviceLog).where(DeviceLog.command_id == command_id, DeviceLog.device_id == device.id))
                if not log or log.result != "PENDING": return
                result = str(payload.get("status", "")).lower()
                log.result = "ACCEPTED" if result == "accepted" else "REJECTED"
                log.completed_at, log.detail = now, payload
            else:
                return
            db.commit()
            _dispatch("device.status", {"device_code": device_code, "status": device.status, "last_seen": now.isoformat()})
            if kind == "ack": _dispatch("device.command.updated", {"command_id": command_id, "status": log.result.lower()})
    except Exception:
        logger.exception("Rejected malformed MQTT message on %s", message.topic)


def start_mqtt(loop):
    global client, event_loop
    event_loop = loop
    if not settings.mqtt_enabled:
        return
    client = mqtt.Client(mqtt.CallbackAPIVersion.VERSION2, client_id="security-monitor-backend")
    if settings.mqtt_username:
        client.username_pw_set(settings.mqtt_username, settings.mqtt_password)
    client.on_connect, client.on_message = _on_connect, _on_message
    client.connect_async(settings.mqtt_host, settings.mqtt_port, keepalive=30)
    client.loop_start()


def stop_mqtt():
    global client
    if client:
        client.loop_stop()
        client.disconnect()
        client = None
