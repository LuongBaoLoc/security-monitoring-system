from collections.abc import Generator

from sqlalchemy import create_engine, event
from sqlalchemy.engine import URL, make_url
from sqlalchemy.orm import DeclarativeBase, Session, sessionmaker

from app.core.config import settings


class Base(DeclarativeBase):
    pass


url = (
    URL.create(
        "postgresql+psycopg", username=settings.postgres_user, password=settings.postgres_password,
        host=settings.postgres_host, port=settings.postgres_port, database=settings.postgres_db,
    )
    if settings.postgres_host
    else make_url(settings.database_url.strip())
)
if url.drivername in {"postgres", "postgresql", "postgresql+asyncpg"}:
    url = url.set(drivername="postgresql+psycopg")

engine = create_engine(url, pool_pre_ping=True)
SessionLocal = sessionmaker(bind=engine, autoflush=False, expire_on_commit=False)

if engine.dialect.name == "sqlite":
    @event.listens_for(engine, "connect")
    def enable_foreign_keys(connection, _record):
        connection.execute("PRAGMA foreign_keys=ON")


def get_db() -> Generator[Session, None, None]:
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()
