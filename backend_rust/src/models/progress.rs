use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StudentStageProgress {
    pub id: Uuid,
    pub application_id: Option<Uuid>,
    pub stage_id: Option<Uuid>,
    pub status: Option<String>,
    pub notes: Option<String>,
    pub updated_by: Option<Uuid>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
