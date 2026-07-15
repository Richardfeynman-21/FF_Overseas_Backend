-- Migration: 008_create_chat_rooms.sql
CREATE TABLE IF NOT EXISTS chat_rooms (
    id UUID PRIMARY KEY,
    student_id UUID REFERENCES students(id) ON DELETE CASCADE,
    agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,
    room_type VARCHAR(50) DEFAULT 'general',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    last_message_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
