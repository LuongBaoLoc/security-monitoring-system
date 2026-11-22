"""Phân phối sự kiện realtime tới dashboard qua WebSocket."""
from fastapi import WebSocket, WebSocketDisconnect
from fastapi.concurrency import run_in_threadpool
from fastapi.encoders import jsonable_encoder
from jwt import InvalidTokenError

from app.core.security import authenticated_user
from app.database.connection import SessionLocal


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
    await connections.send(jsonable_encoder({"type": event_type, "data": data}))


def valid_token(token: str) -> bool:
    with SessionLocal() as db:
        try:
            return authenticated_user(token, db) is not None
        except (InvalidTokenError, TypeError, ValueError):
            return False


async def dashboard_socket(ws: WebSocket, token: str) -> None:
    if not await run_in_threadpool(valid_token, token):
        await ws.close(code=1008)
        return
    await connections.connect(ws)
    try:
        while True:
            await ws.receive_text()
    except WebSocketDisconnect:
        pass
    finally:
        connections.disconnect(ws)
