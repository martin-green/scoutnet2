//! Närvaro: terminsplanering av möten, närvaroregistrering och statistik (ersätter Skojjt).

use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, put},
};
use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sn_domain::{BodyKind, Term, TroopType};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::routes::{BodyRef, audit, bodies::load_visible_body};
use crate::{
    AppState,
    auth::CurrentUser,
    authz::Permission,
    error::{ApiError, ApiResult},
};

const STATUSES: [&str; 3] = ["present", "absent", "excused"];
const KINDS: [&str; 4] = ["meeting", "hike", "camp", "activity"];
const MAX_SERIES: usize = 60;

/// Gemensam SELECT för möten. Tider räknas om till svensk lokal tid i databasen.
macro_rules! meeting_select {
    () => {
        "SELECT m.id, m.troop_id, b.name AS troop_name, b.kind AS troop_kind, b.number AS troop_number,
                b.path::text AS troop_path, m.name, m.kind, m.starts_at,
                (m.starts_at AT TIME ZONE 'Europe/Stockholm')::date AS local_date,
                to_char(m.starts_at AT TIME ZONE 'Europe/Stockholm', 'HH24:MI') AS local_time,
                m.duration_min, m.location, m.locked, m.starts_at < now() AS is_past,
                coalesce(a.present, 0) AS present, coalesce(a.absent, 0) AS absent, coalesce(a.excused, 0) AS excused
         FROM meeting m JOIN body b ON b.id = m.troop_id
         LEFT JOIN LATERAL (
             SELECT count(*) FILTER (WHERE status = 'present') AS present,
                    count(*) FILTER (WHERE status = 'absent') AS absent,
                    count(*) FILTER (WHERE status = 'excused') AS excused
             FROM attendance WHERE meeting_id = m.id
         ) a ON true"
    };
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MeetingDto {
    pub id: Uuid,
    pub troop: BodyRef,
    pub name: String,
    /// `meeting`, `hike`, `camp`, `activity`.
    pub kind: String,
    pub starts_at: DateTime<Utc>,
    /// Lokalt datum (Europe/Stockholm).
    pub date: NaiveDate,
    /// Lokal starttid `HH:MM`.
    pub time: String,
    pub duration_min: i32,
    pub location: Option<String>,
    /// Rapporterat (t.ex. i DAK) och därför låst för ändringar.
    pub locked: bool,
    pub is_past: bool,
    pub present: i64,
    pub absent: i64,
    pub excused: i64,
}

#[derive(sqlx::FromRow)]
struct MeetingRow {
    id: Uuid,
    troop_id: Uuid,
    troop_name: String,
    troop_kind: String,
    troop_number: i32,
    troop_path: String,
    name: String,
    kind: String,
    starts_at: DateTime<Utc>,
    local_date: NaiveDate,
    local_time: String,
    duration_min: i32,
    location: Option<String>,
    locked: bool,
    is_past: bool,
    present: i64,
    absent: i64,
    excused: i64,
}

impl From<MeetingRow> for MeetingDto {
    fn from(r: MeetingRow) -> Self {
        Self {
            id: r.id,
            troop: BodyRef { id: r.troop_id, name: r.troop_name, kind: r.troop_kind, number: r.troop_number },
            name: r.name,
            kind: r.kind,
            starts_at: r.starts_at,
            date: r.local_date,
            time: r.local_time,
            duration_min: r.duration_min,
            location: r.location,
            locked: r.locked,
            is_past: r.is_past,
            present: r.present,
            absent: r.absent,
            excused: r.excused,
        }
    }
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TermQuery {
    /// Termin, t.ex. `HT2026`, `VT 2027` eller termins-id `20261`. Standard: innevarande termin.
    pub term: Option<String>,
}

pub fn parse_term(term: Option<&str>) -> ApiResult<Term> {
    match term.map(str::trim).filter(|t| !t.is_empty()) {
        None => Ok(Term::containing(chrono::Local::now().date_naive())),
        Some(t) if t.chars().all(|c| c.is_ascii_digit()) => {
            t.parse::<i32>().map(Term::from_id).map_err(|_| ApiError::BadRequest(format!("ogiltig termin: {t}")))
        }
        Some(t) => t.parse().map_err(|e: sn_domain::TermParseError| ApiError::BadRequest(e.to_string())),
    }
}

/// Avdelningen måste vara en avdelning (möten hör till avdelningar).
async fn load_troop(state: &AppState, user: &CurrentUser, key: &str, perm: Permission) -> ApiResult<(Uuid, String)> {
    let (id, kind, path) = load_visible_body(state, user, key, perm).await?;
    if kind != BodyKind::Troop.as_str() {
        return Err(ApiError::BadRequest("möten och närvaro hör till en avdelning".into()));
    }
    Ok((id, path))
}

async fn load_meeting(state: &AppState, user: &CurrentUser, id: Uuid, perm: Permission) -> ApiResult<MeetingRow> {
    let row = sqlx::query_as::<_, MeetingRow>(concat!(meeting_select!(), " WHERE m.id = $1"))
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    // Den som inte får läsa ser inte att mötet finns. Den som får läsa men inte skriva får 403.
    if !user.0.can(Permission::AttendanceRead, &row.troop_path) && !user.0.can(perm, &row.troop_path) {
        return Err(ApiError::NotFound);
    }
    // Möten nås via sitt id och inte via enheten, så modulspärren kontrolleras här.
    crate::modules::require(state, super::ID, &row.troop_path).await?;
    if !user.0.can(perm, &row.troop_path) {
        return Err(ApiError::Forbidden);
    }
    Ok(row)
}

// ---------------------------------------------------------------------------------------------
// Översikt per avdelning

#[derive(Debug, Serialize, ToSchema)]
pub struct TroopAttendanceSummary {
    pub troop: BodyRef,
    pub troop_type: Option<String>,
    pub troop_type_label: Option<String>,
    /// Placerade i avdelningen i dag (scouter och ledare).
    pub members: i64,
    pub planned: i64,
    pub held: i64,
    /// Genomförda möten där ingen närvaro har registrerats.
    pub missing_attendance: i64,
    /// Andel närvarande av registrerade (0–1).
    pub attendance_pct: Option<f64>,
    pub next_meeting: Option<DateTime<Utc>>,
    pub last_meeting: Option<DateTime<Utc>>,
    pub next_meeting_id: Option<Uuid>,
    pub last_meeting_id: Option<Uuid>,
    /// Registrerade på senaste mötet (0 = närvaro saknas).
    pub last_meeting_recorded: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TermAttendanceSummary {
    pub term: String,
    pub term_id: i32,
    pub troops: Vec<TroopAttendanceSummary>,
}

#[derive(sqlx::FromRow)]
struct SummaryRow {
    id: Uuid,
    kind: String,
    number: i32,
    name: String,
    troop_type: Option<String>,
    members: i64,
    planned: i64,
    held: i64,
    missing: i64,
    present: Option<i64>,
    recorded: Option<i64>,
    next_at: Option<DateTime<Utc>>,
    last_at: Option<DateTime<Utc>>,
    next_id: Option<Uuid>,
    last_id: Option<Uuid>,
    last_recorded: Option<i64>,
}

/// Närvaro per avdelning under en enhet (kår, distrikt …) för en termin.
#[utoipa::path(get, path = "/v1/bodies/{id}/attendance/summary", tag = "attendance",
    params(("id" = String, Path, description = "Enhetens id eller nyckel, t.ex. `group-1001`"), TermQuery),
    responses((status = 200, body = TermAttendanceSummary), (status = 404, body = crate::error::Problem)))]
pub async fn attendance_summary(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
    Query(q): Query<TermQuery>,
) -> ApiResult<Json<TermAttendanceSummary>> {
    let term = parse_term(q.term.as_deref())?;
    let (root, ..) = load_visible_body(&state, &user, &key, Permission::AttendanceRead).await?;
    let rows = sqlx::query_as::<_, SummaryRow>(
        "WITH root AS (SELECT path FROM body WHERE id = $1),
         troops AS (SELECT b.* FROM body b, root r WHERE b.path <@ r.path AND b.kind = 'troop' AND b.status = 'active'),
         ms AS (SELECT m.* FROM meeting m JOIN troops t ON t.id = m.troop_id
                WHERE (m.starts_at AT TIME ZONE 'Europe/Stockholm')::date BETWEEN $2 AND $3),
         att AS (SELECT a.meeting_id, count(*) FILTER (WHERE a.status = 'present') AS present, count(*) AS recorded
                 FROM attendance a JOIN ms ON ms.id = a.meeting_id GROUP BY 1)
         SELECT t.id, t.kind, t.number, t.name, t.troop_type,
                (SELECT count(DISTINCT pl.person_id) FROM placement pl JOIN body pb ON pb.id = pl.body_id
                 WHERE pb.path <@ t.path AND upper_inf(pl.valid)) AS members,
                count(ms.id) AS planned,
                count(ms.id) FILTER (WHERE ms.starts_at < now()) AS held,
                count(ms.id) FILTER (WHERE ms.starts_at < now() AND att.meeting_id IS NULL) AS missing,
                sum(att.present)::int8 AS present, sum(att.recorded)::int8 AS recorded,
                min(ms.starts_at) FILTER (WHERE ms.starts_at >= now()) AS next_at,
                max(ms.starts_at) FILTER (WHERE ms.starts_at < now()) AS last_at,
                (array_agg(ms.id ORDER BY ms.starts_at) FILTER (WHERE ms.starts_at >= now()))[1] AS next_id,
                (array_agg(ms.id ORDER BY ms.starts_at DESC) FILTER (WHERE ms.starts_at < now()))[1] AS last_id,
                (array_agg(coalesce(att.recorded, 0) ORDER BY ms.starts_at DESC) FILTER (WHERE ms.starts_at < now()))[1] AS last_recorded
         FROM troops t LEFT JOIN ms ON ms.troop_id = t.id LEFT JOIN att ON att.meeting_id = ms.id
         GROUP BY t.id, t.kind, t.number, t.name, t.troop_type, t.path
         ORDER BY array_position(ARRAY['familj','baver','sparare','upptackare','aventyrare','utmanare','rover','annan'], t.troop_type), t.name",
    )
    .bind(root)
    .bind(term.start())
    .bind(term.end())
    .fetch_all(&state.db)
    .await?;

    #[expect(clippy::cast_precision_loss, reason = "närvaroantal är små tal")]
    let troops = rows
        .into_iter()
        .map(|r| TroopAttendanceSummary {
            troop_type_label: r
                .troop_type
                .as_deref()
                .and_then(|t| t.parse::<TroopType>().ok())
                .map(|t| t.label_sv().into()),
            troop_type: r.troop_type,
            troop: BodyRef { id: r.id, name: r.name, kind: r.kind, number: r.number },
            members: r.members,
            planned: r.planned,
            held: r.held,
            missing_attendance: r.missing,
            attendance_pct: match (r.present, r.recorded) {
                (Some(p), Some(n)) if n > 0 => Some(p as f64 / n as f64),
                _ => None,
            },
            next_meeting: r.next_at,
            last_meeting: r.last_at,
            next_meeting_id: r.next_id,
            last_meeting_id: r.last_id,
            last_meeting_recorded: r.last_recorded.unwrap_or(0),
        })
        .collect();
    Ok(Json(TermAttendanceSummary { term: term.to_string(), term_id: term.id(), troops }))
}

// ---------------------------------------------------------------------------------------------
// Närvarolista (matris) för en avdelning och termin

#[derive(Debug, Serialize, ToSchema)]
pub struct AttendanceMatrix {
    pub term: String,
    pub term_id: i32,
    pub troop: BodyRef,
    pub meetings: Vec<MeetingDto>,
    pub people: Vec<AttendanceRow>,
    /// Om den inloggade får planera möten och registrera närvaro.
    pub can_write: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AttendanceRow {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub age: i32,
    pub is_leader: bool,
    pub patrol: Option<String>,
    /// En status per möte i `meetings` (samma ordning): `present`, `absent`, `excused` eller `null`.
    pub statuses: Vec<Option<String>>,
    pub present: i64,
    /// Genomförda möten med registrerad närvaro för personen.
    pub recorded: i64,
    pub hikes: i64,
}

#[derive(sqlx::FromRow)]
struct PersonRow {
    id: Uuid,
    first_name: String,
    last_name: String,
    age: i32,
    is_leader: bool,
    patrol: Option<String>,
}

/// Närvarolistan för en avdelning: terminens möten som kolumner och deltagare som rader.
#[utoipa::path(get, path = "/v1/bodies/{id}/attendance", tag = "attendance",
    params(("id" = String, Path, description = "Avdelningens id eller nyckel, t.ex. `troop-5003`"), TermQuery),
    responses((status = 200, body = AttendanceMatrix), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn attendance_matrix(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
    Query(q): Query<TermQuery>,
) -> ApiResult<Json<AttendanceMatrix>> {
    let term = parse_term(q.term.as_deref())?;
    let (troop_id, path) = load_troop(&state, &user, &key, Permission::AttendanceRead).await?;
    let db = &state.db;

    let (troop, meetings, people, marks) = tokio::try_join!(
        sqlx::query_as::<_, (Uuid, String, String, i32)>("SELECT id, name, kind, number FROM body WHERE id = $1")
            .bind(troop_id)
            .fetch_one(db),
        sqlx::query_as::<_, MeetingRow>(concat!(
            meeting_select!(),
            " WHERE m.troop_id = $1 AND (m.starts_at AT TIME ZONE 'Europe/Stockholm')::date BETWEEN $2 AND $3
              ORDER BY m.starts_at"
        ))
        .bind(troop_id)
        .bind(term.start())
        .bind(term.end())
        .fetch_all(db),
        // Deltagare: placerade i avdelningen (eller dess patruller) någon gång under terminen, och alla
        // som har registrerad närvaro på terminens möten.
        sqlx::query_as::<_, PersonRow>(
            "WITH t AS (SELECT path FROM body WHERE id = $1),
             placed AS (
                 SELECT pl.person_id, bool_or(pl.is_leader) AS is_leader,
                        max(pb.name) FILTER (WHERE pb.kind = 'patrol') AS patrol
                 FROM placement pl JOIN body pb ON pb.id = pl.body_id, t
                 WHERE pb.path <@ t.path AND pl.valid && daterange($2, $3, '[]')
                 GROUP BY pl.person_id),
             attended AS (
                 SELECT a.person_id, bool_or(a.is_leader) AS is_leader FROM attendance a JOIN meeting m ON m.id = a.meeting_id
                 WHERE m.troop_id = $1 AND (m.starts_at AT TIME ZONE 'Europe/Stockholm')::date BETWEEN $2 AND $3
                 GROUP BY a.person_id)
             SELECT p.id, p.first_name, p.last_name, date_part('year', age(p.birth_date))::int AS age,
                    coalesce(pl.is_leader, ad.is_leader, false) AS is_leader, pl.patrol
             FROM person p LEFT JOIN placed pl ON pl.person_id = p.id LEFT JOIN attended ad ON ad.person_id = p.id
             WHERE pl.person_id IS NOT NULL OR ad.person_id IS NOT NULL
             ORDER BY coalesce(pl.is_leader, ad.is_leader, false), pl.patrol NULLS LAST, p.first_name, p.last_name",
        )
        .bind(troop_id)
        .bind(term.start())
        .bind(term.end())
        .fetch_all(db),
        sqlx::query_as::<_, (Uuid, Uuid, String)>(
            "SELECT a.meeting_id, a.person_id, a.status FROM attendance a JOIN meeting m ON m.id = a.meeting_id
             WHERE m.troop_id = $1 AND (m.starts_at AT TIME ZONE 'Europe/Stockholm')::date BETWEEN $2 AND $3",
        )
        .bind(troop_id)
        .bind(term.start())
        .bind(term.end())
        .fetch_all(db),
    )?;

    let column: HashMap<Uuid, usize> = meetings.iter().enumerate().map(|(i, m)| (m.id, i)).collect();
    let hike: Vec<bool> = meetings.iter().map(|m| m.kind != "meeting").collect();
    let mut grid: HashMap<Uuid, Vec<Option<String>>> = HashMap::new();
    for (meeting, person, status) in marks {
        if let Some(&i) = column.get(&meeting) {
            grid.entry(person).or_insert_with(|| vec![None; meetings.len()])[i] = Some(status);
        }
    }
    let people = people
        .into_iter()
        .map(|p| {
            let statuses = grid.remove(&p.id).unwrap_or_else(|| vec![None; meetings.len()]);
            let present = statuses.iter().filter(|s| s.as_deref() == Some("present")).count();
            let hikes = statuses.iter().zip(&hike).filter(|(s, h)| **h && s.as_deref() == Some("present")).count();
            AttendanceRow {
                recorded: i64::try_from(statuses.iter().flatten().count()).unwrap_or(i64::MAX),
                present: i64::try_from(present).unwrap_or(i64::MAX),
                hikes: i64::try_from(hikes).unwrap_or(i64::MAX),
                statuses,
                id: p.id,
                first_name: p.first_name,
                last_name: p.last_name,
                age: p.age,
                is_leader: p.is_leader,
                patrol: p.patrol,
            }
        })
        .collect();

    Ok(Json(AttendanceMatrix {
        term: term.to_string(),
        term_id: term.id(),
        troop: BodyRef { id: troop.0, name: troop.1, kind: troop.2, number: troop.3 },
        meetings: meetings.into_iter().map(MeetingDto::from).collect(),
        people,
        can_write: user.0.can(Permission::AttendanceWrite, &path),
    }))
}

// ---------------------------------------------------------------------------------------------
// Planera möten

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMeetings {
    /// Namn. För en serie läggs veckonumret till, t.ex. "Möte v.43".
    pub name: String,
    /// Första mötets datum.
    pub date: NaiveDate,
    /// Starttid `HH:MM` (svensk tid).
    pub time: String,
    pub duration_min: i32,
    pub location: Option<String>,
    /// `meeting` (standard), `hike`, `camp`, `activity`.
    pub kind: Option<String>,
    /// Upprepa varje vecka till och med detta datum.
    pub repeat_until: Option<NaiveDate>,
    /// Datum i serien som hoppas över (lov, helgdagar).
    #[serde(default)]
    pub skip_dates: Vec<NaiveDate>,
}

fn parse_time(t: &str) -> ApiResult<NaiveTime> {
    NaiveTime::parse_from_str(t.trim(), "%H:%M").map_err(|_| ApiError::BadRequest(format!("ogiltig tid: {t} (HH:MM)")))
}

fn validate_kind(kind: &str) -> ApiResult<()> {
    if KINDS.contains(&kind) { Ok(()) } else { Err(ApiError::BadRequest(format!("ogiltig mötestyp: {kind}"))) }
}

fn validate_duration(min: i32) -> ApiResult<()> {
    if (5..=14 * 24 * 60).contains(&min) {
        Ok(())
    } else {
        Err(ApiError::BadRequest("längden ska vara mellan 5 minuter och 14 dygn".into()))
    }
}

/// Datumen i en veckoserie, utan överhoppade datum.
fn series_dates(first: NaiveDate, until: Option<NaiveDate>, skip: &[NaiveDate]) -> ApiResult<Vec<NaiveDate>> {
    let until = until.unwrap_or(first);
    if until < first {
        return Err(ApiError::BadRequest("slutdatum är före första datum".into()));
    }
    let dates: Vec<NaiveDate> =
        first.iter_weeks().take_while(|d| *d <= until).filter(|d| !skip.contains(d)).take(MAX_SERIES + 1).collect();
    if dates.len() > MAX_SERIES {
        return Err(ApiError::BadRequest(format!("högst {MAX_SERIES} möten per serie")));
    }
    if dates.is_empty() {
        return Err(ApiError::BadRequest("alla datum hoppades över".into()));
    }
    Ok(dates)
}

/// Planerar ett möte eller en veckoserie för avdelningen. Kräver `attendance_write`.
#[utoipa::path(post, path = "/v1/bodies/{id}/meetings", tag = "attendance",
    params(("id" = String, Path, description = "Avdelningens id eller nyckel, t.ex. `troop-5003`")),
    request_body = CreateMeetings,
    responses((status = 201, body = [MeetingDto]), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn create_meetings(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
    Json(req): Json<CreateMeetings>,
) -> ApiResult<(StatusCode, Json<Vec<MeetingDto>>)> {
    let (troop_id, path) = load_troop(&state, &user, &key, Permission::AttendanceRead).await?;
    if !user.0.can(Permission::AttendanceWrite, &path) {
        return Err(ApiError::Forbidden);
    }
    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(ApiError::BadRequest("namnet ska vara 1–100 tecken".into()));
    }
    let kind = req.kind.as_deref().unwrap_or("meeting");
    validate_kind(kind)?;
    validate_duration(req.duration_min)?;
    let time = parse_time(&req.time)?;
    let dates = series_dates(req.date, req.repeat_until, &req.skip_dates)?;
    let series = dates.len() > 1;
    let location = req.location.as_deref().map(str::trim).filter(|l| !l.is_empty());

    let mut tx = state.db.begin().await?;
    let mut ids = Vec::with_capacity(dates.len());
    for date in &dates {
        let label = if series { format!("{name} v.{}", date.iso_week().week()) } else { name.to_owned() };
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO meeting (troop_id, starts_at, duration_min, name, location, kind)
             VALUES ($1, ($2::date + $3::time) AT TIME ZONE 'Europe/Stockholm', $4, $5, $6, $7) RETURNING id",
        )
        .bind(troop_id)
        .bind(date)
        .bind(time)
        .bind(req.duration_min)
        .bind(label)
        .bind(location)
        .bind(kind)
        .fetch_one(&mut *tx)
        .await?;
        ids.push(id);
    }
    tx.commit().await?;
    audit(
        &state.db,
        user.0.account_id,
        "meeting.create",
        "body",
        troop_id,
        serde_json::json!({ "count": ids.len(), "first": dates.first(), "last": dates.last() }),
    )
    .await;

    let rows =
        sqlx::query_as::<_, MeetingRow>(concat!(meeting_select!(), " WHERE m.id = ANY($1) ORDER BY m.starts_at"))
            .bind(&ids)
            .fetch_all(&state.db)
            .await?;
    Ok((StatusCode::CREATED, Json(rows.into_iter().map(MeetingDto::from).collect())))
}

// ---------------------------------------------------------------------------------------------
// Ett möte: deltagarlista och registrering

#[derive(Debug, Serialize, ToSchema)]
pub struct MeetingDetail {
    pub meeting: MeetingDto,
    pub roster: Vec<RosterEntry>,
    pub can_write: bool,
    /// Föregående och nästa möte i samma avdelning, för bläddring.
    pub previous_id: Option<Uuid>,
    pub next_id: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct RosterEntry {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub age: i32,
    pub is_leader: bool,
    pub patrol: Option<String>,
    /// `present`, `absent`, `excused` eller `null` (inte registrerad).
    pub status: Option<String>,
    /// Placerad i avdelningen på mötesdagen. `false` för gäster och personer som har flyttat.
    pub placed: bool,
}

async fn roster(db: &sqlx::PgPool, meeting_id: Uuid) -> ApiResult<Vec<RosterEntry>> {
    Ok(sqlx::query_as::<_, RosterEntry>(
        "WITH m AS (SELECT id, troop_id, (starts_at AT TIME ZONE 'Europe/Stockholm')::date AS d FROM meeting WHERE id = $1),
         t AS (SELECT b.path FROM body b JOIN m ON b.id = m.troop_id),
         placed AS (
             SELECT pl.person_id, bool_or(pl.is_leader) AS is_leader,
                    max(pb.name) FILTER (WHERE pb.kind = 'patrol') AS patrol
             FROM placement pl JOIN body pb ON pb.id = pl.body_id, m, t
             WHERE pb.path <@ t.path AND pl.valid @> m.d
             GROUP BY pl.person_id),
         ids AS (SELECT person_id FROM placed UNION SELECT person_id FROM attendance WHERE meeting_id = $1)
         SELECT p.id, p.first_name, p.last_name, date_part('year', age(m.d, p.birth_date))::int AS age,
                coalesce(pl.is_leader, a.is_leader, false) AS is_leader, pl.patrol, a.status,
                pl.person_id IS NOT NULL AS placed
         FROM ids JOIN person p ON p.id = ids.person_id CROSS JOIN m
         LEFT JOIN placed pl ON pl.person_id = p.id
         LEFT JOIN attendance a ON a.meeting_id = m.id AND a.person_id = p.id
         ORDER BY coalesce(pl.is_leader, a.is_leader, false), pl.patrol NULLS LAST, p.first_name, p.last_name",
    )
    .bind(meeting_id)
    .fetch_all(db)
    .await?)
}

async fn meeting_detail(state: &AppState, user: &CurrentUser, row: MeetingRow) -> ApiResult<MeetingDetail> {
    let can_write = user.0.can(Permission::AttendanceWrite, &row.troop_path);
    let (roster, neighbours) = tokio::try_join!(roster(&state.db, row.id), async {
        sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>)>(
                "SELECT (SELECT id FROM meeting WHERE troop_id = $1 AND (starts_at, id) < ($2, $3) ORDER BY starts_at DESC, id DESC LIMIT 1),
                        (SELECT id FROM meeting WHERE troop_id = $1 AND (starts_at, id) > ($2, $3) ORDER BY starts_at, id LIMIT 1)",
            )
            .bind(row.troop_id)
            .bind(row.starts_at)
            .bind(row.id)
            .fetch_one(&state.db)
            .await
            .map_err(ApiError::from)
    },)?;
    Ok(MeetingDetail { meeting: row.into(), roster, can_write, previous_id: neighbours.0, next_id: neighbours.1 })
}

/// Ett möte med deltagarlista och registrerad närvaro.
#[utoipa::path(get, path = "/v1/meetings/{id}", tag = "attendance",
    params(("id" = Uuid, Path, description = "Mötets id")),
    responses((status = 200, body = MeetingDetail), (status = 404, body = crate::error::Problem)))]
pub async fn get_meeting(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<MeetingDetail>> {
    let row = load_meeting(&state, &user, id, Permission::AttendanceRead).await?;
    Ok(Json(meeting_detail(&state, &user, row).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AttendanceEntry {
    pub person_id: Uuid,
    /// `present`, `absent` eller `excused`.
    pub status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AttendanceUpdate {
    /// Hela närvaron för mötet. Personer som inte finns med tas bort (idempotent helhetsuppdatering).
    pub entries: Vec<AttendanceEntry>,
}

/// Sparar närvaron för ett möte. Kräver `attendance_write`. Låsta (rapporterade) möten ger 409.
#[utoipa::path(put, path = "/v1/meetings/{id}/attendance", tag = "attendance",
    params(("id" = Uuid, Path, description = "Mötets id")),
    request_body = AttendanceUpdate,
    responses((status = 200, body = MeetingDetail), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem),
        (status = 404, body = crate::error::Problem), (status = 409, body = crate::error::Problem)))]
pub async fn put_attendance(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<AttendanceUpdate>,
) -> ApiResult<Json<MeetingDetail>> {
    let row = load_meeting(&state, &user, id, Permission::AttendanceWrite).await?;
    if row.locked {
        return Err(ApiError::Conflict("mötet är rapporterat och låst".into()));
    }
    if let Some(bad) = req.entries.iter().find(|e| !STATUSES.contains(&e.status.as_str())) {
        return Err(ApiError::BadRequest(format!("ogiltig status: {}", bad.status)));
    }
    let roster = roster(&state.db, id).await?;
    let leaders: HashMap<Uuid, bool> = roster.iter().map(|r| (r.id, r.is_leader)).collect();
    if let Some(bad) = req.entries.iter().find(|e| !leaders.contains_key(&e.person_id)) {
        return Err(ApiError::BadRequest(format!("personen {} hör inte till avdelningen", bad.person_id)));
    }

    let persons: Vec<Uuid> = req.entries.iter().map(|e| e.person_id).collect();
    let statuses: Vec<&str> = req.entries.iter().map(|e| e.status.as_str()).collect();
    let is_leader: Vec<bool> = persons.iter().map(|p| leaders[p]).collect();
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM attendance WHERE meeting_id = $1 AND NOT (person_id = ANY($2))")
        .bind(id)
        .bind(&persons)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO attendance (meeting_id, person_id, status, is_leader)
         SELECT $1, * FROM unnest($2::uuid[], $3::text[], $4::bool[])
         ON CONFLICT (meeting_id, person_id) DO UPDATE
           SET status = EXCLUDED.status, is_leader = EXCLUDED.is_leader,
               recorded_at = CASE WHEN attendance.status <> EXCLUDED.status THEN now() ELSE attendance.recorded_at END",
    )
    .bind(id)
    .bind(&persons)
    .bind(&statuses)
    .bind(&is_leader)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let present = statuses.iter().filter(|s| **s == "present").count();
    audit(
        &state.db,
        user.0.account_id,
        "attendance.update",
        "meeting",
        id,
        serde_json::json!({ "entries": persons.len(), "present": present }),
    )
    .await;
    let row = load_meeting(&state, &user, id, Permission::AttendanceWrite).await?;
    Ok(Json(meeting_detail(&state, &user, row).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMeeting {
    pub name: Option<String>,
    pub date: Option<NaiveDate>,
    /// `HH:MM`.
    pub time: Option<String>,
    pub duration_min: Option<i32>,
    /// Tom sträng tar bort platsen.
    pub location: Option<String>,
    pub kind: Option<String>,
}

/// Ändrar ett möte (t.ex. flyttar det). Kräver `attendance_write`.
#[utoipa::path(patch, path = "/v1/meetings/{id}", tag = "attendance",
    params(("id" = Uuid, Path, description = "Mötets id")),
    request_body = UpdateMeeting,
    responses((status = 200, body = MeetingDetail), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem), (status = 409, body = crate::error::Problem)))]
pub async fn update_meeting(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateMeeting>,
) -> ApiResult<Json<MeetingDetail>> {
    let row = load_meeting(&state, &user, id, Permission::AttendanceWrite).await?;
    if row.locked {
        return Err(ApiError::Conflict("mötet är rapporterat och låst".into()));
    }
    let name = req.name.as_deref().map(str::trim);
    if name.is_some_and(|n| n.is_empty() || n.chars().count() > 100) {
        return Err(ApiError::BadRequest("namnet ska vara 1–100 tecken".into()));
    }
    if let Some(kind) = req.kind.as_deref() {
        validate_kind(kind)?;
    }
    if let Some(min) = req.duration_min {
        validate_duration(min)?;
    }
    let time = req.time.as_deref().map(parse_time).transpose()?;
    sqlx::query(
        "UPDATE meeting SET
           name = coalesce($2, name),
           kind = coalesce($3, kind),
           duration_min = coalesce($4, duration_min),
           location = CASE WHEN $5::text IS NULL THEN location ELSE nullif(trim($5), '') END,
           starts_at = (coalesce($6, (starts_at AT TIME ZONE 'Europe/Stockholm')::date)
                        + coalesce($7, (starts_at AT TIME ZONE 'Europe/Stockholm')::time)) AT TIME ZONE 'Europe/Stockholm'
         WHERE id = $1",
    )
    .bind(id)
    .bind(name)
    .bind(req.kind.as_deref())
    .bind(req.duration_min)
    .bind(req.location.as_deref())
    .bind(req.date)
    .bind(time)
    .execute(&state.db)
    .await?;
    audit(&state.db, user.0.account_id, "meeting.update", "meeting", id, serde_json::json!({})).await;
    let row = load_meeting(&state, &user, id, Permission::AttendanceWrite).await?;
    Ok(Json(meeting_detail(&state, &user, row).await?))
}

/// Tar bort ett möte och dess närvaro. Kräver `attendance_write`. Låsta möten kan inte tas bort.
#[utoipa::path(delete, path = "/v1/meetings/{id}", tag = "attendance",
    params(("id" = Uuid, Path, description = "Mötets id")),
    responses((status = 204), (status = 404, body = crate::error::Problem), (status = 409, body = crate::error::Problem)))]
pub async fn delete_meeting(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let row = load_meeting(&state, &user, id, Permission::AttendanceWrite).await?;
    if row.locked {
        return Err(ApiError::Conflict("mötet är rapporterat och låst".into()));
    }
    sqlx::query("DELETE FROM meeting WHERE id = $1").bind(id).execute(&state.db).await?;
    audit(
        &state.db,
        user.0.account_id,
        "meeting.delete",
        "body",
        row.troop_id,
        serde_json::json!({ "meeting": id, "name": row.name, "date": row.local_date }),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/bodies/{id}/attendance", get(attendance_matrix))
        .route("/v1/bodies/{id}/attendance/summary", get(attendance_summary))
        .route("/v1/bodies/{id}/meetings", axum::routing::post(create_meetings))
        .route("/v1/meetings/{id}", get(get_meeting).patch(update_meeting).delete(delete_meeting))
        .route("/v1/meetings/{id}/attendance", put(put_attendance))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("giltigt datum")
    }

    #[test]
    fn weekly_series_skips_dates_and_is_bounded() {
        let dates = series_dates(d(2026, 8, 18), Some(d(2026, 9, 15)), &[d(2026, 9, 1)]).expect("serie");
        assert_eq!(dates, vec![d(2026, 8, 18), d(2026, 8, 25), d(2026, 9, 8), d(2026, 9, 15)]);
        assert_eq!(series_dates(d(2026, 8, 18), None, &[]).expect("ett möte"), vec![d(2026, 8, 18)]);
        assert!(series_dates(d(2026, 8, 18), Some(d(2026, 8, 1)), &[]).is_err());
        assert!(series_dates(d(2026, 1, 1), Some(d(2028, 1, 1)), &[]).is_err(), "för lång serie");
    }

    #[test]
    fn terms_parse_from_label_or_id() {
        assert_eq!(parse_term(Some("HT2026")).expect("ok").id(), 20261);
        assert_eq!(parse_term(Some("20270")).expect("ok").to_string(), "VT 2027");
        assert!(parse_term(Some("sommar")).is_err());
    }
}
