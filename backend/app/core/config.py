"""Application settings loaded from environment."""
from functools import lru_cache

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    app_name: str = "Security Monitoring API"
    database_url: str = "sqlite:///./security_monitor.db"
    postgres_host: str = ""
    postgres_port: int = 5432
    postgres_db: str = "security_monitor"
    postgres_user: str = "security"
    postgres_password: str = ""
    secret_key: str = "change-this-development-secret-before-deploying"
    access_token_expire_minutes: int = 60
    registration_enabled: bool = False
    seed_demo_data: bool = False
    ai_service_token: str = ""
    bootstrap_admin_username: str = ""
    bootstrap_admin_password: str = ""
    cors_origins: str = "http://localhost:8080,http://127.0.0.1:8080,http://localhost:5173"
    alert_confidence_threshold: float = 0.8
    alert_deduplication_seconds: int = 60
    mqtt_host: str = "localhost"
    mqtt_port: int = 1883
    mqtt_username: str = ""
    mqtt_password: str = ""
    mqtt_enabled: bool = False
    mqtt_command_timeout_seconds: int = Field(default=15, ge=1)
    ai_health_url: str = "http://127.0.0.1:8001/health"

    model_config = SettingsConfigDict(env_file=".env", env_file_encoding="utf-8", extra="ignore")

    @property
    def cors_origin_list(self) -> list[str]:
        return [origin.strip() for origin in self.cors_origins.split(",") if origin.strip()]


@lru_cache
def get_settings() -> Settings:
    return Settings()


settings = get_settings()
