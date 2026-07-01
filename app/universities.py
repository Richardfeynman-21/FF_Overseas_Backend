"""
Universities API Router.
Provides endpoints for browsing, filtering, and searching universities,
courses, and scholarships from the universities database.
"""

import logging
import math
from typing import Optional

from fastapi import APIRouter, HTTPException, Query

from app.db import get_pool

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api/universities", tags=["Universities"])

# ---------------------------------------------------------------------------
# Course type keyword mappings
# ---------------------------------------------------------------------------
COURSE_TYPE_KEYWORDS = {
    "STEM": [
        "science", "engineering", "computer", "technology", "math",
        "physics", "data", "cyber", "robotics", "computing",
    ],
    "Business": [
        "business", "management", "mba", "finance", "economics",
        "analytics", "marketing", "hospitality", "logistics",
    ],
    "Arts": [
        "art", "design", "writing", "journalism", "law",
        "criminology", "history", "architecture",
    ],
    "Medicine": [
        "medicine", "pharmacy", "health", "biology", "biomedical", "sports",
    ],
}

# ---------------------------------------------------------------------------
# Helper: parse qs_rank string to numeric SQL expression
# ---------------------------------------------------------------------------
QS_RANK_NUMERIC_EXPR = (
    "NULLIF(regexp_replace(ur.qs_rank_2026, '[^0-9].*', '', 'g'), '')::int"
)


# ---------------------------------------------------------------------------
# GET /api/universities/filters/options
# ---------------------------------------------------------------------------
@router.get("/filters/options")
async def get_filter_options():
    """Return available filter values with counts."""
    pool = get_pool()
    async with pool.acquire() as conn:
        country_rows = await conn.fetch(
            "SELECT country, COUNT(*) AS count FROM universities GROUP BY country ORDER BY country"
        )
        degree_rows = await conn.fetch(
            "SELECT degree_level, COUNT(DISTINCT university_id) AS count "
            "FROM courses GROUP BY degree_level ORDER BY degree_level"
        )

    return {
        "countries": [{"value": r["country"], "count": r["count"]} for r in country_rows],
        "degree_levels": [{"value": r["degree_level"], "count": r["count"]} for r in degree_rows],
    }


# ---------------------------------------------------------------------------
# GET /api/universities/courses/search  (autocomplete)
# ---------------------------------------------------------------------------
@router.get("/courses/search")
async def course_autocomplete(q: str = Query("", min_length=1, description="Search prefix")):
    """Return up to 20 distinct course names matching the query."""
    pool = get_pool()
    async with pool.acquire() as conn:
        rows = await conn.fetch(
            "SELECT DISTINCT course_name FROM courses "
            "WHERE course_name ILIKE $1 ORDER BY course_name LIMIT 20",
            f"%{q}%",
        )
    return [r["course_name"] for r in rows]


# ---------------------------------------------------------------------------
# GET /api/universities  (main listing)
# ---------------------------------------------------------------------------
@router.get("")
async def list_universities(
    search: Optional[str] = Query(None, description="Search by university name or course name"),
    countries: Optional[str] = Query(None, description="Comma-separated country names"),
    degree_levels: Optional[str] = Query(None, description="Comma-separated: Bachelor, Master, PhD"),
    course_search: Optional[str] = Query(None, description="Search specific course name"),
    course_types: Optional[str] = Query(None, description="Comma-separated: STEM, Business, Arts, Medicine"),
    fee_range: Optional[str] = Query(None, description="under18k, 18kto35k, 35kto55k, over55k"),
    min_ranking: Optional[str] = Query(None, description="top10, top50, top100, top500"),
    sort_by: Optional[str] = Query("rank", description="rank, courseCount, tuitionAsc"),
    page: int = Query(1, ge=1),
    page_size: int = Query(12, ge=1, le=100),
    featured: bool = Query(False, description="If true, return top 2 universities by QS rank from each country"),
):
    """
    Main universities listing with filters, search, and pagination.
    Builds a dynamic SQL query based on provided filter parameters.
    """
    pool = get_pool()

    if featured:
        # Special query returning top 2 ranked universities from each country
        sql = f"""
            WITH ranked_unis AS (
                SELECT
                    u.id,
                    u.name,
                    u.country,
                    u.alpha_two_code,
                    u.state_province,
                    u.web_pages,
                    ur.qs_rank_2026,
                    ur.national_rank,
                    ur.overall_score,
                    COUNT(DISTINCT c.id) AS course_count,
                    ROUND(AVG(CASE WHEN c.tuition_fee > 0 THEN c.tuition_fee END)::numeric, 2) AS avg_tuition_fee,
                    MODE() WITHIN GROUP (ORDER BY c.currency) AS currency,
                    COUNT(DISTINCT us.scholarship_id) AS scholarship_count,
                    {QS_RANK_NUMERIC_EXPR} AS rank_numeric,
                    (SELECT ARRAY(
                        SELECT DISTINCT c2.course_name FROM courses c2
                        WHERE c2.university_id = u.id
                        ORDER BY c2.course_name LIMIT 6
                    )) AS sample_programs,
                    (SELECT ARRAY(
                        SELECT DISTINCT c3.degree_level FROM courses c3
                        WHERE c3.university_id = u.id
                        ORDER BY c3.degree_level
                    )) AS degree_levels,
                    ROW_NUMBER() OVER (
                        PARTITION BY u.country 
                        ORDER BY {QS_RANK_NUMERIC_EXPR} ASC NULLS LAST
                    ) as country_rank
                FROM universities u
                LEFT JOIN university_rankings ur ON ur.university_id = u.id
                LEFT JOIN courses c ON c.university_id = u.id
                LEFT JOIN university_scholarships us ON us.university_id = u.id
                GROUP BY u.id, u.name, u.country, u.alpha_two_code, u.state_province, u.web_pages,
                         ur.qs_rank_2026, ur.national_rank, ur.overall_score
            )
            SELECT * FROM ranked_unis
            WHERE country_rank <= 2
            ORDER BY rank_numeric ASC NULLS LAST
        """
        try:
            async with pool.acquire() as conn:
                rows = await conn.fetch(sql)
        except Exception as e:
            logger.error(f"Error querying featured universities: {e}")
            raise HTTPException(status_code=500, detail="Failed to fetch featured universities.")
        
        universities = []
        for row in rows:
            universities.append({
                "id": row["id"],
                "name": row["name"],
                "country": row["country"],
                "alpha_two_code": row["alpha_two_code"],
                "state_province": row["state_province"],
                "web_pages": row["web_pages"] or [],
                "qs_rank_2026": row["qs_rank_2026"],
                "national_rank": row["national_rank"],
                "overall_score": float(row["overall_score"]) if row["overall_score"] is not None else None,
                "course_count": row["course_count"],
                "avg_tuition_fee": float(row["avg_tuition_fee"]) if row["avg_tuition_fee"] is not None else None,
                "currency": row["currency"],
                "scholarship_count": row["scholarship_count"],
                "sample_programs": row["sample_programs"] or [],
                "degree_levels": row["degree_levels"] or [],
            })
            
        return {
            "universities": universities,
            "total": len(universities),
            "page": 1,
            "page_size": len(universities),
            "total_pages": 1,
        }

    # ----- Build WHERE clauses and params dynamically -----
    where_clauses: list[str] = []
    params: list = []
    param_idx = 0  # tracks $1, $2, ...

    # Search filter – match university name OR any course name
    if search:
        param_idx += 1
        search_param = param_idx
        where_clauses.append(
            f"(u.name ILIKE ${search_param} OR EXISTS ("
            f"SELECT 1 FROM courses c_s WHERE c_s.university_id = u.id AND c_s.course_name ILIKE ${search_param}"
            f"))"
        )
        params.append(f"%{search}%")

    # Country filter
    if countries:
        country_list = [c.strip() for c in countries.split(",") if c.strip()]
        if country_list:
            placeholders = ", ".join(f"${param_idx + i + 1}" for i in range(len(country_list)))
            where_clauses.append(f"u.country IN ({placeholders})")
            params.extend(country_list)
            param_idx += len(country_list)

    # Degree-level filter (university must have at least one course at that level)
    if degree_levels:
        dl_list = [d.strip() for d in degree_levels.split(",") if d.strip()]
        if dl_list:
            placeholders = ", ".join(f"${param_idx + i + 1}" for i in range(len(dl_list)))
            where_clauses.append(
                f"EXISTS (SELECT 1 FROM courses c_dl WHERE c_dl.university_id = u.id AND c_dl.degree_level IN ({placeholders}))"
            )
            params.extend(dl_list)
            param_idx += len(dl_list)

    # Course-name search
    if course_search:
        param_idx += 1
        where_clauses.append(
            f"EXISTS (SELECT 1 FROM courses c_cn WHERE c_cn.university_id = u.id AND c_cn.course_name ILIKE ${param_idx})"
        )
        params.append(f"%{course_search}%")

    # Course-type filter (keyword-based ILIKE)
    if course_types:
        ct_list = [ct.strip() for ct in course_types.split(",") if ct.strip()]
        all_keywords: list[str] = []
        for ct in ct_list:
            kws = COURSE_TYPE_KEYWORDS.get(ct, [])
            all_keywords.extend(kws)
        if all_keywords:
            ilike_parts = []
            for kw in all_keywords:
                param_idx += 1
                ilike_parts.append(f"c_ct.course_name ILIKE ${param_idx}")
                params.append(f"%{kw}%")
            where_clauses.append(
                f"EXISTS (SELECT 1 FROM courses c_ct WHERE c_ct.university_id = u.id AND ({' OR '.join(ilike_parts)}))"
            )

    # Ranking filter
    if min_ranking:
        rank_map = {"top10": 10, "top50": 50, "top100": 100, "top500": 500}
        rank_val = rank_map.get(min_ranking)
        if rank_val:
            param_idx += 1
            where_clauses.append(
                f"{QS_RANK_NUMERIC_EXPR} <= ${param_idx}"
            )
            params.append(rank_val)

    where_sql = " AND ".join(where_clauses) if where_clauses else "TRUE"

    # ----- Fee range filter (HAVING on avg_tuition) -----
    having_clauses: list[str] = []
    if fee_range:
        fee_map = {
            "under18k": (None, 18000),
            "18kto35k": (18000, 35000),
            "35kto55k": (35000, 55000),
            "over55k": (55000, None),
        }
        bounds = fee_map.get(fee_range)
        if bounds:
            low, high = bounds
            if low is not None:
                param_idx += 1
                having_clauses.append(f"AVG(CASE WHEN c.tuition_fee > 0 THEN c.tuition_fee END) >= ${param_idx}")
                params.append(low)
            if high is not None:
                param_idx += 1
                having_clauses.append(f"AVG(CASE WHEN c.tuition_fee > 0 THEN c.tuition_fee END) < ${param_idx}")
                params.append(high)

    having_sql = (" AND " + " AND ".join(having_clauses)) if having_clauses else ""

    # ----- Sort order -----
    if sort_by == "courseCount":
        order_sql = "course_count DESC NULLS LAST"
    elif sort_by == "tuitionAsc":
        order_sql = "avg_tuition_fee ASC NULLS LAST"
    else:  # default: rank
        order_sql = "rank_numeric ASC NULLS LAST"

    # ----- Pagination params -----
    param_idx += 1
    limit_param = param_idx
    params.append(page_size)

    param_idx += 1
    offset_param = param_idx
    offset = (page - 1) * page_size
    params.append(offset)

    # ----- Count query -----
    count_sql = f"""
        SELECT COUNT(*) FROM (
            SELECT u.id
            FROM universities u
            LEFT JOIN university_rankings ur ON ur.university_id = u.id
            LEFT JOIN courses c ON c.university_id = u.id
            LEFT JOIN university_scholarships us ON us.university_id = u.id
            WHERE {where_sql}
            GROUP BY u.id, ur.qs_rank_2026
            HAVING TRUE {having_sql}
        ) sub
    """

    # ----- Main data query -----
    data_sql = f"""
        SELECT
            u.id,
            u.name,
            u.country,
            u.alpha_two_code,
            u.state_province,
            u.web_pages,
            ur.qs_rank_2026,
            ur.national_rank,
            ur.overall_score,
            COUNT(DISTINCT c.id) AS course_count,
            ROUND(AVG(CASE WHEN c.tuition_fee > 0 THEN c.tuition_fee END)::numeric, 2) AS avg_tuition_fee,
            MODE() WITHIN GROUP (ORDER BY c.currency) AS currency,
            COUNT(DISTINCT us.scholarship_id) AS scholarship_count,
            {QS_RANK_NUMERIC_EXPR} AS rank_numeric,
            (SELECT ARRAY(
                SELECT DISTINCT c2.course_name FROM courses c2
                WHERE c2.university_id = u.id
                ORDER BY c2.course_name LIMIT 6
            )) AS sample_programs,
            (SELECT ARRAY(
                SELECT DISTINCT c3.degree_level FROM courses c3
                WHERE c3.university_id = u.id
                ORDER BY c3.degree_level
            )) AS degree_levels
        FROM universities u
        LEFT JOIN university_rankings ur ON ur.university_id = u.id
        LEFT JOIN courses c ON c.university_id = u.id
        LEFT JOIN university_scholarships us ON us.university_id = u.id
        WHERE {where_sql}
        GROUP BY u.id, u.name, u.country, u.alpha_two_code, u.state_province, u.web_pages,
                 ur.qs_rank_2026, ur.national_rank, ur.overall_score
        HAVING TRUE {having_sql}
        ORDER BY {order_sql}
        LIMIT ${limit_param} OFFSET ${offset_param}
    """

    try:
        async with pool.acquire() as conn:
            total = await conn.fetchval(count_sql, *params[: param_idx - 2])  # without limit/offset
            rows = await conn.fetch(data_sql, *params)
    except Exception as e:
        logger.error(f"Error querying universities: {e}")
        raise HTTPException(status_code=500, detail="Failed to fetch universities.")

    universities = []
    for row in rows:
        universities.append({
            "id": row["id"],
            "name": row["name"],
            "country": row["country"],
            "alpha_two_code": row["alpha_two_code"],
            "state_province": row["state_province"],
            "web_pages": row["web_pages"] or [],
            "qs_rank_2026": row["qs_rank_2026"],
            "national_rank": row["national_rank"],
            "overall_score": float(row["overall_score"]) if row["overall_score"] is not None else None,
            "course_count": row["course_count"],
            "avg_tuition_fee": float(row["avg_tuition_fee"]) if row["avg_tuition_fee"] is not None else None,
            "currency": row["currency"],
            "scholarship_count": row["scholarship_count"],
            "sample_programs": row["sample_programs"] or [],
            "degree_levels": row["degree_levels"] or [],
        })

    total_pages = math.ceil(total / page_size) if total else 0

    return {
        "universities": universities,
        "total": total,
        "page": page,
        "page_size": page_size,
        "total_pages": total_pages,
    }


# ---------------------------------------------------------------------------
# GET /api/universities/{university_id}  (detail)
# ---------------------------------------------------------------------------
@router.get("/{university_id}")
async def get_university_detail(university_id: int):
    """Return full detail for a single university including rankings, courses, and scholarships."""
    pool = get_pool()

    async with pool.acquire() as conn:
        # University + rankings
        uni = await conn.fetchrow(
            """
            SELECT
                u.id, u.name, u.country, u.alpha_two_code, u.state_province,
                u.domains, u.web_pages, u.created_at,
                ur.qs_rank_2026, ur.qs_rank_2025, ur.national_rank,
                ur.academic_reputation_score, ur.employer_reputation_score,
                ur.faculty_student_score, ur.citations_per_faculty_score,
                ur.international_faculty_score, ur.international_students_score,
                ur.international_research_network_score, ur.employment_outcomes_score,
                ur.sustainability_score, ur.overall_score
            FROM universities u
            LEFT JOIN university_rankings ur ON ur.university_id = u.id
            WHERE u.id = $1
            """,
            university_id,
        )

        if not uni:
            raise HTTPException(status_code=404, detail="University not found.")

        # First 20 courses
        courses = await conn.fetch(
            """
            SELECT id, course_name, degree_level, duration_years, language, tuition_fee, currency
            FROM courses WHERE university_id = $1
            ORDER BY degree_level, course_name LIMIT 20
            """,
            university_id,
        )

        # Scholarships via junction table
        scholarships = await conn.fetch(
            """
            SELECT s.id, s.name, s.type, s.amount, s.coverage, s.eligibility,
                   s.target_degree_level, s.country
            FROM scholarships s
            JOIN university_scholarships us ON us.scholarship_id = s.id
            WHERE us.university_id = $1
            ORDER BY s.name
            """,
            university_id,
        )

    def safe_float(val):
        return float(val) if val is not None else None

    return {
        "university": {
            "id": uni["id"],
            "name": uni["name"],
            "country": uni["country"],
            "alpha_two_code": uni["alpha_two_code"],
            "state_province": uni["state_province"],
            "domains": uni["domains"] or [],
            "web_pages": uni["web_pages"] or [],
            "created_at": uni["created_at"].isoformat() if uni["created_at"] else None,
        },
        "rankings": {
            "qs_rank_2026": uni["qs_rank_2026"],
            "qs_rank_2025": uni["qs_rank_2025"],
            "national_rank": uni["national_rank"],
            "academic_reputation_score": safe_float(uni["academic_reputation_score"]),
            "employer_reputation_score": safe_float(uni["employer_reputation_score"]),
            "faculty_student_score": safe_float(uni["faculty_student_score"]),
            "citations_per_faculty_score": safe_float(uni["citations_per_faculty_score"]),
            "international_faculty_score": safe_float(uni["international_faculty_score"]),
            "international_students_score": safe_float(uni["international_students_score"]),
            "international_research_network_score": safe_float(uni["international_research_network_score"]),
            "employment_outcomes_score": safe_float(uni["employment_outcomes_score"]),
            "sustainability_score": safe_float(uni["sustainability_score"]),
            "overall_score": safe_float(uni["overall_score"]),
        },
        "courses": [
            {
                "id": c["id"],
                "course_name": c["course_name"],
                "degree_level": c["degree_level"],
                "duration_years": c["duration_years"],
                "language": c["language"],
                "tuition_fee": float(c["tuition_fee"]) if c["tuition_fee"] is not None else None,
                "currency": c["currency"],
            }
            for c in courses
        ],
        "scholarships": [
            {
                "id": s["id"],
                "name": s["name"],
                "type": s["type"],
                "amount": s["amount"],
                "coverage": s["coverage"],
                "eligibility": s["eligibility"],
                "target_degree_level": s["target_degree_level"],
                "country": s["country"],
            }
            for s in scholarships
        ],
    }


# ---------------------------------------------------------------------------
# GET /api/universities/{university_id}/courses
# ---------------------------------------------------------------------------
@router.get("/{university_id}/courses")
async def get_university_courses(
    university_id: int,
    degree_level: Optional[str] = Query(None, description="Filter by degree level"),
    search: Optional[str] = Query(None, description="Search course name"),
    page: int = Query(1, ge=1),
    page_size: int = Query(20, ge=1, le=100),
):
    """Return paginated courses for a specific university."""
    pool = get_pool()

    where_clauses = ["university_id = $1"]
    params: list = [university_id]
    param_idx = 1

    if degree_level:
        param_idx += 1
        where_clauses.append(f"degree_level = ${param_idx}")
        params.append(degree_level)

    if search:
        param_idx += 1
        where_clauses.append(f"course_name ILIKE ${param_idx}")
        params.append(f"%{search}%")

    where_sql = " AND ".join(where_clauses)

    param_idx += 1
    limit_param = param_idx
    params.append(page_size)

    param_idx += 1
    offset_param = param_idx
    params.append((page - 1) * page_size)

    async with pool.acquire() as conn:
        # Verify university exists
        exists = await conn.fetchval("SELECT 1 FROM universities WHERE id = $1", university_id)
        if not exists:
            raise HTTPException(status_code=404, detail="University not found.")

        total = await conn.fetchval(
            f"SELECT COUNT(*) FROM courses WHERE {where_sql}",
            *params[:param_idx - 2],  # without limit/offset
        )

        rows = await conn.fetch(
            f"""
            SELECT id, course_name, degree_level, duration_years, language, tuition_fee, currency
            FROM courses
            WHERE {where_sql}
            ORDER BY degree_level, course_name
            LIMIT ${limit_param} OFFSET ${offset_param}
            """,
            *params,
        )

    return {
        "courses": [
            {
                "id": r["id"],
                "course_name": r["course_name"],
                "degree_level": r["degree_level"],
                "duration_years": r["duration_years"],
                "language": r["language"],
                "tuition_fee": float(r["tuition_fee"]) if r["tuition_fee"] is not None else None,
                "currency": r["currency"],
            }
            for r in rows
        ],
        "total": total,
        "page": page,
        "page_size": page_size,
        "total_pages": math.ceil(total / page_size) if total else 0,
    }


# ---------------------------------------------------------------------------
# GET /api/universities/{university_id}/scholarships
# ---------------------------------------------------------------------------
@router.get("/{university_id}/scholarships")
async def get_university_scholarships(university_id: int):
    """Return scholarships linked to a specific university."""
    pool = get_pool()

    async with pool.acquire() as conn:
        exists = await conn.fetchval("SELECT 1 FROM universities WHERE id = $1", university_id)
        if not exists:
            raise HTTPException(status_code=404, detail="University not found.")

        rows = await conn.fetch(
            """
            SELECT s.id, s.name, s.type, s.amount, s.coverage, s.eligibility,
                   s.target_degree_level, s.country
            FROM scholarships s
            JOIN university_scholarships us ON us.scholarship_id = s.id
            WHERE us.university_id = $1
            ORDER BY s.name
            """,
            university_id,
        )

    return {
        "scholarships": [
            {
                "id": r["id"],
                "name": r["name"],
                "type": r["type"],
                "amount": r["amount"],
                "coverage": r["coverage"],
                "eligibility": r["eligibility"],
                "target_degree_level": r["target_degree_level"],
                "country": r["country"],
            }
            for r in rows
        ],
        "total": len(rows),
    }
