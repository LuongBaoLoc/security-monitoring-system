# Database design — PostgreSQL

## ER overview

```text
users 1---* alerts (handled_by)
users 1---* system_logs
cameras 1---* detection_events 1---* alerts
devices 1---* sensors
devices 1---* device_logs
```

## Tables and decisions

- `users`: username unique, password hash only, role constrained to `ADMIN|OPERATOR`, timestamps UTC.
- `cameras`: name, IP, RTSP URL, location, status, `last_seen`; RTSP credentials must be redacted from API output and logs.
- `devices`: unique `device_code` matching `{device_id}` in MQTT topic; type, status, last seen and optional latest actuator values.
- `sensors`: device FK, sensor type/value, recorded time; index `(device_id, sensor_type, created_at DESC)`.
- `detection_events`: camera FK, object type, confidence constrained `[0,1]`, event type, image path, event timestamp; indexes on camera/time and type/time.
- `alerts`: nullable detection FK (supports non-AI alerts), severity `INFO|WARNING|CRITICAL`, description, status, handler FK and handled time; indexes on status/time, severity/time.
- `device_logs`: device FK, command ID, command, result, error/ACK detail and timestamps; command ID unique when present.
- `system_logs`: nullable actor FK for system-generated actions, action, description, timestamp; index on time and actor.

Foreign keys use restrictive or `SET NULL` behavior for audit/event retention; avoid cascading deletion of historical alerts when a camera/device/user is removed. Soft-delete policy for cameras/devices is preferred after history exists.

## Indexes and retention

Use B-tree indexes for FK and common time filters. For larger event tables, consider monthly range partitioning and configurable retention after measurement; avoid premature partitioning in the student demo. Store timestamps as `TIMESTAMPTZ` in UTC. Use Alembic revisions for evolving deployed schema; bootstrap SQL and seed data are for clean local initialization only.

## Seed data

`seed.sql` uses clearly named demo entities and never inserts a default plaintext password. Create an initial admin via a one-time CLI/secure setup flow that hashes a password supplied interactively or by secret, before enabling deployment.
