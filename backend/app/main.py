import asyncio
import logging
from contextlib import asynccontextmanager
from contextlib import suppress

from fastapi import FastAPI, Request, WebSocket
from fastapi.concurrency import run_in_threadpool
from fastapi.exceptions import RequestValidationError
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from sqlalchemy import select
from sqlalchemy.exc import SQLAlchemyError
from sqlalchemy.orm import Session

from app import mqtt
from app.core.config import settings
from app.core.security import hash_password
from app.database.base import initialize_schema
from app.database.connection import engine
from app.models import RuntimeSetting, User
from app.routers import api
from app.services import get_runtime_settings, seed_demo_data_if_empty
from app.websocket import broadcast, dashboard_socket

logger = logging.getLogger(__name__)


def prepare_database() -> None:
    with engine.begin() as connection:
        initialize_schema(connection)
        with Session(bind=connection) as db:
            username = settings.bootstrap_admin_username.strip()
            password = settings.bootstrap_admin_password
            if username and password:
                if not db.scalar(select(User.id).where(User.username == username)):
                    db.add(User(username=username, password_hash=hash_password(password),
                                full_name="System Administrator", role="ADMIN"))
                    db.flush()
            if db.get(RuntimeSetting, 1) is None:
                db.add(RuntimeSetting(id=1, values=get_runtime_settings(db).model_dump()))
            if settings.seed_demo_data:
                seed_demo_data_if_empty(db)
            db.flush()


async def monitor_commands() -> None:
    while True:
        try:
            command_ids = await run_in_threadpool(mqtt.expire_commands)
            for command_id in command_ids:
                await broadcast("device.command.updated", {"command_id": command_id, "status": "timeout"})
        except SQLAlchemyError:
            logger.warning("Unable to update expired MQTT commands")
        await asyncio.sleep(2)


@asynccontextmanager
async def lifespan(_: FastAPI):
    await run_in_threadpool(prepare_database)
    mqtt.start_mqtt(asyncio.get_running_loop())
    monitor = asyncio.create_task(monitor_commands()) if settings.mqtt_enabled else None
    try:
        yield
    finally:
        if monitor:
            monitor.cancel()
            with suppress(asyncio.CancelledError):
                await monitor
        await run_in_threadpool(mqtt.stop_mqtt)
        await run_in_threadpool(engine.dispose)


app = FastAPI(title=settings.app_name, version="1.0.0", lifespan=lifespan)
app.add_middleware(CORSMiddleware, allow_origins=settings.cors_origin_list, allow_credentials=True, allow_methods=["*"], allow_headers=["*"])
app.include_router(api)


@app.exception_handler(RequestValidationError)
async def validation_error(_: Request, exc: RequestValidationError):
    detail = [
        {key: item[key] for key in ("loc", "msg", "type")}
        for item in exc.errors()
    ]
    return JSONResponse(status_code=422, content={"detail": detail})


@app.get("/")
def root():
    return {"name": settings.app_name, "docs": "/docs", "health": "/api/system/health"}


@app.get("/health")
def health():
    return {"status": "ok"}


@app.websocket("/ws/dashboard")
async def ws_dashboard(websocket: WebSocket, token: str):
    await dashboard_socket(websocket, token)
