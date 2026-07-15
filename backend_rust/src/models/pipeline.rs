use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PipelineStage {
    pub id: Uuid,
    pub name: String,
    pub order_index: i32,
    pub description: Option<String>,
    pub is_active: bool,
}
