use serde::{Serialize, Deserialize};
use uuid::Uuid;
use crate::models::chat::ChatMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClientMessage {
    SendMessage {
        room_id: Uuid,
        content: String,
        attachments: Option<serde_json::Value>,
    },
    Typing {
        room_id: Uuid,
    },
    StopTyping {
        room_id: Uuid,
    },
    MarkRead {
        room_id: Uuid,
        message_id: Uuid,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServerMessage {
    NewMessage {
        room_id: Uuid,
        message: ChatMessage,
    },
    UserTyping {
        room_id: Uuid,
        user_id: Uuid,
        user_name: String,
    },
    UserStoppedTyping {
        room_id: Uuid,
        user_id: Uuid,
    },
    ReadReceipt {
        room_id: Uuid,
        message_id: Uuid,
        reader_id: Uuid,
    },
    UserOnline {
        user_id: Uuid,
    },
    UserOffline {
        user_id: Uuid,
    },
    Pong,
    Error {
        message: String,
    },
}
