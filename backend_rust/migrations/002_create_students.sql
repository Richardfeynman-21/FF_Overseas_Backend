-- Migration 002: Create students table
CREATE TABLE IF NOT EXISTS students (
    id UUID PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    full_name VARCHAR(255) NOT NULL,
    phone VARCHAR(50),
    country VARCHAR(100),
    preferred_destination VARCHAR(100),
    preferred_degree_level VARCHAR(100),
    preferred_intake VARCHAR(100),
    profile_data JSONB, -- Stores sensitive encrypted items and general profile
    is_verified BOOLEAN NOT NULL DEFAULT FALSE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    status VARCHAR(50) DEFAULT 'lead',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
