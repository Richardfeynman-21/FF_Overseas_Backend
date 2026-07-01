"""
Database connection pool manager for the universities database.
Uses asyncpg directly for high-performance async PostgreSQL access.
"""

import logging
import asyncpg

from app.config import settings

logger = logging.getLogger(__name__)

_pool: asyncpg.Pool | None = None


async def init_pool() -> None:
    """Initialize the asyncpg connection pool for the universities database."""
    global _pool
    if _pool is not None:
        return

    try:
        _pool = await asyncpg.create_pool(
            dsn=settings.UNIVERSITIES_DATABASE_URL,
            min_size=2,
            max_size=10,
            command_timeout=30,
        )
        logger.info("Universities database pool initialized successfully.")
    except Exception as e:
        logger.error(f"Failed to initialize universities database pool: {e}")
        raise


async def close_pool() -> None:
    """Close the asyncpg connection pool."""
    global _pool
    if _pool is not None:
        await _pool.close()
        _pool = None
        logger.info("Universities database pool closed.")


def get_pool() -> asyncpg.Pool:
    """Get the current connection pool. Raises RuntimeError if not initialized."""
    if _pool is None:
        raise RuntimeError("Universities database pool is not initialized. Call init_pool() first.")
    return _pool
