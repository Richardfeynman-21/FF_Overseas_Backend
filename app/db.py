"""
Database connection pool manager for the universities database.
Uses asyncpg directly for high-performance async PostgreSQL access.
"""

import logging
import asyncpg

from app.config import settings

logger = logging.getLogger(__name__)

_pool: asyncpg.Pool | None = None


async def run_migrations(pool: asyncpg.Pool) -> None:
    """Run universities database schema migrations on startup."""
    async with pool.acquire() as conn:
        try:
            # Create indexes on foreign keys to optimize joins and prevent 524 timeouts
            await conn.execute(
                """
                CREATE INDEX IF NOT EXISTS idx_undergraduate_courses_uni_id ON undergraduate_courses(university_id);
                CREATE INDEX IF NOT EXISTS idx_postgraduate_courses_uni_id ON postgraduate_courses(university_id);
                CREATE INDEX IF NOT EXISTS idx_university_rankings_uni_id ON university_rankings(university_id);
                CREATE INDEX IF NOT EXISTS idx_university_scholarships_uni_id ON university_scholarships(university_id);
                """
            )

            # Check if column exists
            exists = await conn.fetchval(
                """
                SELECT EXISTS (
                    SELECT 1 
                    FROM information_schema.columns 
                    WHERE table_name='universities' AND column_name='course_count'
                );
                """
            )
            if not exists:
                logger.info("Migrating universities database: Adding course_count column...")
                await conn.execute(
                    """
                    ALTER TABLE universities ADD COLUMN course_count INTEGER DEFAULT 0;
                    UPDATE universities u SET course_count = (
                        SELECT COUNT(*) FROM (
                            SELECT id FROM undergraduate_courses WHERE university_id = u.id
                            UNION ALL
                            SELECT id FROM postgraduate_courses WHERE university_id = u.id
                        ) AS all_courses
                    );
                    CREATE INDEX IF NOT EXISTS idx_universities_course_count ON universities(course_count DESC);
                    """
                )
                logger.info("Universities database migration completed successfully.")
            else:
                logger.info("Universities database is up to date (course_count column exists).")
        except Exception as e:
            logger.error(f"Failed to run universities database migration: {e}")


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
        await run_migrations(_pool)
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
