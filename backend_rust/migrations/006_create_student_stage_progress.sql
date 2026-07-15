-- Migration: 006_create_student_stage_progress.sql
CREATE TABLE IF NOT EXISTS student_stage_progress (
    id UUID PRIMARY KEY,
    application_id UUID REFERENCES applications(id) ON DELETE CASCADE,
    stage_id UUID REFERENCES pipeline_stages(id) ON DELETE RESTRICT,
    status VARCHAR(50) DEFAULT 'not_started',
    notes TEXT,
    updated_by UUID,
    completed_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
