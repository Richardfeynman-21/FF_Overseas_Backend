-- Migration 013: Add assigned_agent_id to students table
ALTER TABLE students 
ADD COLUMN IF NOT EXISTS assigned_agent_id UUID REFERENCES agents(id) ON DELETE SET NULL;
