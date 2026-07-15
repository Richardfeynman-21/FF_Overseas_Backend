-- Migration: 014_create_shortlist_and_visa.sql

CREATE TABLE IF NOT EXISTS student_shortlists (
    id UUID PRIMARY KEY,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    university_id INT NOT NULL,
    university_name VARCHAR(255) NOT NULL,
    course_name VARCHAR(255) NOT NULL,
    degree_level VARCHAR(50) NOT NULL,
    country VARCHAR(100) NOT NULL,
    ranking VARCHAR(50),
    tuition VARCHAR(100),
    scholarship VARCHAR(100),
    acceptance_rate VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(student_id, university_id, course_name)
);

CREATE TABLE IF NOT EXISTS student_visa_steps (
    id UUID PRIMARY KEY,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    step_index INT NOT NULL,
    name VARCHAR(255) NOT NULL,
    status VARCHAR(50) NOT NULL,
    date_completed VARCHAR(50),
    description TEXT NOT NULL,
    checklist JSONB NOT NULL,
    documents JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(student_id, step_index)
);
