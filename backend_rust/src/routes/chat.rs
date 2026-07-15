use axum::{
    extract::{State, Query, Path},
    routing::get,
    Json,
    Router,
};
use serde::Deserialize;
use uuid::Uuid;
use crate::state::AppState;
use crate::errors::AppError;
use crate::auth::middleware::AuthUser;
use crate::models::chat::{ChatRoom, ChatMessage};
use crate::services::chat_service;

#[derive(Deserialize)]
pub struct CreateRoomRequest {
    pub student_id: Uuid,
    pub agent_id: Uuid,
    pub room_type: String,
}

#[derive(Deserialize)]
pub struct MessagePagination {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn chat_router() -> Router<AppState> {
    Router::new()
        .route("/rooms", get(list_rooms).post(create_room))
        .route("/rooms/:id/messages", get(get_room_messages))
}

/// GET /api/chat/rooms
/// Lists user's active rooms.
async fn list_rooms(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<ChatRoom>>, AppError> {
    let rooms = chat_service::get_chat_rooms_for_user(
        &state.pg_pool,
        auth_user.claims.sub,
        &auth_user.claims.role,
    )
    .await?;
    Ok(Json(rooms))
}

/// POST /api/chat/rooms
/// Creates a chat room or gets an existing one.
async fn create_room(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(payload): Json<CreateRoomRequest>,
) -> Result<Json<ChatRoom>, AppError> {
    if auth_user.claims.role == "student" {
        if payload.student_id != auth_user.claims.sub {
            return Err(AppError::Forbidden("Forbidden: You can only create chat rooms for yourself".to_string()));
        }

        let assigned_agent_id = sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT assigned_agent_id FROM students WHERE id = $1"
        )
        .bind(auth_user.claims.sub)
        .fetch_one(&state.pg_pool)
        .await
        .map_err(AppError::Database)?;

        match assigned_agent_id {
            Some(agent_id) if agent_id == payload.agent_id => {}
            _ => {
                return Err(AppError::Forbidden("Forbidden: You can only chat with your assigned advisor".to_string()));
            }
        }
    } else if auth_user.claims.role == "agent" {
        if payload.agent_id != auth_user.claims.sub {
            return Err(AppError::Forbidden("Forbidden: You can only create chat rooms for yourself".to_string()));
        }

        let assigned_agent_id = sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT assigned_agent_id FROM students WHERE id = $1"
        )
        .bind(payload.student_id)
        .fetch_optional(&state.pg_pool)
        .await
        .map_err(AppError::Database)?
        .flatten();

        match assigned_agent_id {
            Some(agent_id) if agent_id == auth_user.claims.sub => {}
            _ => {
                return Err(AppError::Forbidden("Forbidden: You are not assigned to this student".to_string()));
            }
        }
    }

    let room = chat_service::create_or_get_chat_room(
        &state.pg_pool,
        payload.student_id,
        payload.agent_id,
        &payload.room_type,
    )
    .await?;
    Ok(Json(room))
}

/// GET /api/chat/rooms/:id/messages
/// Retrieve a paginated list of messages for a chat room.
async fn get_room_messages(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(room_id): Path<Uuid>,
    Query(pagination): Query<MessagePagination>,
) -> Result<Json<Vec<ChatMessage>>, AppError> {
    // 1. Fetch the room to verify it exists
    let room = chat_service::get_chat_room(&state.pg_pool, room_id).await?;

    // 2. Authorize the user (they must be the student, agent or an admin/superadmin)
    let user_id = auth_user.claims.sub;
    let role = auth_user.claims.role.as_str();

    if role != "admin" && role != "superadmin" {
        if room.student_id != Some(user_id) && room.agent_id != Some(user_id) {
            return Err(AppError::Forbidden("You are not a participant in this chat room".to_string()));
        }
    }

    let limit = pagination.limit.unwrap_or(50);
    let offset = pagination.offset.unwrap_or(0);

    let messages = chat_service::get_messages(&state.pg_pool, room_id, limit, offset).await?;
    Ok(Json(messages))
}
