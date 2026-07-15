use sqlx::PgPool;
use uuid::Uuid;
use crate::models::chat::{ChatRoom, ChatMessage};
use crate::errors::AppError;

/// Retrieve all active chat rooms for a user, depending on their role.
pub async fn get_chat_rooms_for_user(
    pool: &PgPool,
    user_id: Uuid,
    role: &str,
) -> Result<Vec<ChatRoom>, AppError> {
    let rooms = if role == "student" {
        sqlx::query_as::<_, ChatRoom>(
            "SELECT id, student_id, agent_id, room_type, is_active, last_message_at, created_at \
             FROM chat_rooms \
             WHERE student_id = $1 AND is_active = true \
             ORDER BY last_message_at DESC NULLS LAST, created_at DESC"
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?
    } else {
        sqlx::query_as::<_, ChatRoom>(
            "SELECT id, student_id, agent_id, room_type, is_active, last_message_at, created_at \
             FROM chat_rooms \
             WHERE agent_id = $1 AND is_active = true \
             ORDER BY last_message_at DESC NULLS LAST, created_at DESC"
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?
    };
    Ok(rooms)
}

/// Retrieve a single chat room by its ID.
pub async fn get_chat_room(
    pool: &PgPool,
    room_id: Uuid,
) -> Result<ChatRoom, AppError> {
    sqlx::query_as::<_, ChatRoom>(
        "SELECT id, student_id, agent_id, room_type, is_active, last_message_at, created_at \
         FROM chat_rooms \
         WHERE id = $1"
    )
    .bind(room_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("Chat room with ID {} not found", room_id)))
}

/// Create a new chat room or re-activate/retrieve an existing one for the student/agent pair.
pub async fn create_or_get_chat_room(
    pool: &PgPool,
    student_id: Uuid,
    agent_id: Uuid,
    room_type: &str,
) -> Result<ChatRoom, AppError> {
    // Check if room already exists
    let existing = sqlx::query_as::<_, ChatRoom>(
        "SELECT id, student_id, agent_id, room_type, is_active, last_message_at, created_at \
         FROM chat_rooms \
         WHERE student_id = $1 AND agent_id = $2 AND room_type = $3"
    )
    .bind(student_id)
    .bind(agent_id)
    .bind(room_type)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?;

    if let Some(room) = existing {
        if !room.is_active {
            // Re-activate room
            let updated = sqlx::query_as::<_, ChatRoom>(
                "UPDATE chat_rooms \
                 SET is_active = true \
                 WHERE id = $1 \
                 RETURNING id, student_id, agent_id, room_type, is_active, last_message_at, created_at"
            )
            .bind(room.id)
            .fetch_one(pool)
            .await
            .map_err(AppError::Database)?;
            return Ok(updated);
        }
        return Ok(room);
    }

    // Create a new room
    let room_id = Uuid::new_v4();
    let new_room = sqlx::query_as::<_, ChatRoom>(
        "INSERT INTO chat_rooms (id, student_id, agent_id, room_type, is_active, created_at) \
         VALUES ($1, $2, $3, $4, true, NOW()) \
         RETURNING id, student_id, agent_id, room_type, is_active, last_message_at, created_at"
    )
    .bind(room_id)
    .bind(student_id)
    .bind(agent_id)
    .bind(room_type)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(new_room)
}

/// Retrieve paginated messages for a chat room.
pub async fn get_messages(
    pool: &PgPool,
    room_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<ChatMessage>, AppError> {
    sqlx::query_as::<_, ChatMessage>(
        "SELECT id, room_id, sender_id, sender_role, content, attachments, is_read, created_at \
         FROM chat_messages \
         WHERE room_id = $1 \
         ORDER BY created_at DESC \
         LIMIT $2 OFFSET $3"
    )
    .bind(room_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)
}

/// Insert a new message into a chat room and update the last_message_at timestamp of the room.
pub async fn create_message(
    pool: &PgPool,
    room_id: Uuid,
    sender_id: Uuid,
    sender_role: &str,
    content: &str,
    attachments: Option<serde_json::Value>,
) -> Result<ChatMessage, AppError> {
    let mut tx = pool.begin().await.map_err(AppError::Database)?;

    let message_id = Uuid::new_v4();
    let message = sqlx::query_as::<_, ChatMessage>(
        "INSERT INTO chat_messages (id, room_id, sender_id, sender_role, content, attachments, is_read, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, false, NOW()) \
         RETURNING id, room_id, sender_id, sender_role, content, attachments, is_read, created_at"
    )
    .bind(message_id)
    .bind(room_id)
    .bind(sender_id)
    .bind(sender_role)
    .bind(content)
    .bind(attachments)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::Database)?;

    sqlx::query(
        "UPDATE chat_rooms \
         SET last_message_at = NOW() \
         WHERE id = $1"
    )
    .bind(room_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Database)?;

    if sender_role == "student" {
        sqlx::query(
            "UPDATE students SET status = 'in_progress' WHERE id = $1 AND status = 'lead'"
        )
        .bind(sender_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
    }

    tx.commit().await.map_err(AppError::Database)?;

    Ok(message)
}

/// Mark a specific message in a room as read by a recipient.
pub async fn mark_message_as_read(
    pool: &PgPool,
    room_id: Uuid,
    message_id: Uuid,
    reader_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE chat_messages \
         SET is_read = true \
         WHERE id = $1 AND room_id = $2 AND sender_id != $3"
    )
    .bind(message_id)
    .bind(room_id)
    .bind(reader_id)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(())
}

/// Retrieve the names/IDs of all contacts (other participants in active rooms) for a user.
pub async fn get_contacts_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    #[derive(sqlx::FromRow)]
    struct ContactRow {
        student_id: Option<Uuid>,
        agent_id: Option<Uuid>,
    }

    let rows = sqlx::query_as::<_, ContactRow>(
        "SELECT student_id, agent_id \
         FROM chat_rooms \
         WHERE (student_id = $1 OR agent_id = $1) AND is_active = true"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)?;

    let mut contacts = Vec::new();
    for row in rows {
        if let Some(sid) = row.student_id {
            if sid != user_id {
                contacts.push(sid);
            }
        }
        if let Some(aid) = row.agent_id {
            if aid != user_id {
                contacts.push(aid);
            }
        }
    }
    contacts.sort();
    contacts.dedup();
    Ok(contacts)
}

/// Retrieve a user's full name based on their ID, checking students, agents, and admin_users tables.
pub async fn get_user_name(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<String, AppError> {
    if let Some(name) = sqlx::query_scalar::<_, String>(
        "SELECT full_name FROM students WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)? {
        return Ok(name);
    }

    if let Some(name) = sqlx::query_scalar::<_, String>(
        "SELECT full_name FROM agents WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)? {
        return Ok(name);
    }

    if let Some(name) = sqlx::query_scalar::<_, String>(
        "SELECT full_name FROM admin_users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)? {
        return Ok(name);
    }

    Ok("Unknown User".to_string())
}
