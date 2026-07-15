use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChatRoom {
    pub id: Uuid,
    pub student_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub room_type: Option<String>,
    pub is_active: bool,
    pub last_message_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChatMessage {
    pub id: Uuid,
    pub room_id: Option<Uuid>,
    pub sender_id: Uuid,
    pub sender_role: String,
    pub content: String,
    pub attachments: Option<serde_json::Value>,
    pub is_read: bool,
    pub created_at: DateTime<Utc>,
}
