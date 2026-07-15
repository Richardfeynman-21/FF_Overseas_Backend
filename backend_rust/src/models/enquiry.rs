use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Enquiry {
    pub id: Uuid,
    pub student_id: Option<Uuid>,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub preferred_destination: String,
    pub message: Option<String>,
    pub status: Option<String>,
    pub assigned_agent: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
