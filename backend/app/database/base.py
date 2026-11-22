"""Initialize the schema and upgrade data written by the previous backend."""
from datetime import datetime, timezone

from sqlalchemy import Connection, inspect, text
from sqlalchemy.dialects.postgresql import INET, JSONB, UUID
from sqlalchemy.types import DateTime

from app.models import Base

TIMESTAMPS = {
    "users": ("created_at",),
    "cameras": ("last_seen", "created_at", "updated_at"),
    "devices": ("last_seen", "created_at"),
    "sensors": ("created_at",),
    "detection_events": ("event_timestamp", "created_at"),
    "alerts": ("created_at", "handled_at"),
    "device_logs": ("created_at", "completed_at"),
    "system_logs": ("created_at",),
}


def initialize_schema(connection: Connection) -> None:
    dialect = connection.dialect.name
    if dialect == "postgresql":
        connection.execute(text("SELECT pg_advisory_xact_lock(552903616)"))
        connection.execute(text("SET LOCAL TIME ZONE 'UTC'"))
    elif dialect == "sqlite":
        connection.exec_driver_sql("BEGIN IMMEDIATE")

    Base.metadata.create_all(connection)
    if dialect == "postgresql":
        upgrade_postgresql(connection)
    elif dialect == "sqlite":
        upgrade_sqlite(connection)
    for table in Base.metadata.sorted_tables:
        for index in table.indexes:
            index.create(connection, checkfirst=True)


def upgrade_postgresql(connection: Connection) -> None:
    inspector = inspect(connection)
    for table, names in TIMESTAMPS.items():
        column_types = {column["name"]: column["type"] for column in inspector.get_columns(table)}
        for name in names:
            datatype = column_types[name]
            if not isinstance(datatype, DateTime) or not datatype.timezone:
                connection.exec_driver_sql(
                    f"ALTER TABLE {table} ALTER COLUMN {name} TYPE TIMESTAMPTZ USING {name}::timestamptz"
                )
        if table == "sensors" and not isinstance(column_types["sensor_value"], JSONB):
            connection.exec_driver_sql(
                "ALTER TABLE sensors ALTER COLUMN sensor_value TYPE JSONB USING sensor_value::jsonb"
            )
        if table == "device_logs":
            if not isinstance(column_types["detail"], JSONB):
                connection.exec_driver_sql(
                    "ALTER TABLE device_logs ALTER COLUMN detail TYPE JSONB USING detail::jsonb"
                )
            if isinstance(column_types["command_id"], UUID):
                connection.exec_driver_sql(
                    "ALTER TABLE device_logs ALTER COLUMN command_id TYPE VARCHAR(36) USING command_id::text"
                )
        if table == "cameras" and isinstance(column_types["ip_address"], INET):
            connection.exec_driver_sql(
                "ALTER TABLE cameras ALTER COLUMN ip_address TYPE VARCHAR(64) "
                "USING CASE WHEN masklen(ip_address) = CASE family(ip_address) WHEN 4 THEN 32 ELSE 128 END "
                "THEN host(ip_address) ELSE ip_address::text END"
            )


def upgrade_sqlite(connection: Connection) -> None:
    def normalize_timestamp(value):
        parsed = datetime.fromisoformat(value)
        utc = parsed.astimezone(timezone.utc) if parsed.tzinfo else parsed
        return utc.strftime("%Y-%m-%d %H:%M:%S.%f")

    raw = connection.connection.driver_connection
    raw.create_function("normalize_legacy_timestamp", 1, normalize_timestamp)
    try:
        for table, names in TIMESTAMPS.items():
            for name in names:
                connection.exec_driver_sql(
                    f"UPDATE {table} SET {name} = normalize_legacy_timestamp({name}) "
                    f"WHERE instr({name}, 'T') > 0 OR instr({name}, '+') > 0"
                )
    finally:
        raw.create_function("normalize_legacy_timestamp", 1, None)
