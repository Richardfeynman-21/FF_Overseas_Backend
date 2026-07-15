use axum::{
    extract::{ws::{WebSocketUpgrade, WebSocket, Message}, State, Query},
    response::Response,
};
use uuid::Uuid;
use crate::state::AppState;
use crate::errors::AppError;
use crate::ws::messages::{ClientMessage, ServerMessage};
use crate::ws::hub::ChatHub;

#[derive(serde::Deserialize)]
pub struct WsParams {
    pub token: String,
}

/// Upgrades incoming HTTP GET requests at /ws/chat to a WebSocket connection.
/// Authenticates the user via the `token` query parameter.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsParams>,
    State(state): State<AppState>,
) -> Result<Response, AppError> {
    // Decode and validate token
    let claims = crate::auth::jwt::decode_access_token(&params.token, &state.config)
        .map_err(|_| AppError::Unauthorized("Invalid token".to_string()))?;

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, claims.sub, claims.role, state)))
}

/// Handles the WebSocket session lifecycle.
async fn handle_socket(
    mut socket: WebSocket,
    user_id: Uuid,
    role: String,
    state: AppState,
) {
    tracing::info!("User {} ({}) connected to chat WebSocket", user_id, role);

    // Create channel for sending messages to this user
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    ChatHub::register(user_id, tx);

    // Broadcast UserOnline to user's contacts
    if let Ok(contacts) = crate::services::chat_service::get_contacts_for_user(&state.pg_pool, user_id).await {
        let online_event = ServerMessage::UserOnline { user_id };
        if let Ok(msg_text) = serde_json::to_string(&online_event) {
            for contact_id in contacts {
                if ChatHub::is_online(contact_id) {
                    ChatHub::send_to_user(contact_id, Message::Text(msg_text.clone()));
                }
            }
        }
    }

    let mut last_heartbeat = std::time::Instant::now();
    let mut heartbeat_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    // Consume the first tick immediately so the first interval ticks in 30 seconds
    heartbeat_interval.tick().await;

    loop {
        tokio::select! {
            // Outgoing message to client
            outgoing = rx.recv() => {
                if let Some(msg) = outgoing {
                    if socket.send(msg).await.is_err() {
                        break;
                    }
                } else {
                    break;
                }
            }

            // Incoming message from client
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(msg)) => {
                        last_heartbeat = std::time::Instant::now();

                        match msg {
                            Message::Text(text) => {
                                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                                    match client_msg {
                                        ClientMessage::SendMessage { room_id, content, attachments } => {
                                            match crate::services::chat_service::create_message(
                                                &state.pg_pool,
                                                room_id,
                                                user_id,
                                                &role,
                                                &content,
                                                attachments,
                                            )
                                            .await {
                                                Ok(chat_msg) => {
                                                    let server_msg = ServerMessage::NewMessage { room_id, message: chat_msg };
                                                    if let Ok(text) = serde_json::to_string(&server_msg) {
                                                        // Send confirmation back to sender
                                                        let _ = socket.send(Message::Text(text.clone())).await;

                                                        // Send to other member
                                                        if let Some(other_id) = get_other_member_id(&state.pg_pool, room_id, user_id).await {
                                                            ChatHub::send_to_user(other_id, Message::Text(text));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let err_msg = ServerMessage::Error { message: format!("Failed to save message: {}", e) };
                                                    if let Ok(text) = serde_json::to_string(&err_msg) {
                                                        let _ = socket.send(Message::Text(text)).await;
                                                    }
                                                }
                                            }
                                        }
                                        ClientMessage::Typing { room_id } => {
                                            let user_name = crate::services::chat_service::get_user_name(&state.pg_pool, user_id)
                                                .await
                                                .unwrap_or_else(|_| "User".to_string());
                                            let server_msg = ServerMessage::UserTyping { room_id, user_id, user_name };
                                            if let Ok(text) = serde_json::to_string(&server_msg) {
                                                if let Some(other_id) = get_other_member_id(&state.pg_pool, room_id, user_id).await {
                                                    ChatHub::send_to_user(other_id, Message::Text(text));
                                                }
                                            }
                                        }
                                        ClientMessage::StopTyping { room_id } => {
                                            let server_msg = ServerMessage::UserStoppedTyping { room_id, user_id };
                                            if let Ok(text) = serde_json::to_string(&server_msg) {
                                                if let Some(other_id) = get_other_member_id(&state.pg_pool, room_id, user_id).await {
                                                    ChatHub::send_to_user(other_id, Message::Text(text));
                                                }
                                            }
                                        }
                                        ClientMessage::MarkRead { room_id, message_id } => {
                                            match crate::services::chat_service::mark_message_as_read(
                                                &state.pg_pool,
                                                room_id,
                                                message_id,
                                                user_id,
                                            )
                                            .await {
                                                Ok(_) => {
                                                    let server_msg = ServerMessage::ReadReceipt { room_id, message_id, reader_id: user_id };
                                                    if let Ok(text) = serde_json::to_string(&server_msg) {
                                                        if let Some(other_id) = get_other_member_id(&state.pg_pool, room_id, user_id).await {
                                                            ChatHub::send_to_user(other_id, Message::Text(text));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let err_msg = ServerMessage::Error { message: format!("Failed to mark read: {}", e) };
                                                    if let Ok(text) = serde_json::to_string(&err_msg) {
                                                        let _ = socket.send(Message::Text(text)).await;
                                                    }
                                                }
                                            }
                                        }
                                        ClientMessage::Ping => {
                                            let pong_msg = ServerMessage::Pong;
                                            if let Ok(text) = serde_json::to_string(&pong_msg) {
                                                let _ = socket.send(Message::Text(text)).await;
                                            }
                                        }
                                    }
                                } else {
                                    let err_msg = ServerMessage::Error { message: "Invalid payload format".to_string() };
                                    if let Ok(text) = serde_json::to_string(&err_msg) {
                                        let _ = socket.send(Message::Text(text)).await;
                                    }
                                }
                            }
                            Message::Binary(_) => {
                                // Only text message formats containing JSON are processed
                            }
                            Message::Ping(_) => {
                                let _ = socket.send(Message::Pong(vec![])).await;
                            }
                            Message::Pong(_) => {
                                // Heartbeat updated above
                            }
                            Message::Close(_) => {
                                break;
                            }
                        }
                    }
                    Some(Err(_)) | None => {
                        break;
                    }
                }
            }

            // Connection heartbeat check
            _ = heartbeat_interval.tick() => {
                if last_heartbeat.elapsed() > std::time::Duration::from_secs(40) {
                    tracing::warn!("WS Connection for user {} timed out (no heartbeat)", user_id);
                    break;
                }
                if socket.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
        }
    }

    // Cleanup session when connection terminates
    ChatHub::deregister(user_id);
    tracing::info!("User {} disconnected from chat WebSocket", user_id);

    // Broadcast UserOffline to contacts
    if let Ok(contacts) = crate::services::chat_service::get_contacts_for_user(&state.pg_pool, user_id).await {
        let offline_event = ServerMessage::UserOffline { user_id };
        if let Ok(msg_text) = serde_json::to_string(&offline_event) {
            for contact_id in contacts {
                if ChatHub::is_online(contact_id) {
                    ChatHub::send_to_user(contact_id, Message::Text(msg_text.clone()));
                }
            }
        }
    }
}

/// Helper function to retrieve the other member in a chat room.
async fn get_other_member_id(
    pool: &sqlx::PgPool,
    room_id: Uuid,
    current_user_id: Uuid,
) -> Option<Uuid> {
    match crate::services::chat_service::get_chat_room(pool, room_id).await {
        Ok(room) => {
            if room.student_id == Some(current_user_id) {
                room.agent_id
            } else if room.agent_id == Some(current_user_id) {
                room.student_id
            } else {
                None
            }
        }
        Err(_) => None,
    }
}
