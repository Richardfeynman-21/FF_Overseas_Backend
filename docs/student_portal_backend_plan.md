# 🚀 Fly & Flourish — Student Portal Backend Implementation Plan

> **Stack:** FastAPI (Python) + PostgreSQL + JWT Auth  
> **Integrates with:** Existing Express frontend ([server.ts](file:///media/rishi/Ubuntu/SummerProjects/FFoverseas/my-frontend/server.ts)) and FastAPI chatbot backend ([main.py](file:///media/rishi/Ubuntu/SummerProjects/FFoverseas/Backend/main.py))

---

## Current State Analysis

| Layer | What Exists | What's Missing |
|-------|------------|----------------|
| **Frontend** | Student Login ([StudentLogin.tsx](file:///media/rishi/Ubuntu/SummerProjects/FFoverseas/my-frontend/src/pages/StudentLogin.tsx)), Dashboard ([StudentDashboard.tsx](file:///media/rishi/Ubuntu/SummerProjects/FFoverseas/my-frontend/src/pages/StudentDashboard.tsx)), Admin Config ([AdminConfig.tsx](file:///media/rishi/Ubuntu/SummerProjects/FFoverseas/my-frontend/src/pages/AdminConfig.tsx)) | All data is mocked with `localStorage` — no real auth or persistence |
| **Express Server** | Enquiry form → Google Sheets + Email notifications | No student accounts, no DB, no auth |
| **FastAPI Backend** | Public chatbot with Gemini/OpenRouter, rate limiting, sessions | No student-specific endpoints, no database |
| **Database** | ❌ None | Everything — need PostgreSQL with full schema |

---

## Architecture Overview

```mermaid
graph TB
    subgraph "Client (React + Vite)"
        A[StudentLogin.tsx] --> B[StudentDashboard.tsx]
        C[AdminConfig.tsx]
        D[ConsultationForm.tsx]
        E[ChatbotButton.tsx]
    end

    subgraph "Express Server (server.ts - Port 3000)"
        F["/api/enquiries — Form Submissions"]
        G["Vite Dev Middleware"]
    end

    subgraph "FastAPI Backend (Port 8000)"
        H["/api/public-chat — Chatbot"]
        I["/api/auth/* — Authentication"]
        J["/api/students/* — Student CRUD"]
        K["/api/applications/* — Application Tracking"]
        L["/api/documents/* — Document Management"]
        M["/api/admin/* — Admin Operations"]
        N["/api/universities/* — University Data"]
        O["/api/chat/* — Authenticated Chat"]
    end

    subgraph "PostgreSQL Database"
        P[(students)]
        Q[(applications)]
        R[(application_stages)]
        S[(documents)]
        T[(universities)]
        U[(chat_messages)]
        V[(enquiries)]
        W[(admin_users)]
    end

    A --> I
    B --> J & K & L & N & O
    C --> M
    D --> F
    E --> H

    I & J & K & L & M & N & O --> P & Q & R & S & T & U & V & W
```

---

## PostgreSQL Database Schema

### Table 1: `admin_users`
> Admin staff who manage student applications and configure pipeline stages.

```sql
CREATE TABLE admin_users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           VARCHAR(150) UNIQUE NOT NULL,
    password_hash   VARCHAR(255) NOT NULL,
    full_name       VARCHAR(100) NOT NULL,
    role            VARCHAR(20) NOT NULL DEFAULT 'counsellor'
                        CHECK (role IN ('super_admin', 'admin', 'counsellor')),
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed a super admin
-- INSERT INTO admin_users (email, password_hash, full_name, role)
-- VALUES ('admin@ffoverseas.in', '<bcrypt_hash>', 'Super Admin', 'super_admin');
```

### Table 2: `students`
> Registered students on the portal. Created by admin after an enquiry is qualified.

```sql
CREATE TABLE students (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           VARCHAR(150) UNIQUE NOT NULL,
    password_hash   VARCHAR(255) NOT NULL,
    full_name       VARCHAR(100) NOT NULL,
    phone           VARCHAR(30),
    date_of_birth   DATE,
    nationality     VARCHAR(60),
    passport_number VARCHAR(30),

    -- Academic Profile
    highest_degree  VARCHAR(50),                -- e.g. 'Bachelor', '12th Grade'
    field_of_study  VARCHAR(100),               -- e.g. 'Computer Science'
    gpa             DECIMAL(4,2),               -- e.g. 8.50 (out of 10)
    gpa_scale       SMALLINT DEFAULT 10,        -- 4 or 10
    english_test    VARCHAR(20),                -- 'IELTS', 'TOEFL', 'PTE', 'Duolingo'
    english_score   VARCHAR(20),                -- e.g. '7.5', '110'

    -- Preferences
    target_destination  VARCHAR(50),            -- 'USA', 'UK', 'Canada', etc.
    target_degree       VARCHAR(50),            -- 'Masters', 'Bachelors', 'PhD'
    target_intake       VARCHAR(30),            -- 'Fall 2026', 'Spring 2027'
    budget_range        VARCHAR(50),            -- e.g. '$30,000 - $50,000/yr'

    -- Assigned Counsellor
    assigned_admin_id   UUID REFERENCES admin_users(id) ON DELETE SET NULL,

    -- Account Status
    is_active           BOOLEAN NOT NULL DEFAULT TRUE,
    email_verified      BOOLEAN NOT NULL DEFAULT FALSE,
    profile_completed   BOOLEAN NOT NULL DEFAULT FALSE,
    avatar_url          TEXT,
    last_login_at       TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_students_email ON students(email);
CREATE INDEX idx_students_assigned_admin ON students(assigned_admin_id);
CREATE INDEX idx_students_destination ON students(target_destination);
```

### Table 3: `universities`
> Master catalogue of partner universities. Admin-managed.

```sql
CREATE TABLE universities (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            VARCHAR(200) NOT NULL,
    country         VARCHAR(60) NOT NULL,       -- 'USA', 'UK', 'Canada', etc.
    country_code    CHAR(2) NOT NULL,           -- 'US', 'GB', 'CA', etc.
    flag_emoji      VARCHAR(10),                -- '🇺🇸'
    city            VARCHAR(100),
    qs_ranking      VARCHAR(20),                -- 'QS #1'
    tuition_range   VARCHAR(100),               -- '$55,000 - $61,000/yr'
    scholarship_max VARCHAR(100),               -- 'Up to $30,000/yr'
    acceptance_rate VARCHAR(10),                -- '3.9%'
    programs        TEXT[] NOT NULL DEFAULT '{}', -- Array: ['CS', 'Engineering']
    website_url     TEXT,
    logo_url        TEXT,
    description     TEXT,
    is_partner      BOOLEAN NOT NULL DEFAULT TRUE,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_universities_country ON universities(country);
CREATE INDEX idx_universities_ranking ON universities(qs_ranking);
```

### Table 4: `applications`
> Each student can have multiple university applications. Core tracking entity.

```sql
CREATE TABLE applications (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id      UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    university_id   UUID NOT NULL REFERENCES universities(id) ON DELETE RESTRICT,
    program         VARCHAR(150) NOT NULL,      -- 'M.S. Computer Science'
    intake          VARCHAR(30),                -- 'Fall 2026'
    status          VARCHAR(30) NOT NULL DEFAULT 'draft'
                        CHECK (status IN (
                            'draft',
                            'submitted',
                            'under_review',
                            'offer_received',
                            'offer_accepted',
                            'offer_declined',
                            'rejected',
                            'deferred',
                            'withdrawn'
                        )),
    priority        SMALLINT DEFAULT 1 CHECK (priority BETWEEN 1 AND 5),
                                                -- 1=Ambitious, 3=Target, 5=Safe
    scholarship_amount  DECIMAL(12,2),
    scholarship_status  VARCHAR(20) DEFAULT 'not_applied'
                        CHECK (scholarship_status IN (
                            'not_applied', 'applied', 'awarded', 'rejected'
                        )),
    offer_letter_url    TEXT,
    notes               TEXT,
    submitted_at        TIMESTAMPTZ,
    decision_at         TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(student_id, university_id, program)
);

CREATE INDEX idx_applications_student ON applications(student_id);
CREATE INDEX idx_applications_status ON applications(status);
```

### Table 5: `pipeline_stages` (Template)
> Admin-defined pipeline template. Replaces the `localStorage`-based approach in [AdminConfig.tsx](file:///media/rishi/Ubuntu/SummerProjects/FFoverseas/my-frontend/src/pages/AdminConfig.tsx).

```sql
CREATE TABLE pipeline_stages (
    id              SERIAL PRIMARY KEY,
    name            VARCHAR(100) NOT NULL,
    description     TEXT,
    sort_order      SMALLINT NOT NULL DEFAULT 0,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed default stages
INSERT INTO pipeline_stages (name, description, sort_order) VALUES
    ('Profile Submitted',       'Student personal and academic profile recorded.',        1),
    ('Documents Verified',      'Submitted documents verified by admissions team.',       2),
    ('University Shortlisted',  'Best-fit universities matched to student profile.',      3),
    ('Application Sent',        'Finalized applications dispatched to universities.',     4),
    ('Offer Letter',            'Acceptance letters and offer confirmations received.',    5),
    ('Visa Processing',         'Visa prep, mock interviews, and embassy submission.',    6),
    ('Pre-Departure Briefing',  'Final orientation: accommodation, travel, cultural prep.', 7);
```

### Table 6: `student_stage_progress`
> Per-student progress through the pipeline. Links students to stages.

```sql
CREATE TABLE student_stage_progress (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id      UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    stage_id        INT NOT NULL REFERENCES pipeline_stages(id) ON DELETE CASCADE,
    status          VARCHAR(20) NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('completed', 'current', 'pending')),
    completed_at    TIMESTAMPTZ,
    notes           TEXT,
    updated_by      UUID REFERENCES admin_users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(student_id, stage_id)
);

CREATE INDEX idx_stage_progress_student ON student_stage_progress(student_id);
```

### Table 7: `documents`
> Student document uploads with verification tracking.

```sql
CREATE TABLE documents (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id      UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    doc_type        VARCHAR(50) NOT NULL
                        CHECK (doc_type IN (
                            'passport',
                            'transcripts',
                            'sop',
                            'lor',
                            'financial',
                            'english_test',
                            'photos',
                            'resume',
                            'other'
                        )),
    file_name       VARCHAR(255) NOT NULL,
    file_url        TEXT NOT NULL,               -- S3/Cloudinary/local path
    file_size_bytes BIGINT,
    mime_type       VARCHAR(100),
    status          VARCHAR(20) NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending', 'verified', 'rejected')),
    rejection_reason TEXT,
    verified_by     UUID REFERENCES admin_users(id),
    verified_at     TIMESTAMPTZ,
    uploaded_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_documents_student ON documents(student_id);
CREATE INDEX idx_documents_type ON documents(doc_type);
```

### Table 8: `enquiries`
> Migrates the current Google Sheets–only enquiry flow into the DB (while keeping Sheets as backup sync).

```sql
CREATE TABLE enquiries (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            VARCHAR(100) NOT NULL,
    email           VARCHAR(150) NOT NULL,
    phone           VARCHAR(30) NOT NULL,
    destination     VARCHAR(50),
    degree          VARCHAR(50),
    source          VARCHAR(50) DEFAULT 'website', -- 'website', 'referral', 'ad'
    status          VARCHAR(20) NOT NULL DEFAULT 'new'
                        CHECK (status IN ('new', 'contacted', 'qualified', 'converted', 'closed')),
    assigned_to     UUID REFERENCES admin_users(id),
    notes           TEXT,
    converted_student_id UUID REFERENCES students(id),
    sheet_synced    BOOLEAN DEFAULT FALSE,
    email_sent      BOOLEAN DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_enquiries_status ON enquiries(status);
CREATE INDEX idx_enquiries_email ON enquiries(email);
```

### Table 9: `chat_messages`
> Persists authenticated student chat (replaces the in-memory `chatMessages` state in the dashboard).

```sql
CREATE TABLE chat_messages (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id      UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    sender          VARCHAR(10) NOT NULL CHECK (sender IN ('student', 'bot', 'admin')),
    message         TEXT NOT NULL,
    metadata        JSONB,                       -- Store bot provider, tokens used, etc.
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_chat_student ON chat_messages(student_id);
CREATE INDEX idx_chat_created ON chat_messages(created_at DESC);
```

### Table 10: `refresh_tokens`
> JWT refresh token store for secure session management.

```sql
CREATE TABLE refresh_tokens (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL,               -- Can be student or admin
    user_type       VARCHAR(10) NOT NULL CHECK (user_type IN ('student', 'admin')),
    token_hash      VARCHAR(255) NOT NULL,
    device_info     VARCHAR(255),
    ip_address      INET,
    expires_at      TIMESTAMPTZ NOT NULL,
    revoked         BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id, user_type);
CREATE INDEX idx_refresh_tokens_hash ON refresh_tokens(token_hash);
```

### Table 11: `audit_log`
> Tracks all sensitive admin actions for accountability.

```sql
CREATE TABLE audit_log (
    id              BIGSERIAL PRIMARY KEY,
    actor_id        UUID NOT NULL,
    actor_type      VARCHAR(10) NOT NULL CHECK (actor_type IN ('student', 'admin')),
    action          VARCHAR(50) NOT NULL,        -- 'update_stage', 'verify_doc', 'create_student'
    entity_type     VARCHAR(30),                 -- 'student', 'application', 'document'
    entity_id       UUID,
    old_value       JSONB,
    new_value       JSONB,
    ip_address      INET,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_actor ON audit_log(actor_id);
CREATE INDEX idx_audit_entity ON audit_log(entity_type, entity_id);
CREATE INDEX idx_audit_time ON audit_log(created_at DESC);
```

---

## Entity Relationship Diagram

```mermaid
erDiagram
    admin_users ||--o{ students : "assigned counsellor"
    admin_users ||--o{ enquiries : "assigned to"
    admin_users ||--o{ documents : "verified by"
    admin_users ||--o{ student_stage_progress : "updated by"
    admin_users ||--o{ audit_log : "actor"

    students ||--o{ applications : "has many"
    students ||--o{ documents : "uploads"
    students ||--o{ student_stage_progress : "progresses through"
    students ||--o{ chat_messages : "sends/receives"

    universities ||--o{ applications : "target of"

    pipeline_stages ||--o{ student_stage_progress : "template for"

    enquiries ||--o| students : "converts to"

    students ||--o{ refresh_tokens : "authenticated via"
    admin_users ||--o{ refresh_tokens : "authenticated via"
```

---

## API Endpoints Design

### 🔐 Authentication (`/api/auth/`)

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| `POST` | `/api/auth/student/login` | Student email + password login → JWT | Public |
| `POST` | `/api/auth/admin/login` | Admin email + password login → JWT | Public |
| `POST` | `/api/auth/refresh` | Refresh access token using refresh token | Refresh Token |
| `POST` | `/api/auth/logout` | Revoke refresh token | Access Token |
| `POST` | `/api/auth/forgot-password` | Send password reset email | Public |
| `POST` | `/api/auth/reset-password` | Reset password with token | Reset Token |

### 👨‍🎓 Students (`/api/students/`)

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| `GET` | `/api/students/me` | Get current student profile | Student |
| `PUT` | `/api/students/me` | Update profile (phone, prefs, academic) | Student |
| `PUT` | `/api/students/me/password` | Change password | Student |
| `GET` | `/api/students/me/dashboard` | Aggregated dashboard data (stats, progress, recent activity) | Student |

### 📋 Applications (`/api/applications/`)

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| `GET` | `/api/applications/` | List student's applications | Student |
| `GET` | `/api/applications/{id}` | Get application details | Student |

### 📄 Documents (`/api/documents/`)

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| `GET` | `/api/documents/` | List student's documents with status | Student |
| `POST` | `/api/documents/upload` | Upload a document (multipart) | Student |
| `DELETE` | `/api/documents/{id}` | Remove a pending document | Student |

### 📊 Progress (`/api/progress/`)

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| `GET` | `/api/progress/` | Get student's pipeline progress | Student |
| `GET` | `/api/progress/stages` | Get pipeline stage template | Student |

### 🏫 Universities (`/api/universities/`)

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| `GET` | `/api/universities/` | List all partner universities (filterable by country) | Student |
| `GET` | `/api/universities/{id}` | Get university details | Student |

### 💬 Chat (`/api/chat/`)

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| `GET` | `/api/chat/history` | Get chat message history (paginated) | Student |
| `POST` | `/api/chat/send` | Send message → AI reply (persisted) | Student |

### 🛡️ Admin (`/api/admin/`)

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| `GET` | `/api/admin/students` | List all students (search, filter, paginate) | Admin |
| `POST` | `/api/admin/students` | Create a student account (from enquiry or manual) | Admin |
| `GET` | `/api/admin/students/{id}` | Get student full profile | Admin |
| `PUT` | `/api/admin/students/{id}` | Update student details | Admin |
| `DELETE` | `/api/admin/students/{id}` | Deactivate student | Admin |
| `PUT` | `/api/admin/students/{id}/stage` | Update a student's pipeline stage | Admin |
| `PUT` | `/api/admin/documents/{id}/verify` | Verify or reject a document | Admin |
| `GET` | `/api/admin/pipeline` | Get pipeline stage template | Admin |
| `POST` | `/api/admin/pipeline` | Add a pipeline stage | Admin |
| `PUT` | `/api/admin/pipeline/{id}` | Edit a pipeline stage | Admin |
| `DELETE` | `/api/admin/pipeline/{id}` | Remove a pipeline stage | Admin |
| `PUT` | `/api/admin/pipeline/reorder` | Reorder pipeline stages | Admin |
| `GET` | `/api/admin/enquiries` | List all enquiries | Admin |
| `PUT` | `/api/admin/enquiries/{id}` | Update enquiry status | Admin |
| `POST` | `/api/admin/enquiries/{id}/convert` | Convert enquiry → student account | Admin |
| `POST` | `/api/admin/applications` | Create application for a student | Admin |
| `PUT` | `/api/admin/applications/{id}` | Update application status | Admin |
| `GET` | `/api/admin/universities` | List universities (admin view) | Admin |
| `POST` | `/api/admin/universities` | Add university | Admin |
| `PUT` | `/api/admin/universities/{id}` | Edit university | Admin |
| `GET` | `/api/admin/audit-log` | View audit trail | Super Admin |

---

## Project Structure

```
Backend/
├── .env                        # Environment variables (DB, JWT secrets, etc.)
├── requirements.txt            # Python dependencies
├── alembic.ini                 # Alembic migration config
├── alembic/                    # Database migrations
│   ├── env.py
│   └── versions/
│       └── 001_initial_schema.py
│
├── app/
│   ├── __init__.py
│   ├── main.py                 # FastAPI app entry point
│   ├── config.py               # Settings (Pydantic BaseSettings)
│   ├── database.py             # SQLAlchemy engine + session
│   │
│   ├── models/                 # SQLAlchemy ORM models
│   │   ├── __init__.py
│   │   ├── admin.py
│   │   ├── student.py
│   │   ├── university.py
│   │   ├── application.py
│   │   ├── pipeline.py
│   │   ├── document.py
│   │   ├── enquiry.py
│   │   ├── chat.py
│   │   └── auth.py
│   │
│   ├── schemas/                # Pydantic request/response schemas
│   │   ├── __init__.py
│   │   ├── auth.py
│   │   ├── student.py
│   │   ├── university.py
│   │   ├── application.py
│   │   ├── pipeline.py
│   │   ├── document.py
│   │   ├── enquiry.py
│   │   └── chat.py
│   │
│   ├── routers/                # API route handlers
│   │   ├── __init__.py
│   │   ├── auth.py
│   │   ├── students.py
│   │   ├── universities.py
│   │   ├── applications.py
│   │   ├── progress.py
│   │   ├── documents.py
│   │   ├── chat.py
│   │   └── admin.py
│   │
│   ├── services/               # Business logic layer
│   │   ├── __init__.py
│   │   ├── auth_service.py     # JWT creation, password hashing
│   │   ├── student_service.py
│   │   ├── application_service.py
│   │   ├── document_service.py
│   │   ├── chat_service.py     # Gemini/OpenRouter integration
│   │   ├── email_service.py    # Nodemailer-equivalent via SMTP
│   │   └── sheets_service.py   # Google Sheets sync (optional)
│   │
│   ├── middleware/
│   │   ├── __init__.py
│   │   ├── auth.py             # JWT dependency injection
│   │   └── rate_limiter.py     # IP-based rate limiting
│   │
│   └── utils/
│       ├── __init__.py
│       ├── security.py         # Password hashing (bcrypt)
│       ├── validators.py       # Email, phone, etc.
│       └── file_upload.py      # File upload handling
│
└── tests/
    ├── __init__.py
    ├── conftest.py
    ├── test_auth.py
    ├── test_students.py
    └── test_admin.py
```

---

## Phased Implementation Roadmap

### Phase 1: Foundation (Days 1–2)
> [!IMPORTANT]
> This phase establishes the database and core infrastructure. Nothing works without this.

- [ ] Install PostgreSQL, create `ffoverseas_db` database
- [ ] Set up project structure with `app/` package
- [ ] Configure SQLAlchemy + Alembic migrations
- [ ] Create all 11 tables via migration
- [ ] Implement `config.py` with Pydantic `BaseSettings`
- [ ] Seed `pipeline_stages` and `universities` with existing data from [data.ts](file:///media/rishi/Ubuntu/SummerProjects/FFoverseas/my-frontend/src/data.ts)

### Phase 2: Authentication (Days 2–3)
> [!IMPORTANT]
> Replaces the fake `localStorage.setItem('ff_student_token')` in [StudentLogin.tsx](file:///media/rishi/Ubuntu/SummerProjects/FFoverseas/my-frontend/src/pages/StudentLogin.tsx).

- [ ] Implement bcrypt password hashing utilities
- [ ] Build JWT access + refresh token system
- [ ] Create `/api/auth/student/login` and `/api/auth/admin/login`
- [ ] Create `/api/auth/refresh` and `/api/auth/logout`
- [ ] Build `get_current_user` dependency for route protection
- [ ] Update frontend `StudentLogin.tsx` to call real API
- [ ] Add token storage (httpOnly cookie or secure localStorage)

### Phase 3: Student Portal APIs (Days 3–5)
> Replaces all hardcoded data in [StudentDashboard.tsx](file:///media/rishi/Ubuntu/SummerProjects/FFoverseas/my-frontend/src/pages/StudentDashboard.tsx).

- [ ] `GET /api/students/me` — profile data
- [ ] `GET /api/students/me/dashboard` — aggregated stats
- [ ] `GET /api/progress/` — pipeline progress per student
- [ ] `GET /api/universities/` — filterable university list
- [ ] `GET /api/applications/` — student's applications
- [ ] `GET /api/documents/` — document checklist with upload status
- [ ] `POST /api/documents/upload` — file upload (store locally or S3)

### Phase 4: Admin APIs (Days 5–7)
> Replaces `localStorage`-based admin in [AdminConfig.tsx](file:///media/rishi/Ubuntu/SummerProjects/FFoverseas/my-frontend/src/pages/AdminConfig.tsx).

- [ ] Student CRUD (create, list, update, deactivate)
- [ ] Pipeline stage CRUD (add, edit, delete, reorder)
- [ ] Stage progress updates (mark stage completed for a student)
- [ ] Document verification (approve/reject)
- [ ] Application management (create, update status)
- [ ] Enquiry management (list, status update, convert to student)
- [ ] Audit logging middleware

### Phase 5: Chat Integration (Days 7–8)
> Merges the existing chatbot from [main.py](file:///media/rishi/Ubuntu/SummerProjects/FFoverseas/Backend/main.py) into the new backend with persistence.

- [ ] Migrate Gemini/OpenRouter logic from existing `main.py`
- [ ] Add `chat_messages` persistence
- [ ] `POST /api/chat/send` — authenticated student chat (no free limit)
- [ ] `GET /api/chat/history` — paginated history
- [ ] Keep `/api/public-chat` for unauthenticated homepage chatbot

### Phase 6: Frontend Integration (Days 8–10)
> Wire up all React pages to real APIs.

- [ ] Create `api.ts` service layer with Axios/Fetch + interceptors
- [ ] Update `StudentLogin.tsx` → real JWT auth
- [ ] Update `StudentDashboard.tsx` → fetch from `/api/students/me/dashboard`
- [ ] Update university tab → fetch from `/api/universities/`
- [ ] Update progress tab → fetch from `/api/progress/`
- [ ] Update document checklist → fetch from `/api/documents/`
- [ ] Update chat tab → fetch from `/api/chat/`
- [ ] Update `AdminConfig.tsx` → call `/api/admin/pipeline/*`
- [ ] Add proper error handling, loading states, and toast notifications

---

## Environment Variables (`.env`)

```env
# Database
DATABASE_URL=postgresql+asyncpg://ffoverseas:your_password@localhost:5432/ffoverseas_db

# JWT
JWT_SECRET_KEY=your-256-bit-secret-key-change-this-in-production
JWT_ALGORITHM=HS256
JWT_ACCESS_TOKEN_EXPIRE_MINUTES=30
JWT_REFRESH_TOKEN_EXPIRE_DAYS=7

# AI Chatbot
GEMINI_API_KEY=your_gemini_key
OPENROUTER_API_KEY=your_openrouter_key

# Email (SMTP)
SMTP_HOST=smtp.gmail.com
SMTP_PORT=465
SMTP_USER=your_email@gmail.com
SMTP_PASS=your_app_password

# Admin
ADMIN_EMAIL=admin@ffoverseas.in

# Google Sheets (optional sync)
GOOGLE_SPREADSHEET_ID=your_sheet_id
GOOGLE_SERVICE_ACCOUNT_JSON='{...}'

# File Uploads
UPLOAD_DIR=./uploads
MAX_UPLOAD_SIZE_MB=10
```

---

## Updated `requirements.txt`

```txt
# Core
fastapi>=0.115.0
uvicorn[standard]>=0.30.0
pydantic>=2.8.0
pydantic-settings>=2.4.0

# Database
sqlalchemy[asyncio]>=2.0.30
asyncpg>=0.30.0
alembic>=1.14.0

# Authentication
python-jose[cryptography]>=3.3.0
passlib[bcrypt]>=1.7.4
bcrypt>=4.2.0

# HTTP & AI
httpx>=0.27.0
python-multipart>=0.0.12

# Email
aiosmtplib>=3.0.0

# Utilities
python-dotenv>=1.0.1

# File uploads
aiofiles>=24.0.0

# Google Sheets (optional)
google-api-python-client>=2.140.0
google-auth>=2.32.0
```

---

## Security Considerations

> [!CAUTION]
> These are critical for a production deployment.

| Area | Implementation |
|------|---------------|
| **Passwords** | bcrypt with 12 rounds, never store plaintext |
| **JWT** | Short-lived access tokens (30 min), long-lived refresh tokens (7 days) with DB revocation |
| **CORS** | Lock down to your frontend domain in production (no `*`) |
| **SQL Injection** | SQLAlchemy ORM parameterized queries throughout |
| **File Uploads** | Validate MIME types, enforce 10 MB limit, sanitize filenames |
| **Rate Limiting** | IP + token-based rate limits on auth and chat endpoints |
| **Input Validation** | Pydantic schemas validate every request body |
| **HTTPS** | Required in production (terminate at reverse proxy) |
| **Audit Trail** | All admin mutations logged to `audit_log` table |
| **Secrets** | `.env` file excluded from git, env vars in production |

---

## Key Design Decisions

> [!NOTE]
> Rationale for the major architectural choices.

1. **FastAPI over Express for the new backend** — Your existing chatbot is already in FastAPI/Python. Keeping the backend in Python gives you one language for all server logic, async support via `asyncpg`, and Pydantic for bulletproof validation.

2. **Keep Express `server.ts` as-is** — It serves the Vite frontend and handles the `/api/enquiries` form endpoint. Adding PostgreSQL logic here would create a dual-DB-driver setup. Instead, the Express enquiry route can optionally forward to FastAPI for DB persistence.

3. **UUID primary keys** — Unpredictable IDs are more secure than sequential integers for an education portal with sensitive student data.

4. **Pipeline stages as a separate template table** — Instead of embedding stages per student, we have a global template (`pipeline_stages`) and per-student progress (`student_stage_progress`). This means admins can modify the template, and each student's journey is tracked independently.

5. **Enquiry → Student conversion** — The enquiry flow (consultation form → Google Sheets) stays intact. A new admin endpoint lets counsellors "convert" a qualified enquiry into a full student account, seeding their profile from the enquiry data.

---

> [!TIP]
> Ready to proceed? I recommend starting with **Phase 1** (database setup and schema creation) and **Phase 2** (authentication). These two phases give you a functional login system backed by real PostgreSQL data, which unblocks everything else.
