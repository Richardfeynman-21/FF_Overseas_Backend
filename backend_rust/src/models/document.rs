use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Document {
    pub id: Uuid,
    pub student_id: Option<Uuid>,
    pub application_id: Option<Uuid>,
    pub file_name: String,
    pub file_path: String,
    pub mime_type: String,
    pub file_size_bytes: i64,
    pub doc_type: String,
    pub status: Option<String>,
    pub verified_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
