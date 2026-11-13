"""Phân phối sự kiện realtime tới dashboard qua WebSocket."""
from fastapi import WebSocket, WebSocketDisconnect
from jose import jwt

from app.core.config import settings
from app.database.connection import SessionLocal
from app.models import User


class Connections:
    def __init__(self) -> None:
        self.clients: set[WebSocket] = set()

    async def connect(self, ws: WebSocket) -> None:
        await ws.accept()
        self.clients.add(ws)

    def disconnect(self, ws: WebSocket) -> None:
        self.clients.discard(ws)

    async def send(self, message: dict) -> None:
        for ws in list(self.clients):
            try:
                await ws.send_json(message)
            except Exception:
                self.disconnect(ws)


connections = Connections()


async def broadcast(event_type: str, data: dict) -> None:
    await connections.send({"type": event_type, "data": data})


async def dashboard_socket(ws: WebSocket, token: str) -> None:
    try:
        claims = jwt.decode(token, settings.secret_key, algorithms=["HS256"])
        with SessionLocal() as db:
            user = db.get(User, int(claims["sub"]))
            if not user or not user.is_active:
                raise ValueError("invalid user")
    except Exception:
        await ws.close(code=1008)
        return
    await connections.connect(ws)
    try:
        while True:
            await ws.receive_text()
    except WebSocketDisconnect:
        connections.disconnect(ws)
