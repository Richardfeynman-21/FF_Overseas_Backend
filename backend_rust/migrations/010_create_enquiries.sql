-- Migration: 010_create_enquiries.sql
CREATE TABLE IF NOT EXISTS enquiries (
    id UUID PRIMARY KEY,
    student_id UUID REFERENCES students(id) ON DELETE SET NULL,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL,
    phone VARCHAR(50) NOT NULL,
    preferred_destination VARCHAR(100) NOT NULL,
    message TEXT,
    status VARCHAR(50) DEFAULT 'new',
    assigned_agent UUID REFERENCES agents(id) ON DELETE SET NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
