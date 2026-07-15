use dashmap::DashMap;
use uuid::Uuid;
use std::sync::OnceLock;
use tokio::sync::mpsc::UnboundedSender;
use axum::extract::ws::Message;

pub struct ChatHub;

/// Helper function to retrieve the static global map of active user connections.
fn hub_map() -> &'static DashMap<Uuid, UnboundedSender<Message>> {
    static INSTANCE: OnceLock<DashMap<Uuid, UnboundedSender<Message>>> = OnceLock::new();
    INSTANCE.get_or_init(DashMap::new)
}

impl ChatHub {
    /// Register a user's WebSocket channel sender.
    pub fn register(user_id: Uuid, sender: UnboundedSender<Message>) {
        hub_map().insert(user_id, sender);
    }

    /// Deregister a user's connection.
    pub fn deregister(user_id: Uuid) {
        hub_map().remove(&user_id);
    }

    /// Send a message directly to a specific user's WebSocket.
    /// Returns true if the message was sent successfully, false if the user is offline/error.
    pub fn send_to_user(user_id: Uuid, message: Message) -> bool {
        if let Some(sender) = hub_map().get(&user_id) {
            sender.send(message).is_ok()
        } else {
            false
        }
    }

    /// Check if a user is currently registered as online.
    pub fn is_online(user_id: Uuid) -> bool {
        hub_map().contains_key(&user_id)
    }
}
