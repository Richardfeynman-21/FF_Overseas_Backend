-- Migration: 005_create_pipeline.sql
CREATE TABLE IF NOT EXISTS pipeline_stages (
    id UUID PRIMARY KEY,
    name VARCHAR(255) UNIQUE NOT NULL,
    order_index INT UNIQUE NOT NULL,
    description TEXT,
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);
