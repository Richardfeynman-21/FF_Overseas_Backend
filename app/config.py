from pydantic_settings import BaseSettings, SettingsConfigDict
from typing import Optional

class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore"
    )
    
    # AI Chatbot API keys
    GEMINI_API_KEY: Optional[str] = None
    OPENROUTER_API_KEY: Optional[str] = None
    
    # Rate Limiting & Session Configuration
    FREE_QUESTION_LIMIT: int = 5
    RATE_LIMIT_MAX: int = 20
    RATE_LIMIT_WINDOW: int = 600  # 10 minutes in seconds

    # Access Control & Security
    ALLOWED_ORIGINS: str = "http://localhost:3000,http://localhost:5173,https://ffoverseas.in,https://www.ffoverseas.in"
    FRONTEND_API_KEY: Optional[str] = None

settings = Settings()
