pub mod admin;
pub mod student;
pub mod agent;
pub mod application;
pub mod pipeline;
pub mod progress;
pub mod document;
pub mod chat;
pub mod enquiry;
pub mod token;
pub mod audit;
pub mod shortlist;
pub mod visa;


// SQLite-specific models for rate limiting and local cache
pub mod sqlite {
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
    pub struct RateLimit {
        pub key: String,
        pub timestamp: i64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
    pub struct CacheEntry {
        pub key: String,
        pub value: String,
        pub expires_at: i64,
    }
}
