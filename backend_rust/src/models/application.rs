use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Application {
    pub id: Uuid,
    pub student_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub university_id: i32,
    pub university_name: String,
    pub course_name: String,
    pub degree_level: String,
    pub status: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
