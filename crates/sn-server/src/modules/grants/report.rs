//! Bidragsunderlag: kontroll av sammankomster mot kommunens regler (aktivitetsbidrag) och
//! export av närvaro i DAK-format (Digitalt Aktivitetskort 2.2), kompatibelt med Skojjt.

use std::{collections::HashMap, fmt::Write as _};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime};
use serde::Serialize;
use sn_domain::{Personnummer, Term};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::modules::attendance::{TermQuery, parse_term};
use crate::routes::{BodyRef, audit, bodies::load_visible_body};
use crate::{
    AppState,
    auth::CurrentUser,
    authz::Permission,
    error::{ApiError, ApiResult},
};

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct GrantRules {
    pub name: String,
    pub municipality_code: String,
    pub year: i32,
    pub min_age: i16,
    pub max_age: i16,
    pub min_leader_age: i16,
    pub min_participants: i16,
    pub min_leaders: i16,
    pub min_minutes: i16,
    pub amount_per_female_ore: i32,
    pub amount_per_male_ore: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MeetingGrant {
    pub meeting_id: Uuid,
    pub troop: BodyRef,
    pub date: NaiveDate,
    pub time: String,
    pub name: String,
    pub kind: String,
    pub duration_min: i32,
    /// Närvarande deltagare (inte ledare).
    pub participants: i64,
    /// Deltagare inom åldersgränserna och med giltigt personnummer.
    pub eligible: i64,
    pub eligible_female: i64,
    pub eligible_male: i64,
    pub leaders: i64,
    pub qualifying_leaders: i64,
    /// Närvarande som inte räknas för att personnummer saknas.
    pub missing_national_id: i64,
    pub approved: bool,
    /// Varför sammankomsten inte godkänns, eller varningar.
    pub issues: Vec<String>,
    pub amount_ore: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TroopGrant {
    pub troop: BodyRef,
    pub held: i64,
    pub approved: i64,
    pub amount_ore: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GrantReport {
    pub term: String,
    pub term_id: i32,
    /// Kåren som söker bidraget.
    pub group: BodyRef,
    pub municipality: Option<String>,
    pub municipality_code: Option<String>,
    pub association_id: Option<String>,
    /// Kommunens regler som gäller för terminen, om de finns.
    pub rules: Option<GrantRules>,
    /// Problem som hindrar ansökan (t.ex. kommun-ID saknas).
    pub problems: Vec<String>,
    pub troops: Vec<TroopGrant>,
    pub meetings: Vec<MeetingGrant>,
    pub held: i64,
    pub approved: i64,
    pub amount_ore: i64,
    /// Om den inloggade får ladda ner DAK-filer (innehåller personnummer).
    pub can_export_dak: bool,
}

#[derive(sqlx::FromRow)]
struct GroupRow {
    id: Uuid,
    name: String,
    number: i32,
    municipality: Option<String>,
    municipality_code: Option<String>,
    association_id: Option<String>,
    org_number: Option<String>,
}

#[derive(sqlx::FromRow)]
struct MeetingRow {
    id: Uuid,
    troop_id: Uuid,
    troop_name: String,
    troop_number: i32,
    name: String,
    kind: String,
    local_start: NaiveDateTime,
    duration_min: i32,
    location: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PresentRow {
    meeting_id: Uuid,
    person_id: Uuid,
    is_leader: bool,
    birth_date: NaiveDate,
    national_id: Option<String>,
}

/// Kåren som enheten hör till (eller enheten själv). Bidrag söks per kår.
async fn load_scope(state: &AppState, user: &CurrentUser, key: &str) -> ApiResult<(GroupRow, String, bool)> {
    let (_, kind, path) = load_visible_body(state, user, key, Permission::ReportsRead).await?;
    if kind != "group" && kind != "troop" {
        return Err(ApiError::BadRequest("bidragsunderlag tas fram per kår eller avdelning".into()));
    }
    let group = sqlx::query_as::<_, GroupRow>(
        "SELECT id, name, number, municipality, municipality_code, association_id, org_number
         FROM body WHERE kind = 'group' AND path @> $1::ltree ORDER BY nlevel(path) DESC LIMIT 1",
    )
    .bind(&path)
    .fetch_one(&state.db)
    .await?;
    Ok((group, path, kind == "troop"))
}

async fn load_rules(state: &AppState, code: Option<&str>, year: i32) -> ApiResult<Option<GrantRules>> {
    let Some(code) = code else { return Ok(None) };
    Ok(sqlx::query_as::<_, GrantRules>(
        "SELECT name, municipality_code, year, min_age, max_age, min_leader_age, min_participants, min_leaders,
                min_minutes, amount_per_female_ore, amount_per_male_ore
         FROM grant_ruleset WHERE municipality_code = $1 AND year <= $2 ORDER BY year DESC LIMIT 1",
    )
    .bind(code)
    .bind(year)
    .fetch_optional(&state.db)
    .await?)
}

async fn load_meetings(state: &AppState, path: &str, term: Term) -> ApiResult<(Vec<MeetingRow>, Vec<PresentRow>)> {
    let meetings = sqlx::query_as::<_, MeetingRow>(
        "SELECT m.id, m.troop_id, b.name AS troop_name, b.number AS troop_number, m.name, m.kind,
                (m.starts_at AT TIME ZONE 'Europe/Stockholm') AS local_start, m.duration_min, m.location
         FROM meeting m JOIN body b ON b.id = m.troop_id
         WHERE b.path <@ $1::ltree AND m.starts_at < now()
           AND (m.starts_at AT TIME ZONE 'Europe/Stockholm')::date BETWEEN $2 AND $3
         ORDER BY b.name, m.starts_at",
    )
    .bind(path)
    .bind(term.start())
    .bind(term.end())
    .fetch_all(&state.db)
    .await?;
    let ids: Vec<Uuid> = meetings.iter().map(|m| m.id).collect();
    let present = sqlx::query_as::<_, PresentRow>(
        "SELECT a.meeting_id, a.person_id, a.is_leader, p.birth_date, p.national_id
         FROM attendance a JOIN person p ON p.id = a.person_id
         WHERE a.meeting_id = ANY($1) AND a.status = 'present'",
    )
    .bind(&ids)
    .fetch_all(&state.db)
    .await?;
    Ok((meetings, present))
}

/// Kontrollerar en sammankomst mot reglerna. Ålder räknas som sammankomstens år minus födelseår.
fn evaluate(m: &MeetingRow, present: &[&PresentRow], rules: Option<&GrantRules>) -> MeetingGrant {
    let year = m.local_start.year();
    let mut g = MeetingGrant {
        meeting_id: m.id,
        troop: BodyRef { id: m.troop_id, name: m.troop_name.clone(), kind: "troop".into(), number: m.troop_number },
        date: m.local_start.date(),
        time: m.local_start.format("%H:%M").to_string(),
        name: m.name.clone(),
        kind: m.kind.clone(),
        duration_min: m.duration_min,
        participants: 0,
        eligible: 0,
        eligible_female: 0,
        eligible_male: 0,
        leaders: 0,
        qualifying_leaders: 0,
        missing_national_id: 0,
        approved: false,
        issues: Vec::new(),
        amount_ore: 0,
    };
    for p in present {
        let age = year - p.birth_date.year();
        let pnr = p.national_id.as_deref().and_then(|n| n.parse::<Personnummer>().ok());
        if p.is_leader {
            g.leaders += 1;
            if rules.is_none_or(|r| age >= i32::from(r.min_leader_age)) {
                g.qualifying_leaders += 1;
            }
            continue;
        }
        g.participants += 1;
        let Some(pnr) = pnr else {
            g.missing_national_id += 1;
            continue;
        };
        if rules.is_none_or(|r| (i32::from(r.min_age)..=i32::from(r.max_age)).contains(&age)) {
            g.eligible += 1;
            if pnr.legal_sex_is_female() {
                g.eligible_female += 1;
            } else {
                g.eligible_male += 1;
            }
        }
    }
    let Some(r) = rules else {
        g.issues.push("Kommunen saknar bidragsregler i Scoutnet 2".into());
        return g;
    };
    if g.duration_min < i32::from(r.min_minutes) {
        g.issues.push(format!("Sammankomsten är {} min, kräver minst {} min", g.duration_min, r.min_minutes));
    }
    if g.qualifying_leaders < i64::from(r.min_leaders) {
        g.issues.push(format!(
            "{} ledare (minst {} år), kräver minst {}",
            g.qualifying_leaders, r.min_leader_age, r.min_leaders
        ));
    }
    if g.eligible < i64::from(r.min_participants) {
        g.issues.push(format!(
            "{} deltagare {}–{} år, kräver minst {}",
            g.eligible, r.min_age, r.max_age, r.min_participants
        ));
    }
    if g.participants + g.leaders == 0 {
        g.issues.insert(0, "Ingen närvaro registrerad".into());
    }
    g.approved = g.issues.is_empty();
    if g.missing_national_id > 0 {
        g.issues.push(format!("{} deltagare saknar personnummer och räknas inte", g.missing_national_id));
    }
    if g.approved {
        g.amount_ore =
            g.eligible_female * i64::from(r.amount_per_female_ore) + g.eligible_male * i64::from(r.amount_per_male_ore);
    }
    g
}

/// Bidragsunderlag för en kår eller avdelning och termin: varje genomförd sammankomst kontrolleras
/// mot kommunens regler (ålder, antal deltagare och ledare, längd) och bidraget beräknas.
#[utoipa::path(get, path = "/v1/bodies/{id}/grants", tag = "grants",
    params(("id" = String, Path, description = "Kårens eller avdelningens id eller nyckel"), TermQuery),
    responses((status = 200, body = GrantReport), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn grant_report(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
    Query(q): Query<TermQuery>,
) -> ApiResult<Json<GrantReport>> {
    let term = parse_term(q.term.as_deref())?;
    let (group, path, _) = load_scope(&state, &user, &key).await?;
    let rules = load_rules(&state, group.municipality_code.as_deref(), term.year()).await?;
    let (meetings, present) = load_meetings(&state, &path, term).await?;

    let mut by_meeting: HashMap<Uuid, Vec<&PresentRow>> = HashMap::new();
    for p in &present {
        by_meeting.entry(p.meeting_id).or_default().push(p);
    }
    let evaluated: Vec<MeetingGrant> = meetings
        .iter()
        .map(|m| evaluate(m, by_meeting.get(&m.id).map_or(&[][..], Vec::as_slice), rules.as_ref()))
        .collect();

    let mut troops: Vec<TroopGrant> = Vec::new();
    for m in &evaluated {
        if troops.last().is_none_or(|t| t.troop.id != m.troop.id) {
            troops.push(TroopGrant { troop: m.troop.clone(), held: 0, approved: 0, amount_ore: 0 });
        }
        if let Some(t) = troops.last_mut() {
            t.held += 1;
            t.approved += i64::from(m.approved);
            t.amount_ore += m.amount_ore;
        }
    }

    let mut problems = Vec::new();
    if group.municipality_code.is_none() {
        problems.push("Kommun-ID saknas för kåren".to_owned());
    } else if rules.is_none() {
        problems.push(format!(
            "Det finns inga bidragsregler för {} i Scoutnet 2 ännu",
            group.municipality.as_deref().unwrap_or("kommunen")
        ));
    }
    if group.association_id.is_none() {
        problems.push("Förenings-ID (för DAK) saknas för kåren".to_owned());
    }

    Ok(Json(GrantReport {
        term: term.to_string(),
        term_id: term.id(),
        held: i64::try_from(evaluated.len()).unwrap_or(i64::MAX),
        approved: troops.iter().map(|t| t.approved).sum(),
        amount_ore: troops.iter().map(|t| t.amount_ore).sum(),
        group: BodyRef { id: group.id, name: group.name, kind: "group".into(), number: group.number },
        municipality: group.municipality,
        municipality_code: group.municipality_code,
        association_id: group.association_id,
        rules,
        problems,
        troops,
        meetings: evaluated,
        can_export_dak: user.0.can(Permission::NationalIdRead, &path),
    }))
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

#[derive(sqlx::FromRow)]
struct CardPerson {
    id: Uuid,
    member_no: i32,
    first_name: String,
    last_name: String,
    national_id: Option<String>,
    email: Option<String>,
    mobile_phone: Option<String>,
    is_leader: bool,
}

struct DakCard<'a> {
    group: &'a GroupRow,
    troop_name: &'a str,
    troop_number: i32,
    term: Term,
    location: &'a str,
    meetings: &'a [MeetingRow],
    present: &'a HashMap<Uuid, Vec<&'a PresentRow>>,
    people: &'a [CardPerson],
}

/// DAK 2.2-XML med samma struktur och koder som Skojjt, så att kommunerna känner igen filerna.
fn dak_xml(c: &DakCard<'_>) -> String {
    let e = xml_escape;
    let digits = |n: &Option<String>| {
        n.as_deref().map(|n| n.chars().filter(char::is_ascii_digit).collect::<String>()).unwrap_or_default()
    };
    let uid: HashMap<Uuid, (String, bool)> =
        c.people.iter().map(|p| (p.id, (format!("{:02}", p.member_no), p.is_leader))).collect();
    let mut x = String::new();
    let _ = writeln!(x, r#"<?xml version="1.0" encoding="utf-8"?>"#);
    let _ = writeln!(
        x,
        r#"<Aktivitetskort xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://sverigesforeningssystem.se/importSchema.xsd">"#
    );
    let _ =
        writeln!(x, r#"  <Kommun kommunID="{}" version="2.2">"#, e(c.group.municipality_code.as_deref().unwrap_or("")));
    let _ = writeln!(
        x,
        r#"    <Foerening foereningsID="{}" foereningsNamn="{}" organisationsnummer="{}">"#,
        e(c.group.association_id.as_deref().unwrap_or("")),
        e(&c.group.name),
        e(c.group.org_number.as_deref().unwrap_or(""))
    );
    let _ = writeln!(
        x,
        r#"      <Naervarokort NaervarokortNummer="{}">"#,
        i64::from(c.troop_number) * 1000 + i64::from(c.term.id())
    );
    let _ = writeln!(x, "        <Aktivitet>Scouting</Aktivitet>");
    let _ = writeln!(x, "        <Lokal>{}</Lokal>", e(c.location));
    let _ = writeln!(x, "        <NamnPaaKort>{}</NamnPaaKort>", e(c.troop_name));
    let _ = writeln!(x, "        <Sammankomster>");
    for m in c.meetings {
        let start = m.local_start;
        let end_of_day = start.date().and_time(NaiveTime::from_hms_opt(23, 59, 59).unwrap_or(NaiveTime::MIN));
        let stop = (start + Duration::minutes(i64::from(m.duration_min))).min(end_of_day);
        let activity = if m.kind == "meeting" { m.name.clone() } else { format!("{} #hike", m.name) };
        let _ = writeln!(
            x,
            r#"          <Sammankomst Datum="{}" kod="{}-{}-{}-{}">"#,
            start.format("%Y-%m-%d"),
            c.group.number,
            c.troop_number,
            c.term.id(),
            start.format("%m%d")
        );
        let _ = writeln!(x, "            <StartTid>{}</StartTid>", start.format("%H:%M:%S"));
        let _ = writeln!(x, "            <StoppTid>{}</StoppTid>", stop.format("%H:%M:%S"));
        let _ = writeln!(x, "            <Aktivitet>{}</Aktivitet>", e(&activity));
        let _ = writeln!(x, "            <Lokal>{}</Lokal>", e(m.location.as_deref().unwrap_or(c.location)));
        let _ = writeln!(x, "            <Typ>Moete</Typ>");
        let _ = writeln!(x, "            <Metod>Add</Metod>");
        let mut attending: Vec<&(String, bool)> =
            c.present.get(&m.id).into_iter().flatten().filter_map(|p| uid.get(&p.person_id)).collect();
        attending.sort();
        for (list, leader) in [("DeltagarLista", false), ("LedarLista", true)] {
            let tag = if leader { "Ledare" } else { "Deltagare" };
            let _ = writeln!(x, "            <{list}>");
            for (id, _) in attending.iter().filter(|(_, l)| *l == leader) {
                let _ = writeln!(
                    x,
                    r#"              <{tag} id="{id}"><Handikapp>false</Handikapp><Naervarande>true</Naervarande></{tag}>"#
                );
            }
            let _ = writeln!(x, "            </{list}>");
        }
        let _ = writeln!(x, "          </Sammankomst>");
    }
    let _ = writeln!(x, "        </Sammankomster>");
    let _ = writeln!(x, "      </Naervarokort>");
    let _ = writeln!(x, "    </Foerening>");
    let _ = writeln!(x, "  </Kommun>");
    let mut people: Vec<&CardPerson> = c.people.iter().collect();
    people.sort_by_key(|p| p.member_no);
    let _ = writeln!(x, "  <DeltagarRegister>");
    for p in people.iter().filter(|p| !p.is_leader) {
        let _ = writeln!(
            x,
            r#"    <Deltagare id="{:02}"><Foernamn>{}</Foernamn><Efternamn>{}</Efternamn><Personnummer>{}</Personnummer></Deltagare>"#,
            p.member_no,
            e(&p.first_name),
            e(&p.last_name),
            digits(&p.national_id)
        );
    }
    let _ = writeln!(x, "  </DeltagarRegister>");
    let _ = writeln!(x, "  <LedarRegister>");
    for p in people.iter().filter(|p| p.is_leader) {
        let _ = writeln!(
            x,
            r#"    <Ledare id="{:02}"><Foernamn>{}</Foernamn><Efternamn>{}</Efternamn><Personnummer>{}</Personnummer><Epost>{}</Epost><MobilNr>{}</MobilNr></Ledare>"#,
            p.member_no,
            e(&p.first_name),
            e(&p.last_name),
            digits(&p.national_id),
            e(p.email.as_deref().unwrap_or("")),
            e(p.mobile_phone.as_deref().unwrap_or(""))
        );
    }
    let _ = writeln!(x, "  </LedarRegister>");
    let _ = writeln!(x, "</Aktivitetskort>");
    x
}

/// DAK-fil (Digitalt Aktivitetskort 2.2) för en avdelning och termin, att lämna till kommunen.
/// Innehåller personnummer: kräver `national_id_read` och färsk inloggning. Varje export loggas.
#[utoipa::path(get, path = "/v1/bodies/{id}/grants/dak", tag = "grants",
    params(("id" = String, Path, description = "Avdelningens id eller nyckel, t.ex. `troop-5003`"), TermQuery),
    responses((status = 200, description = "DAK-XML", content_type = "application/xml"), (status = 400, body = crate::error::Problem),
        (status = 401, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn dak_export(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
    Query(q): Query<TermQuery>,
) -> ApiResult<Response> {
    let term = parse_term(q.term.as_deref())?;
    let (group, path, is_troop) = load_scope(&state, &user, &key).await?;
    if !is_troop {
        return Err(ApiError::BadRequest("DAK-filer tas fram per avdelning".into()));
    }
    if !user.0.can(Permission::NationalIdRead, &path) {
        return Err(ApiError::Forbidden);
    }
    if !user.0.has_recent_auth() {
        return Err(ApiError::StepUpRequired);
    }
    if group.municipality_code.is_none() || group.association_id.is_none() {
        return Err(ApiError::Conflict("kommun-ID och förenings-ID måste vara ifyllda för kåren".into()));
    }
    let (troop_id, troop_name, troop_number, location): (Uuid, String, i32, Option<String>) =
        sqlx::query_as("SELECT id, name, number, meeting_location FROM body WHERE path = $1::ltree")
            .bind(&path)
            .fetch_one(&state.db)
            .await?;
    let (meetings, present) = load_meetings(&state, &path, term).await?;
    let mut by_meeting: HashMap<Uuid, Vec<&PresentRow>> = HashMap::new();
    for p in &present {
        by_meeting.entry(p.meeting_id).or_default().push(p);
    }
    let people = sqlx::query_as::<_, CardPerson>(
        "WITH t AS (SELECT path FROM body WHERE id = $1),
         placed AS (
             SELECT pl.person_id, bool_or(pl.is_leader) AS is_leader
             FROM placement pl JOIN body pb ON pb.id = pl.body_id, t
             WHERE pb.path <@ t.path AND pl.valid && daterange($2, $3, '[]') GROUP BY pl.person_id),
         attended AS (SELECT person_id, bool_or(is_leader) AS is_leader FROM attendance WHERE meeting_id = ANY($4) GROUP BY person_id)
         SELECT p.id, p.member_no, p.first_name, p.last_name, p.national_id, p.email, p.mobile_phone,
                coalesce(pl.is_leader, ad.is_leader, false) AS is_leader
         FROM person p LEFT JOIN placed pl ON pl.person_id = p.id LEFT JOIN attended ad ON ad.person_id = p.id
         WHERE pl.person_id IS NOT NULL OR ad.person_id IS NOT NULL",
    )
    .bind(troop_id)
    .bind(term.start())
    .bind(term.end())
    .bind(meetings.iter().map(|m| m.id).collect::<Vec<_>>())
    .fetch_all(&state.db)
    .await?;

    let xml = dak_xml(&DakCard {
        group: &group,
        troop_name: &troop_name,
        troop_number,
        term,
        location: location.as_deref().unwrap_or(""),
        meetings: &meetings,
        present: &by_meeting,
        people: &people,
    });
    audit(
        &state.db,
        user.0.account_id,
        "grant.dak.export",
        "body",
        troop_id,
        serde_json::json!({
            "term": term.to_string(),
            "meetings": meetings.len(),
            "count": people.len(),
            // Exakt vilka som fanns i filen, så att varje person kan se exporten i sin logg.
            "persons": people.iter().map(|p| p.id).collect::<Vec<_>>(),
        }),
    )
    .await;
    let filename = format!("DAK-avdelning-{troop_number}-{}.xml", term.to_string().replace(' ', ""));
    Ok((
        [
            (header::CONTENT_TYPE, "application/xml; charset=utf-8".to_owned()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
        ],
        xml,
    )
        .into_response())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/bodies/{id}/grants", get(grant_report))
        .route("/v1/bodies/{id}/grants/dak", get(dak_export))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> GrantRules {
        GrantRules {
            name: "Göteborg".into(),
            municipality_code: "1480".into(),
            year: 2026,
            min_age: 7,
            max_age: 25,
            min_leader_age: 13,
            min_participants: 3,
            min_leaders: 1,
            min_minutes: 60,
            amount_per_female_ore: 989,
            amount_per_male_ore: 802,
        }
    }

    fn meeting(minutes: i32) -> MeetingRow {
        MeetingRow {
            id: Uuid::nil(),
            troop_id: Uuid::nil(),
            troop_name: "Vargarna".into(),
            troop_number: 5003,
            name: "Möte v.42".into(),
            kind: "meeting".into(),
            local_start: NaiveDate::from_ymd_opt(2026, 10, 15).and_then(|d| d.and_hms_opt(18, 30, 0)).expect("datum"),
            duration_min: minutes,
            location: None,
        }
    }

    fn person(year: i32, idx: u8, leader: bool) -> PresentRow {
        let birth = NaiveDate::from_ymd_opt(year, 3, 1).expect("datum");
        PresentRow {
            meeting_id: Uuid::nil(),
            person_id: Uuid::now_v7(),
            is_leader: leader,
            birth_date: birth,
            national_id: Some(Personnummer::test_number(birth, idx).to_string()),
        }
    }

    #[test]
    fn meeting_is_approved_and_paid_by_legal_sex() {
        // Två flickor (jämnt födelsenummer) och en pojke, 10 år, plus en ledare.
        let people = [person(2016, 0, false), person(2016, 2, false), person(2016, 1, false), person(1990, 3, true)];
        let refs: Vec<&PresentRow> = people.iter().collect();
        let g = evaluate(&meeting(90), &refs, Some(&rules()));
        assert!(g.approved, "{:?}", g.issues);
        assert_eq!((g.eligible_female, g.eligible_male), (2, 1));
        assert_eq!(g.amount_ore, 2 * 989 + 802);
    }

    #[test]
    fn meeting_fails_on_duration_leaders_and_age() {
        let people = [person(2016, 0, false), person(2022, 2, false), person(1990, 1, false), person(2015, 3, true)];
        let refs: Vec<&PresentRow> = people.iter().collect();
        let g = evaluate(&meeting(45), &refs, Some(&rules()));
        assert!(!g.approved);
        assert_eq!(g.eligible, 1, "4-åring och 36-åring räknas inte");
        assert_eq!(g.qualifying_leaders, 0, "11-årig ledare räknas inte");
        assert_eq!(g.issues.len(), 3);
        assert_eq!(g.amount_ore, 0);
    }

    #[test]
    fn xml_is_escaped() {
        assert_eq!(xml_escape("Kåren <\"A&B\">"), "Kåren &lt;&quot;A&amp;B&quot;&gt;");
    }
}
