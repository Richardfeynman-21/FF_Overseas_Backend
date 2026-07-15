use sqlx::PgPool;
use uuid::Uuid;
use crate::models::visa::VisaStep;
use crate::errors::AppError;

pub async fn get_or_init_visa_steps(
    pool: &PgPool,
    student_id: Uuid,
) -> Result<Vec<VisaStep>, AppError> {
    // 1. Try to fetch existing steps
    let steps = sqlx::query_as::<_, VisaStep>(
        "SELECT id, student_id, step_index, name, status, date_completed,
                description, checklist, documents, created_at, updated_at
         FROM student_visa_steps
         WHERE student_id = $1
         ORDER BY step_index ASC"
    )
    .bind(student_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)?;

    if !steps.is_empty() {
        return Ok(steps);
    }

    // 2. If empty, initialize the 6 standard steps
    let default_steps = vec![
        (
            0,
            "DS-160 Form Submission",
            "completed",
            Some("Jun 12, 2026".to_string()),
            "The DS-160 Online Nonimmigrant Visa Application form must be completed and submitted online prior to your visa interview.",
            serde_json::json!([
                { "id": "ds-ceac", "label": "Create CEAC account & save application ID", "completed": true },
                { "id": "ds-fields", "label": "Complete all personal, travel, and sponsor fields", "completed": true },
                { "id": "ds-photo", "label": "Upload digital visa passport photograph", "completed": true },
                { "id": "ds-submit", "label": "Submit form & print DS-160 confirmation page", "completed": true }
            ]),
            serde_json::json!([
                { "name": "DS-160 Confirmation.pdf", "url": "#" },
                { "name": "Visa Photo Receipt.jpg", "url": "#" }
            ])
        ),
        (
            1,
            "SEVIS I-901 Fee Payment",
            "completed",
            Some("Jun 18, 2026".to_string()),
            "All international students must pay the SEVIS I-901 fee to register with the Student and Exchange Visitor Information System.",
            serde_json::json!([
                { "id": "sev-i20", "label": "Receive physical/digital I-20 from university", "completed": true },
                { "id": "sev-id", "label": "Locate SEVIS ID (starts with N) on form I-20", "completed": true },
                { "id": "sev-pay", "label": "Pay $350 fee on FMJfee.com portal", "completed": true },
                { "id": "sev-receipt", "label": "Download & print official payment receipt", "completed": true }
            ]),
            serde_json::json!([
                { "name": "SEVIS I-901 Receipt.pdf", "url": "#" },
                { "name": "I-20 Form Signed.pdf", "url": "#" }
            ])
        ),
        (
            2,
            "Interview Slot Booking",
            "active",
            None,
            "Schedule two appointments: one for Biometrics (VAC) and one for your Consular Interview at the US Embassy or Consulate.",
            serde_json::json!([
                { "id": "slot-cgi", "label": "Create profile on CGI Federal scheduling portal", "completed": true },
                { "id": "slot-fee", "label": "Pay the MRV visa application fee ($185)", "completed": false },
                { "id": "slot-vac", "label": "Book Biometrics / VAC appointment", "completed": false },
                { "id": "slot-consular", "label": "Book Consular interview slot", "completed": false }
            ]),
            serde_json::json!([
                { "name": "MRV Fee Payment Receipt", "url": "#" }
            ])
        ),
        (
            3,
            "Mock Interview Preparation",
            "pending",
            None,
            "Attend a live practice mock session with our senior counselor to build confidence and polish your verbal responses.",
            serde_json::json!([
                { "id": "mock-attend", "label": "Schedule & attend mock session with advisor", "completed": false },
                { "id": "mock-common", "label": "Review top 50 F-1 visa practice questions", "completed": false },
                { "id": "mock-financial", "label": "Organize liquid asset proofs & sponsor letters", "completed": false }
            ]),
            serde_json::json!([])
        ),
        (
            4,
            "Embassy Consular Interview",
            "pending",
            None,
            "Visit the Embassy or Consulate on the scheduled date for your interview. Bring all original documents in a physical folder.",
            serde_json::json!([
                { "id": "emb-vac", "label": "Attend Biometrics / fingerprinting session", "completed": false },
                { "id": "emb-consular", "label": "Attend Consular interview with original files", "completed": false },
                { "id": "emb-approval", "label": "Receive visa approval from consular officer", "completed": false }
            ]),
            serde_json::json!([])
        ),
        (
            5,
            "Passport Retrieval",
            "locked",
            None,
            "Track the status of your passport delivery or pick up location once the visa is printed.",
            serde_json::json!([
                { "id": "pass-track", "label": "Track passport status online via Blue Dart/CGI", "completed": false },
                { "id": "pass-receive", "label": "Retrieve passport with stamped visa", "completed": false }
            ]),
            serde_json::json!([])
        ),
    ];

    let mut inserted_steps = Vec::new();
    for (step_idx, name, status, date_comp, desc, checklist, documents) in default_steps {
        let id = Uuid::new_v4();
        let step = sqlx::query_as::<_, VisaStep>(
            "INSERT INTO student_visa_steps (
                id, student_id, step_index, name, status, date_completed, description, checklist, documents
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING id, student_id, step_index, name, status, date_completed, description, checklist, documents, created_at, updated_at"
        )
        .bind(id)
        .bind(student_id)
        .bind(step_idx)
        .bind(name)
        .bind(status)
        .bind(date_comp)
        .bind(desc)
        .bind(checklist)
        .bind(documents)
        .fetch_one(pool)
        .await
        .map_err(AppError::Database)?;
        
        inserted_steps.push(step);
    }

    Ok(inserted_steps)
}

pub async fn update_visa_step(
    pool: &PgPool,
    student_id: Uuid,
    step_index: i32,
    status: &str,
    checklist: serde_json::Value,
    date_completed: Option<String>,
) -> Result<VisaStep, AppError> {
    let step = sqlx::query_as::<_, VisaStep>(
        "UPDATE student_visa_steps
         SET status = $3,
             checklist = $4,
             date_completed = $5,
             updated_at = NOW()
         WHERE student_id = $1 AND step_index = $2
         RETURNING id, student_id, step_index, name, status, date_completed,
                   description, checklist, documents, created_at, updated_at"
    )
    .bind(student_id)
    .bind(step_index)
    .bind(status)
    .bind(checklist)
    .bind(date_completed)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("Visa step {} for student {} not found", step_index, student_id)))?;

    Ok(step)
}
