use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use chrono::{DateTime, Days, NaiveDate, Utc};
use serde::Serialize;
use sn_domain::{Personnummer, Term, TroopType};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{BodyRef, audit, me::RoleGrantDto, members::RefDto, members::TroopRefDto};
use crate::{
    AppState,
    auth::CurrentUser,
    authz::{Permission, role_label},
    error::{ApiError, ApiResult},
};

/// Hela personkortet med alla fält som Scoutnet har. Grupper som den inloggade saknar
/// behörighet för är `null` (se `docs/04-faltkatalog.md`).
#[derive(Debug, Serialize, ToSchema)]
#[expect(clippy::struct_excessive_bools, reason = "API-DTO med oberoende flaggor")]
pub struct PersonDetail {
    pub id: Uuid,
    pub member_no: i32,
    pub first_name: String,
    pub last_name: String,
    pub nickname: Option<String>,
    pub birth_date: NaiveDate,
    pub age: i32,
    pub sex: String,
    /// `active`, `new`, `unregistered`, `auto_unregistered`, `deceased`, `active_non_member`, `anonymized`, `locked`.
    pub person_status: String,
    pub preferred_language: String,
    pub presentation: Option<String>,
    pub photo_url: Option<String>,
    pub created_at: DateTime<Utc>,
    /// När personen (eller vårdnadshavare) senast bekräftade uppgifterna.
    pub confirmed_at: Option<DateTime<Utc>>,
    pub protected_identity: bool,
    /// Vuxen som kan tänka sig att hjälpa till (Scoutnet: `contact_leader_interest`).
    pub can_help: bool,
    /// Kräver `members_contact_read`.
    pub contact: Option<ContactDto>,
    /// Kräver `personal_read`. Vid skyddad identitet visas endast förmedlingsadress.
    pub addresses: Option<Vec<AddressDto>>,
    /// `ÅÅÅÅMMDD-****`. Kräver `personal_read` och döljs vid skyddad identitet.
    pub national_id_masked: Option<String>,
    pub membership: MembershipDto,
    pub troop: Option<TroopRefDto>,
    pub patrol: Option<RefDto>,
    pub is_leader: bool,
    /// Anhöriga/vårdnadshavare i ordning (Scoutnet: anhörig 1, anhörig 2). Kontaktuppgifter kräver `members_contact_read`.
    pub guardians: Vec<GuardianDto>,
    /// Hushåll/familj. Endast medlemmar som den inloggade får se listas.
    pub household: Option<HouseholdDto>,
    /// Utskicks- och fakturainställningar. Kräver `members_contact_read`.
    pub preferences: Option<PreferencesDto>,
    /// Samtycken (t.ex. publicering av bild). Synliga för alla som ser personen, så att ledare kan följa dem.
    pub consents: Vec<ConsentDto>,
    /// Intern anteckning. Kräver `members_manage`.
    pub note: Option<String>,
    /// Kräver `personal_read`.
    pub police_check: Option<PoliceCheckDto>,
    /// Kräver `personal_read`.
    pub follow_ups: Option<Vec<FollowUpDto>>,
    /// Terminsavgifter. Kräver `economy_read`.
    pub fees: Option<Vec<TermFeeDto>>,
    pub merits: Vec<MeritDto>,
    pub custom_fields: Vec<CustomFieldDto>,
    pub roles: Vec<RoleGrantDto>,
    /// Avslutade roller (Scoutnet: Rollhistorik).
    pub role_history: Vec<RoleHistoryDto>,
    pub placements: Vec<PlacementDto>,
    pub attendance: AttendanceSummary,
    /// Den inloggades behörigheter för just den här personen.
    pub permissions: Vec<Permission>,
    pub is_self: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ContactDto {
    pub email: Option<String>,
    pub alt_email: Option<String>,
    /// Personlig adress @scouterna.se om sådan finns.
    pub scouterna_email: Option<String>,
    pub mobile_phone: Option<String>,
    pub home_phone: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AddressDto {
    /// `home`, `postal`, `invoice`, `temporary`, `other`.
    pub address_type: String,
    pub is_primary: bool,
    pub care_of: Option<String>,
    pub line1: String,
    pub line2: Option<String>,
    pub line3: Option<String>,
    pub postcode: Option<String>,
    pub city: Option<String>,
    pub country_code: String,
    pub valid_from: Option<NaiveDate>,
    pub valid_to: Option<NaiveDate>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MembershipDto {
    pub status: String,
    pub group: RefDto,
    pub member_since: Option<NaiveDate>,
    /// Datum då kåren godkände medlemskapet.
    pub confirmed_at: Option<NaiveDate>,
    pub waiting_since: Option<NaiveDate>,
    pub wished_troop_type: Option<String>,
    /// Kräver `economy_read`.
    pub fee_group: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct GuardianDto {
    pub position: i16,
    pub name: String,
    pub relation: String,
    pub email: Option<String>,
    pub mobile_phone: Option<String>,
    pub home_phone: Option<String>,
    pub has_account: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HouseholdDto {
    pub id: Uuid,
    pub name: String,
    pub members: Vec<HouseholdMemberDto>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HouseholdMemberDto {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub age: i32,
    pub group_name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[expect(clippy::struct_excessive_bools, reason = "oberoende inställningar")]
pub struct PreferencesDto {
    pub newsletter: bool,
    /// Medlemstidningen Scouterna.
    pub magazine: bool,
    /// Adress uppdateras automatiskt från SPAR.
    pub spar_sync: bool,
    /// Utskick går till alternativ e-post.
    pub mailing_to_alt_email: bool,
    /// `email`, `alt_email`, `post`, `efaktura`, `kivra`.
    pub invoice_delivery_membership: String,
    pub invoice_delivery_events: String,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct ConsentDto {
    /// `photo_publication`, `publish_profile`, `publish_roles`.
    pub kind: String,
    pub granted: bool,
    pub decided_at: DateTime<Utc>,
    /// `self`, `guardian`, `registrar`.
    pub decided_by: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PoliceCheckDto {
    /// Krävs p.g.a. ledaruppdrag, roll eller "tvinga registerutdrag".
    pub required: bool,
    pub forced: bool,
    /// `not_required`, `missing`, `expired`, `expiring`, `valid`.
    pub status: String,
    pub shown_on: Option<NaiveDate>,
    pub issued_on: Option<NaiveDate>,
    pub valid_until: Option<NaiveDate>,
    pub history: Vec<PoliceCheckEntryDto>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct PoliceCheckEntryDto {
    pub issued_on: NaiveDate,
    pub shown_on: NaiveDate,
    pub verified_by: Option<String>,
    pub body_name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FollowUpDto {
    pub held_on: NaiveDate,
    pub role_key: Option<String>,
    pub role_label: Option<String>,
    pub body_name: Option<String>,
    pub registered_by: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TermFeeDto {
    pub term: String,
    pub term_id: i32,
    pub group: RefDto,
    /// Belopp i öre.
    pub amount_ore: i32,
    pub fee_group: Option<String>,
    pub due_date: NaiveDate,
    /// OCR/KID-referens.
    pub kid: Option<String>,
    pub paid_on: Option<NaiveDate>,
    /// `paid`, `unpaid`, `overdue`.
    pub status: String,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct MeritDto {
    /// `award`, `badge`, `course`, `certification`.
    pub kind: String,
    pub name: String,
    pub note: Option<String>,
    pub issued_by: Option<String>,
    pub completed_on: NaiveDate,
    pub place: Option<String>,
    pub organizer: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct CustomFieldDto {
    pub key: String,
    pub label: String,
    /// Enheten (oftast kåren) som definierat fältet.
    pub body_name: String,
    pub value: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleHistoryDto {
    pub body: BodyRef,
    pub role_key: String,
    pub role_label: String,
    pub from: NaiveDate,
    /// Sista dagen (inklusive).
    pub to: Option<NaiveDate>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PlacementDto {
    pub body: BodyRef,
    pub from: NaiveDate,
    /// Sista dagen (inklusive), `null` om placeringen pågår.
    pub to: Option<NaiveDate>,
    pub is_leader: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AttendanceSummary {
    pub term: String,
    pub present: i64,
    pub total: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NationalIdResponse {
    pub national_id: String,
}

#[derive(sqlx::FromRow)]
#[expect(clippy::struct_excessive_bools, reason = "speglar tabellens flaggor")]
struct Row {
    id: Uuid,
    member_no: i32,
    first_name: String,
    last_name: String,
    nickname: Option<String>,
    birth_date: NaiveDate,
    age: i32,
    sex: String,
    person_status: String,
    preferred_language: String,
    presentation: Option<String>,
    photo_url: Option<String>,
    created_at: DateTime<Utc>,
    confirmed_at: Option<DateTime<Utc>>,
    email: Option<String>,
    alt_email: Option<String>,
    scouterna_email: Option<String>,
    mobile_phone: Option<String>,
    home_phone: Option<String>,
    mailing_to_alt_email: bool,
    newsletter: bool,
    magazine: bool,
    spar_sync: bool,
    force_police_check: bool,
    invoice_delivery_membership: String,
    invoice_delivery_events: String,
    note: Option<String>,
    national_id: Option<String>,
    protected_identity: bool,
    can_help: bool,
    household_id: Option<Uuid>,
    household_name: Option<String>,
    membership_status: String,
    membership_confirmed_at: Option<NaiveDate>,
    fee_group: Option<String>,
    group_id: Uuid,
    group_name: String,
    member_since: Option<NaiveDate>,
    waiting_since: Option<NaiveDate>,
    wished_troop_type: Option<String>,
    troop_id: Option<Uuid>,
    troop_name: Option<String>,
    troop_type: Option<String>,
    patrol_id: Option<Uuid>,
    patrol_name: Option<String>,
    is_leader: bool,
    effective_path: String,
}

async fn load_row(state: &AppState, id: Uuid) -> ApiResult<Option<Row>> {
    Ok(sqlx::query_as::<_, Row>(
        "SELECT mc.id, mc.member_no, mc.first_name, mc.last_name, mc.nickname, mc.birth_date,
                date_part('year', age(mc.birth_date))::int AS age, mc.sex, mc.person_status, mc.preferred_language,
                mc.presentation, mc.photo_url, mc.created_at, mc.confirmed_at, mc.email, mc.alt_email, mc.scouterna_email,
                mc.mobile_phone, mc.home_phone, mc.mailing_to_alt_email, mc.newsletter, mc.magazine, mc.spar_sync,
                mc.force_police_check, mc.invoice_delivery_membership, mc.invoice_delivery_events, mc.note, mc.national_id,
                mc.protected_identity, mc.can_help, mc.household_id, h.name AS household_name, mc.membership_status,
                mc.membership_confirmed_at, mc.fee_group, mc.group_id, mc.group_name, mc.member_since, mc.waiting_since,
                mc.wished_troop_type, mc.troop_id, mc.troop_name, mc.troop_type, mc.patrol_id, mc.patrol_name, mc.is_leader,
                mc.effective_path::text AS effective_path
         FROM member_current mc LEFT JOIN household h ON h.id = mc.household_id
         WHERE mc.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?)
}

/// Status för registerutdrag. Ett utdrag gäller i tre år; `expiring` om det går ut inom 90 dagar.
fn police_status(required: bool, valid_until: Option<NaiveDate>, today: NaiveDate) -> &'static str {
    match valid_until {
        _ if !required => "not_required",
        None => "missing",
        Some(until) if until < today => "expired",
        Some(until) if today.checked_add_days(Days::new(90)).is_some_and(|soon| until < soon) => "expiring",
        Some(_) => "valid",
    }
}

fn fee_status(paid_on: Option<NaiveDate>, due_date: NaiveDate, today: NaiveDate) -> &'static str {
    match paid_on {
        Some(_) => "paid",
        None if due_date < today => "overdue",
        None => "unpaid",
    }
}

type PoliceStatusRow = (bool, Option<NaiveDate>, Option<NaiveDate>, Option<NaiveDate>);
type FeeRow = (i32, Uuid, String, i32, Option<String>, NaiveDate, Option<String>, Option<NaiveDate>);

/// En roll eller placering på en enhet under en period. `detail` är rollnyckeln för roller.
#[derive(sqlx::FromRow)]
struct BodySpanRow {
    id: Uuid,
    name: String,
    kind: String,
    number: i32,
    detail: String,
    is_leader: bool,
    valid_from: NaiveDate,
    valid_to: Option<NaiveDate>,
}

impl BodySpanRow {
    fn into_ref(self) -> BodyRef {
        BodyRef { id: self.id, name: self.name, kind: self.kind, number: self.number }
    }
}

/// En person med alla fältgrupper. Fält filtreras efter behörighet och varje visning loggas i revisionsloggen.
#[utoipa::path(get, path = "/v1/people/{id}", tag = "members",
    params(("id" = Uuid, Path, description = "Personens id")),
    responses((status = 200, body = PersonDetail), (status = 404, body = crate::error::Problem)))]
#[expect(clippy::too_many_lines, reason = "sammanställer en API-vy, linjär kod")]
pub async fn get_person(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<PersonDetail>> {
    let p = &user.0;
    let row = load_row(&state, id).await?.ok_or(ApiError::NotFound)?;
    let is_self = p.person_id == row.id;
    if !is_self && !p.can(Permission::MembersRead, &row.effective_path) {
        return Err(ApiError::NotFound);
    }
    let path = row.effective_path.as_str();
    let contact = is_self || p.can(Permission::MembersContactRead, path);
    let personal = is_self || p.can(Permission::PersonalRead, path);
    let economy = is_self || p.can(Permission::EconomyRead, path);
    let manage = p.can(Permission::MembersManage, path);
    let today = chrono::Local::now().date_naive();
    let term = Term::containing(today);
    let db = &state.db;

    let (addresses, guardians, household, consents, merits, custom_fields, roles, role_history, placements, attendance) = tokio::try_join!(
        sqlx::query_as::<_, AddressDto>(
            "SELECT address_type, is_primary, care_of, line1, line2, line3, postcode, city, country_code, valid_from, valid_to
             FROM person_address WHERE person_id = $1 AND (NOT $2 OR address_type = 'postal')
             ORDER BY is_primary DESC, address_type",
        )
        .bind(id)
        .bind(row.protected_identity)
        .fetch_all(db),
        sqlx::query_as::<_, GuardianDto>(
            "SELECT position, name, relation, email, mobile_phone, home_phone, account_person_id IS NOT NULL AS has_account
             FROM guardian WHERE child_id = $1 ORDER BY position, name",
        )
        .bind(id)
        .fetch_all(db),
        sqlx::query_as::<_, (Uuid, String, String, i32, Option<String>, Option<String>)>(
            "SELECT p.id, p.first_name, p.last_name, date_part('year', age(p.birth_date))::int, mc.group_name,
                    mc.effective_path::text
             FROM person p LEFT JOIN member_current mc ON mc.id = p.id
             WHERE p.household_id = $1 AND p.id <> $2 ORDER BY p.birth_date",
        )
        .bind(row.household_id)
        .bind(id)
        .fetch_all(db),
        sqlx::query_as::<_, ConsentDto>("SELECT kind, granted, decided_at, decided_by FROM consent WHERE person_id = $1 ORDER BY kind")
            .bind(id)
            .fetch_all(db),
        sqlx::query_as::<_, MeritDto>(
            "SELECT kind, name, note, issued_by, completed_on, place, organizer FROM person_merit
             WHERE person_id = $1 ORDER BY completed_on DESC, name",
        )
        .bind(id)
        .fetch_all(db),
        sqlx::query_as::<_, CustomFieldDto>(
            "SELECT cf.key, cf.label, b.name AS body_name, cv.value
             FROM custom_value cv JOIN custom_field cf ON cf.id = cv.custom_field_id JOIN body b ON b.id = cf.body_id
             WHERE cv.person_id = $1 ORDER BY b.name, cf.label",
        )
        .bind(id)
        .fetch_all(db),
        sqlx::query_as::<_, BodySpanRow>(
            "SELECT b.id, b.name, b.kind, b.number, ra.role_key AS detail, false AS is_leader,
                    lower(ra.valid) AS valid_from, upper(ra.valid) - 1 AS valid_to
             FROM role_assignment ra JOIN body b ON b.id = ra.body_id
             WHERE ra.person_id = $1 AND ra.valid @> current_date ORDER BY nlevel(b.path), b.name",
        )
        .bind(id)
        .fetch_all(db),
        sqlx::query_as::<_, BodySpanRow>(
            "SELECT b.id, b.name, b.kind, b.number, ra.role_key AS detail, false AS is_leader,
                    lower(ra.valid) AS valid_from, upper(ra.valid) - 1 AS valid_to
             FROM role_assignment ra JOIN body b ON b.id = ra.body_id
             WHERE ra.person_id = $1 AND NOT ra.valid @> current_date ORDER BY lower(ra.valid) DESC",
        )
        .bind(id)
        .fetch_all(db),
        sqlx::query_as::<_, BodySpanRow>(
            "SELECT b.id, b.name, b.kind, b.number, '' AS detail, pl.is_leader,
                    lower(pl.valid) AS valid_from, upper(pl.valid) - 1 AS valid_to
             FROM placement pl JOIN body b ON b.id = pl.body_id WHERE pl.person_id = $1 ORDER BY lower(pl.valid) DESC, b.kind",
        )
        .bind(id)
        .fetch_all(db),
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT count(*) FILTER (WHERE a.status = 'present'), count(*)
             FROM attendance a JOIN meeting m ON m.id = a.meeting_id
             WHERE a.person_id = $1 AND m.starts_at >= $2 AND m.starts_at <= $3",
        )
        .bind(id)
        .bind(term.start())
        .bind(term.end())
        .fetch_one(db),
    )?;

    // Känsliga grupper hämtas bara om de får visas.
    let (police, police_history, follow_ups, fees) = tokio::try_join!(
        async {
            if !personal {
                return Ok(None);
            }
            sqlx::query_as::<_, PoliceStatusRow>(
                "SELECT required, shown_on, issued_on, valid_until FROM police_check_status WHERE person_id = $1",
            )
            .bind(id)
            .fetch_optional(db)
            .await
        },
        async {
            if !personal {
                return Ok(Vec::new());
            }
            sqlx::query_as::<_, PoliceCheckEntryDto>(
                "SELECT pc.issued_on, pc.shown_on, v.first_name || ' ' || v.last_name AS verified_by, b.name AS body_name
                 FROM police_check pc LEFT JOIN person v ON v.id = pc.verified_by_person_id LEFT JOIN body b ON b.id = pc.body_id
                 WHERE pc.person_id = $1 ORDER BY pc.shown_on DESC",
            )
            .bind(id)
            .fetch_all(db)
            .await
        },
        async {
            if !personal {
                return Ok(Vec::new());
            }
            sqlx::query_as::<_, (NaiveDate, Option<String>, Option<String>, Option<String>)>(
                "SELECT f.held_on, f.role_key, b.name, r.first_name || ' ' || r.last_name
                 FROM follow_up_talk f LEFT JOIN body b ON b.id = f.body_id LEFT JOIN person r ON r.id = f.registered_by_person_id
                 WHERE f.person_id = $1 ORDER BY f.held_on DESC",
            )
            .bind(id)
            .fetch_all(db)
            .await
        },
        async {
            if !economy {
                return Ok(Vec::new());
            }
            sqlx::query_as::<_, FeeRow>(
                "SELECT tf.term_id, tf.group_id, b.name, tf.amount_ore, tf.fee_group, tf.due_date, tf.kid, tf.paid_on
                 FROM term_fee tf JOIN body b ON b.id = tf.group_id WHERE tf.person_id = $1 ORDER BY tf.term_id DESC",
            )
            .bind(id)
            .fetch_all(db)
            .await
        },
    )?;

    audit(
        db,
        p.account_id,
        "person.read",
        "person",
        id,
        serde_json::json!({ "fields": { "contact": contact, "personal": personal, "economy": economy, "note": manage } }),
    )
    .await;

    let mut permissions: Vec<Permission> = p.permissions_on(path).into_iter().collect();
    if is_self && !permissions.contains(&Permission::MembersRead) {
        permissions.push(Permission::MembersRead);
    }

    let household = row.household_id.zip(row.household_name).map(|(hid, name)| HouseholdDto {
        id: hid,
        name,
        members: household
            .into_iter()
            .filter(|(.., member_path)| {
                is_self || member_path.as_deref().is_some_and(|mp| p.can(Permission::MembersRead, mp))
            })
            .map(|(mid, first_name, last_name, age, group_name, _)| HouseholdMemberDto {
                id: mid,
                first_name,
                last_name,
                age,
                group_name,
            })
            .collect(),
    });

    let police_check = police.map(|(required, shown_on, issued_on, valid_until)| PoliceCheckDto {
        required,
        forced: row.force_police_check,
        status: police_status(required, valid_until, today).into(),
        shown_on,
        issued_on,
        valid_until,
        history: police_history,
    });

    Ok(Json(PersonDetail {
        id: row.id,
        member_no: row.member_no,
        first_name: row.first_name,
        last_name: row.last_name,
        nickname: row.nickname,
        birth_date: row.birth_date,
        age: row.age,
        sex: row.sex,
        person_status: row.person_status,
        preferred_language: row.preferred_language,
        presentation: row.presentation,
        photo_url: row.photo_url,
        created_at: row.created_at,
        confirmed_at: row.confirmed_at,
        protected_identity: row.protected_identity,
        can_help: row.can_help,
        contact: contact.then_some(ContactDto {
            email: row.email,
            alt_email: row.alt_email,
            scouterna_email: row.scouterna_email,
            mobile_phone: row.mobile_phone,
            home_phone: row.home_phone,
        }),
        addresses: personal.then_some(addresses),
        national_id_masked: row
            .national_id
            .filter(|_| personal && !row.protected_identity)
            .map(|n| n.parse::<Personnummer>().map_or_else(|_| "********-****".into(), Personnummer::masked)),
        membership: MembershipDto {
            status: row.membership_status,
            group: RefDto { id: row.group_id, name: row.group_name },
            member_since: row.member_since,
            confirmed_at: row.membership_confirmed_at,
            waiting_since: row.waiting_since,
            wished_troop_type: row.wished_troop_type,
            fee_group: row.fee_group.filter(|_| economy),
        },
        troop: row.troop_id.zip(row.troop_name).map(|(id, name)| TroopRefDto {
            id,
            name,
            troop_type_label: row
                .troop_type
                .as_deref()
                .and_then(|t| t.parse::<TroopType>().ok())
                .map(|t| t.label_sv().into()),
            troop_type: row.troop_type,
        }),
        patrol: row.patrol_id.zip(row.patrol_name).map(|(id, name)| RefDto { id, name }),
        is_leader: row.is_leader,
        guardians: guardians
            .into_iter()
            .map(|g| GuardianDto {
                email: g.email.filter(|_| contact),
                mobile_phone: g.mobile_phone.filter(|_| contact),
                home_phone: g.home_phone.filter(|_| contact),
                ..g
            })
            .collect(),
        household,
        preferences: contact.then_some(PreferencesDto {
            newsletter: row.newsletter,
            magazine: row.magazine,
            spar_sync: row.spar_sync,
            mailing_to_alt_email: row.mailing_to_alt_email,
            invoice_delivery_membership: row.invoice_delivery_membership,
            invoice_delivery_events: row.invoice_delivery_events,
        }),
        consents,
        note: row.note.filter(|_| manage),
        police_check,
        follow_ups: personal.then(|| {
            follow_ups
                .into_iter()
                .map(|(held_on, role_key, body_name, registered_by)| FollowUpDto {
                    held_on,
                    role_label: role_key.as_deref().map(|k| role_label(k).to_owned()),
                    role_key,
                    body_name,
                    registered_by,
                })
                .collect()
        }),
        fees: economy.then(|| {
            fees.into_iter()
                .map(|(term_id, group_id, group_name, amount_ore, fee_group, due_date, kid, paid_on)| TermFeeDto {
                    term: Term::from_id(term_id).to_string(),
                    term_id,
                    group: RefDto { id: group_id, name: group_name },
                    amount_ore,
                    fee_group,
                    due_date,
                    kid,
                    paid_on,
                    status: fee_status(paid_on, due_date, today).into(),
                })
                .collect()
        }),
        merits,
        custom_fields,
        roles: roles
            .into_iter()
            .map(|r| RoleGrantDto {
                role_label: role_label(&r.detail).to_owned(),
                body_id: r.id,
                body_name: r.name,
                body_kind: r.kind,
                body_number: r.number,
                role_key: r.detail,
            })
            .collect(),
        role_history: role_history
            .into_iter()
            .map(|r| RoleHistoryDto {
                role_label: role_label(&r.detail).to_owned(),
                role_key: r.detail.clone(),
                from: r.valid_from,
                to: r.valid_to,
                body: r.into_ref(),
            })
            .collect(),
        placements: placements
            .into_iter()
            .map(|r| PlacementDto { from: r.valid_from, to: r.valid_to, is_leader: r.is_leader, body: r.into_ref() })
            .collect(),
        attendance: AttendanceSummary { term: term.to_string(), present: attendance.0, total: attendance.1 },
        permissions,
        is_self,
    }))
}

/// Visar hela personnumret. Kräver behörigheten `national_id_read` och färsk inloggning (step-up). Loggas alltid.
#[utoipa::path(post, path = "/v1/people/{id}/reveal-national-id", tag = "members",
    params(("id" = Uuid, Path, description = "Personens id")),
    responses((status = 200, body = NationalIdResponse), (status = 401, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn reveal_national_id(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<NationalIdResponse>> {
    let p = &user.0;
    let row = load_row(&state, id).await?.ok_or(ApiError::NotFound)?;
    if !p.can(Permission::MembersRead, &row.effective_path) {
        return Err(ApiError::NotFound);
    }
    if !p.can(Permission::NationalIdRead, &row.effective_path) {
        return Err(ApiError::Forbidden);
    }
    if !p.has_recent_auth() {
        return Err(ApiError::StepUpRequired);
    }
    let national_id = row.national_id.ok_or(ApiError::NotFound)?;
    audit(&state.db, p.account_id, "person.national_id.reveal", "person", id, serde_json::json!({})).await;
    Ok(Json(NationalIdResponse { national_id }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/people/{id}", get(get_person))
        .route("/v1/people/{id}/reveal-national-id", post(reveal_national_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("giltigt datum")
    }

    #[test]
    fn police_status_rules() {
        let today = d(2026, 10, 16);
        assert_eq!(police_status(false, None, today), "not_required");
        assert_eq!(police_status(true, None, today), "missing");
        assert_eq!(police_status(true, Some(d(2026, 10, 15)), today), "expired");
        assert_eq!(police_status(true, Some(d(2026, 12, 1)), today), "expiring");
        assert_eq!(police_status(true, Some(d(2028, 1, 1)), today), "valid");
    }

    #[test]
    fn fee_status_rules() {
        let today = d(2026, 10, 16);
        assert_eq!(fee_status(Some(d(2026, 9, 1)), d(2026, 9, 30), today), "paid");
        assert_eq!(fee_status(None, d(2026, 9, 30), today), "overdue");
        assert_eq!(fee_status(None, d(2026, 10, 31), today), "unpaid");
    }
}
