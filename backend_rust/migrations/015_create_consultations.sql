-- Migration: 015_create_consultations.sql
CREATE TABLE IF NOT EXISTS consultations (
    id UUID PRIMARY KEY,
    student_id UUID REFERENCES students(id) ON DELETE CASCADE,
    booking_date DATE NOT NULL,
    booking_time VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'booked',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
