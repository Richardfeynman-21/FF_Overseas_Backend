-- Migration: 004_create_applications.sql
CREATE TABLE IF NOT EXISTS applications (
    id UUID PRIMARY KEY,
    student_id UUID REFERENCES students(id) ON DELETE CASCADE,
    agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,
    university_id INT NOT NULL,
    university_name VARCHAR(255) NOT NULL,
    course_name VARCHAR(255) NOT NULL,
    degree_level VARCHAR(50) NOT NULL,
    status VARCHAR(50) DEFAULT 'draft',
    metadata JSONB,
    submitted_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
