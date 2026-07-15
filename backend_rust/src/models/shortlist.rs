use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Shortlist {
    pub id: Uuid,
    pub student_id: Uuid,
    pub university_id: i32,
    pub university_name: String,
    pub course_name: String,
    pub degree_level: String,
    pub country: String,
    pub ranking: Option<String>,
    pub tuition: Option<String>,
    pub scholarship: Option<String>,
    pub acceptance_rate: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}
