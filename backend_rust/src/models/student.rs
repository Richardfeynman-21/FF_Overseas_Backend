use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Student {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub full_name: String,
    pub phone: Option<String>,
    pub country: Option<String>,
    pub preferred_destination: Option<String>,
    pub preferred_degree_level: Option<String>,
    pub preferred_intake: Option<String>,
    pub profile_data: Option<serde_json::Value>,
    pub is_verified: bool,
    pub is_active: bool,
    pub status: Option<String>,
    pub created_at: DateTime<Utc>,
    pub assigned_agent_id: Option<Uuid>,
    #[sqlx(default)]
    pub assigned_agent_name: Option<String>,
}

/// Helper struct for the student profile data, which may contain sensitive fields
/// like passport number, test scores, etc. (which are encrypted using crypto utils)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentProfileData {
    pub passport_number_encrypted: Option<String>,
    pub test_scores_encrypted: Option<String>,
    pub academic_history: Option<serde_json::Value>,
    pub budget: Option<String>,
}
