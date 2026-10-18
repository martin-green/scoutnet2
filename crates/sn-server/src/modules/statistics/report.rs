use std::{collections::HashMap, time::Instant};

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use chrono::{Days, NaiveDate};
use serde::Serialize;
use sn_domain::{Term, TroopType};
use utoipa::ToSchema;

use crate::{
    AppState,
    auth::CurrentUser,
    authz::Permission,
    error::ApiResult,
    modules::{attendance, is_active},
    routes::bodies::{group_scope, load_visible_body},
};

/// Antal terminer bakåt i medlemsutvecklingen.
const TERMS: usize = 6;
/// Åldersgrupper: undre gränser. Den första gruppen är 0–6 år.
const AGE_LIMITS: [i32; 8] = [7, 10, 13, 16, 19, 26, 41, 61];
const AGE_LABELS: [&str; 9] = ["0–6", "7–9", "10–12", "13–15", "16–18", "19–25", "26–40", "41–60", "61+"];

#[derive(Debug, Serialize, ToSchema)]
pub struct Statistics {
    pub term: String,
    pub totals: Totals,
    /// Aktiva medlemmar per åldersgrupp och kön.
    pub age_groups: Vec<AgeGroup>,
    /// Per åldersgren (avdelningstyp).
    pub troop_types: Vec<TroopTypeStats>,
    /// Aktiva medlemmar vid varje terminsstart och i dag, äldst först.
    pub trend: Vec<TrendPoint>,
    /// Nya och avslutade medlemskap per termin, äldst först. Tom för avdelningar.
    pub flows: Vec<Flow>,
    /// Väntelistan per önskad gren. Tom för avdelningar.
    pub waiting: Vec<WaitingStats>,
    /// Om närvaron är med (kräver modulen Närvaro).
    pub attendance_included: bool,
    pub took_ms: f64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Totals {
    pub active: i64,
    pub scouts: i64,
    pub leaders: i64,
    pub waiting: i64,
    pub pending: i64,
    pub troops: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgeGroup {
    pub label: String,
    pub female: i64,
    pub male: i64,
    /// Annat eller okänt kön.
    pub other: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TroopTypeStats {
    pub troop_type: String,
    pub label: String,
    pub troops: i64,
    pub scouts: i64,
    pub leaders: i64,
    pub scouts_per_leader: Option<f64>,
    /// Andel närvarande på terminens genomförda möten (0–1).
    pub attendance_pct: Option<f64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TrendPoint {
    pub label: String,
    pub date: NaiveDate,
    pub active: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Flow {
    pub term: String,
    pub joined: i64,
    pub left: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WaitingStats {
    pub troop_type: Option<String>,
    pub label: String,
    pub count: i64,
    pub median_days: Option<f64>,
}

fn troop_label(t: Option<&str>) -> String {
    t.and_then(|t| t.parse::<TroopType>().ok()).map_or_else(|| "Ingen önskan".to_owned(), |t| t.label_sv().to_owned())
}

/// Antal per status, ledare, gren, kön och åldersgrupp, i en genomgång av medlemmarna.
#[derive(sqlx::FromRow)]
struct MemberCell {
    status: String,
    is_leader: bool,
    troop_type: Option<String>,
    sex: String,
    bucket: i32,
    n: i64,
}

async fn member_cells(state: &AppState, scope: &str, path: &str) -> ApiResult<Vec<MemberCell>> {
    Ok(sqlx::query_as::<_, MemberCell>(
        "SELECT mc.membership_status AS status, mc.is_leader, mc.troop_type, mc.sex,
                width_bucket(date_part('year', age(mc.birth_date)), $3::float8[])::int AS bucket, count(*) AS n
         FROM member_current mc WHERE mc.group_path <@ $1::ltree AND mc.effective_path <@ $2::ltree
         GROUP BY 1, 2, 3, 4, 5",
    )
    .bind(scope)
    .bind(path)
    .bind(AGE_LIMITS.map(f64::from).to_vec())
    .fetch_all(&state.db)
    .await?)
}

async fn troop_counts(state: &AppState, path: &str) -> ApiResult<HashMap<String, i64>> {
    Ok(sqlx::query_as::<_, (String, i64)>(
        "SELECT coalesce(troop_type, 'annan'), count(*) FROM body
         WHERE kind = 'troop' AND status = 'active' AND path <@ $1::ltree GROUP BY 1",
    )
    .bind(path)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .collect())
}

/// Andel närvarande per gren på terminens genomförda möten. Tom om modulen Närvaro är av.
async fn attendance_by_type(state: &AppState, path: &str, term: Term) -> ApiResult<Option<HashMap<String, f64>>> {
    if !is_active(state, attendance::ID, path).await? {
        return Ok(None);
    }
    let term_end = term.end().checked_add_days(Days::new(1)).unwrap_or_else(|| term.end());
    Ok(Some(
        sqlx::query_as::<_, (String, Option<f64>)>(
            "SELECT coalesce(b.troop_type, 'annan'),
                    (count(*) FILTER (WHERE a.status = 'present'))::float8 / nullif(count(*), 0)
             FROM attendance a JOIN meeting mt ON mt.id = a.meeting_id JOIN body b ON b.id = mt.troop_id
             WHERE b.path <@ $1::ltree AND mt.starts_at >= $2 AND mt.starts_at < $3 AND mt.starts_at < now()
             GROUP BY 1",
        )
        .bind(path)
        .bind(term.start())
        .bind(term_end)
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .filter_map(|(t, p)| p.map(|p| (t, p)))
        .collect(),
    ))
}

/// Antal aktiva vid varje datum. Avdelningar räknas på placeringar, större enheter på primära
/// medlemskap. Medlemskapen grupperas först på giltighetstid, så att varje datum prövas mot
/// några tusen intervall i stället för mot varje medlemskap.
async fn active_at(
    state: &AppState,
    path: &str,
    dates: &[NaiveDate],
    is_troop: bool,
) -> ApiResult<Vec<(NaiveDate, i64)>> {
    let sql = if is_troop {
        "SELECT d, count(DISTINCT pl.person_id)
         FROM unnest($1::date[]) AS d
         LEFT JOIN (placement pl JOIN body b ON b.id = pl.body_id AND b.path <@ $2::ltree) ON pl.valid @> d
         GROUP BY d ORDER BY d"
    } else {
        "SELECT d.d, coalesce(sum(x.n), 0)::int8
         FROM unnest($1::date[]) AS d(d)
         LEFT JOIN (SELECT m.valid, count(*) AS n FROM membership m JOIN body g ON g.id = m.group_id
                    WHERE g.path <@ $2::ltree AND m.is_primary AND m.status IN ('active', 'ended')
                    GROUP BY m.valid) x ON x.valid @> d.d
         GROUP BY d.d ORDER BY d.d"
    };
    Ok(sqlx::query_as(sql).bind(dates).bind(path).fetch_all(&state.db).await?)
}

/// Nya och avslutade primära medlemskap per termin.
async fn flows(state: &AppState, path: &str, terms: &[Term]) -> ApiResult<Vec<Flow>> {
    let ids: Vec<i32> = terms.iter().map(|t| t.id()).collect();
    let starts: Vec<NaiveDate> = terms.iter().map(|t| t.start()).collect();
    let ends: Vec<NaiveDate> = terms.iter().map(|t| t.end()).collect();
    let rows: Vec<(i32, i64, i64)> = sqlx::query_as(
        "SELECT t.id,
                count(*) FILTER (WHERE lower(m.valid) BETWEEN t.s AND t.e),
                count(*) FILTER (WHERE m.status = 'ended' AND upper(m.valid) - 1 BETWEEN t.s AND t.e)
         FROM unnest($1::int[], $2::date[], $3::date[]) AS t(id, s, e)
         JOIN membership m ON m.is_primary AND m.status IN ('active', 'ended')
                          AND (lower(m.valid) BETWEEN t.s AND t.e OR upper(m.valid) - 1 BETWEEN t.s AND t.e)
         JOIN body g ON g.id = m.group_id AND g.path <@ $4::ltree
         GROUP BY t.id",
    )
    .bind(&ids)
    .bind(&starts)
    .bind(&ends)
    .bind(path)
    .fetch_all(&state.db)
    .await?;
    Ok(terms
        .iter()
        .map(|t| {
            let row = rows.iter().find(|r| r.0 == t.id());
            Flow { term: t.to_string(), joined: row.map_or(0, |r| r.1), left: row.map_or(0, |r| r.2) }
        })
        .collect())
}

async fn waiting(state: &AppState, scope: &str, path: &str) -> ApiResult<Vec<WaitingStats>> {
    Ok(sqlx::query_as::<_, (Option<String>, i64, Option<f64>)>(
        "SELECT mc.wished_troop_type, count(*),
                percentile_cont(0.5) WITHIN GROUP (ORDER BY (current_date - mc.waiting_since)::float8)
         FROM member_current mc
         WHERE mc.group_path <@ $1::ltree AND mc.effective_path <@ $2::ltree AND mc.membership_status = 'waiting'
         GROUP BY 1
         ORDER BY array_position(ARRAY['familj','baver','sparare','upptackare','aventyrare','utmanare','rover','annan'], mc.wished_troop_type)",
    )
    .bind(scope)
    .bind(path)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(t, count, median)| WaitingStats { label: troop_label(t.as_deref()), troop_type: t, count, median_days: median })
    .collect())
}

/// Statistik för en enhet och allt under den. Bara antal, inga personuppgifter.
#[utoipa::path(get, path = "/v1/bodies/{id}/statistics", tag = "statistics",
    params(("id" = String, Path, description = "Enhetens id eller nyckel, t.ex. `group-1001` eller `district-1`")),
    responses((status = 200, body = Statistics), (status = 404, body = crate::error::Problem)))]
pub async fn statistics(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
) -> ApiResult<Json<Statistics>> {
    let started = Instant::now();
    let (_, kind, path) = load_visible_body(&state, &user, &key, Permission::ReportsRead).await?;
    let scope = group_scope(&state, &path).await?;
    let today = chrono::Local::now().date_naive();
    let term = Term::containing(today);
    let is_troop = matches!(kind.as_str(), "troop" | "patrol");

    // Terminer, äldst först. Trenden visar varje terminsstart och dagens datum.
    let mut terms = vec![term];
    for _ in 1..TERMS {
        let prev = terms.last().copied().unwrap_or(term).previous();
        terms.push(prev);
    }
    terms.reverse();
    let mut dates: Vec<NaiveDate> = terms.iter().map(|t| t.start()).filter(|d| *d <= today).collect();
    dates.push(today);

    // Delarna är oberoende och körs samtidigt på olika anslutningar. Avdelningar har inga
    // medlemskap eller väntelista av egen.
    let (cells, troops, att, trend_rows, flows, waiting) = tokio::try_join!(
        member_cells(&state, &scope, &path),
        troop_counts(&state, &path),
        attendance_by_type(&state, &path, term),
        active_at(&state, &path, &dates, is_troop),
        async { if is_troop { Ok(Vec::new()) } else { flows(&state, &path, &terms).await } },
        async { if is_troop { Ok(Vec::new()) } else { waiting(&state, &scope, &path).await } },
    )?;

    let sum = |f: &dyn Fn(&MemberCell) -> bool| cells.iter().filter(|c| f(c)).map(|c| c.n).sum::<i64>();
    let active = sum(&|c| c.status == "active");
    let leaders = sum(&|c| c.status == "active" && c.is_leader);
    let age_groups = AGE_LABELS
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let in_group = |c: &MemberCell| c.status == "active" && usize::try_from(c.bucket).ok() == Some(i);
            AgeGroup {
                label: (*label).to_owned(),
                female: sum(&|c| in_group(c) && c.sex == "female"),
                male: sum(&|c| in_group(c) && c.sex == "male"),
                other: sum(&|c| in_group(c) && c.sex != "female" && c.sex != "male"),
            }
        })
        .collect();
    let troop_types = TroopType::ALL
        .iter()
        .filter_map(|t| {
            let key = t.as_str();
            let of_type = |c: &MemberCell| c.status == "active" && c.troop_type.as_deref() == Some(key);
            let scouts = sum(&|c| of_type(c) && !c.is_leader);
            let leaders = sum(&|c| of_type(c) && c.is_leader);
            let count = troops.get(key).copied().unwrap_or(0);
            (count > 0 || scouts + leaders > 0).then(|| TroopTypeStats {
                troop_type: key.to_owned(),
                label: t.label_sv().to_owned(),
                troops: count,
                scouts,
                leaders,
                #[expect(clippy::cast_precision_loss, reason = "antal personer")]
                scouts_per_leader: (leaders > 0 && *t != TroopType::Annan).then(|| scouts as f64 / leaders as f64),
                attendance_pct: att.as_ref().and_then(|a| a.get(key).copied()),
            })
        })
        .collect();
    let trend = trend_rows
        .into_iter()
        .map(|(date, active)| TrendPoint {
            label: if date == today {
                "I dag".to_owned()
            } else {
                terms.iter().find(|t| t.start() == date).map_or_else(|| date.to_string(), ToString::to_string)
            },
            date,
            active,
        })
        .collect();

    Ok(Json(Statistics {
        term: term.to_string(),
        totals: Totals {
            active,
            scouts: active - leaders,
            leaders,
            waiting: sum(&|c| c.status == "waiting"),
            pending: sum(&|c| c.status == "pending"),
            troops: troops.values().sum(),
        },
        age_groups,
        troop_types,
        trend,
        flows,
        waiting,
        attendance_included: att.is_some(),
        took_ms: started.elapsed().as_secs_f64() * 1000.0,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/bodies/{id}/statistics", get(statistics))
}
