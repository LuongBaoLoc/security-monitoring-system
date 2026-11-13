from contextlib import asynccontextmanager

from fastapi import FastAPI, WebSocket
from fastapi.middleware.cors import CORSMiddleware

from app.core.config import settings
from app.database.base import Base
from app.database.connection import SessionLocal, engine
from app.models import User
from app.routers import api, dashboard_socket
from app.services import seed_demo_data_if_empty


@asynccontextmanager
async def lifespan(_: FastAPI):
    Base.metadata.create_all(bind=engine)
    # A configurable bootstrap account is created only when both values are supplied.
    import os
    username, password = os.getenv("BOOTSTRAP_ADMIN_USERNAME"), os.getenv("BOOTSTRAP_ADMIN_PASSWORD")
    with SessionLocal() as db:
        if username and password:
            from app.core.security import hash_password
            if not db.query(User).filter(User.username == username).first():
                db.add(User(username=username, password_hash=hash_password(password), full_name="System Administrator", role="ADMIN"))
                db.commit()
        if settings.seed_demo_data:
            seed_demo_data_if_empty(db)
    from app.mqtt import start_mqtt, stop_mqtt
    import asyncio
    start_mqtt(asyncio.get_running_loop())
    yield
    stop_mqtt()


app = FastAPI(title=settings.app_name, version="1.0.0", lifespan=lifespan)
app.add_middleware(CORSMiddleware, allow_origins=settings.cors_origin_list, allow_credentials=True, allow_methods=["*"], allow_headers=["*"])
app.include_router(api)


@app.get("/")
def root():
    return {"name": settings.app_name, "docs": "/docs", "health": "/api/system/health"}


@app.get("/health")
def health():
    return {"status": "ok"}


@app.websocket("/ws/dashboard")
async def ws_dashboard(websocket: WebSocket, token: str):
    await dashboard_socket(websocket, token)
