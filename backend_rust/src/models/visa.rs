use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VisaStep {
    pub id: Uuid,
    pub student_id: Uuid,
    pub step_index: i32,
    pub name: String,
    pub status: String,
    pub date_completed: Option<String>,
    pub description: String,
    pub checklist: serde_json::Value,
    pub documents: serde_json::Value,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
