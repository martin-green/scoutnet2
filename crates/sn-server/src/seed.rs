//! Syntetisk demodata. Inga riktiga personuppgifter:
//! - personnummer är Skatteverkets **testpersonnummer** (födelsenummer 980–999),
//! - e-post ligger på `example.org` (reserverad domän),
//! - telefonnummer ligger i PTS serie för fiktiv användning (070-174 06 05–99),
//! - medlemsnummer börjar på 9 000 001 (utanför dagens Scoutnets serie).

use std::ops::Bound;

use anyhow::Context;
use chrono::{DateTime, Datelike, Days, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc, Weekday};
use sn_domain::{Personnummer, Term, TroopType};
use sqlx::{PgPool, Postgres, QueryBuilder, postgres::types::PgRange};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SeedConfig {
    pub districts: usize,
    pub groups_per_district: usize,
    pub seed: u64,
    pub today: NaiveDate,
}

impl Default for SeedConfig {
    fn default() -> Self {
        Self { districts: 4, groups_per_district: 5, seed: 2026, today: chrono::Local::now().date_naive() }
    }
}

#[derive(Debug, Default)]
pub struct SeedSummary {
    pub bodies: usize,
    pub persons: usize,
    pub memberships: usize,
    pub meetings: usize,
    pub attendance: usize,
    pub personas: Vec<(String, Uuid)>,
}

/// Deterministisk slumpgenerator (`SplitMix64`) så att demodata blir likadan varje gång.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Heltal i `lo..=hi`.
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next_u64() % (hi - lo + 1)
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next_u64() % 100 < percent
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        let len = u64::try_from(xs.len()).expect("liten slice");
        &xs[usize::try_from(self.range(0, len - 1)).expect("index ryms i usize")]
    }
}

const GIRLS: &[&str] = &[
    "Alice", "Maja", "Elsa", "Astrid", "Wilma", "Freja", "Saga", "Ella", "Alma", "Ebba", "Agnes", "Selma", "Olivia",
    "Lilly", "Vera", "Signe", "Stella", "Ellen", "Klara", "Ines", "Juni", "Molly", "Tilda", "Leah", "Nora", "Emma",
    "Linnea", "Sara", "Hanna", "Ida", "Anna", "Maria", "Karin", "Eva", "Lena", "Sofia", "Johanna", "Malin", "Kristina",
    "Helena", "Amira", "Yasmin",
];
const BOYS: &[&str] = &[
    "William",
    "Liam",
    "Noah",
    "Hugo",
    "Lucas",
    "Oliver",
    "Oscar",
    "Elias",
    "Alexander",
    "Adam",
    "Vincent",
    "Charlie",
    "Leo",
    "Ludvig",
    "Nils",
    "Theo",
    "Olle",
    "Matteo",
    "August",
    "Elton",
    "Viggo",
    "Melvin",
    "Isak",
    "Edvin",
    "Love",
    "Sixten",
    "Malte",
    "Gustav",
    "Erik",
    "Lars",
    "Anders",
    "Johan",
    "Per",
    "Karl",
    "Mikael",
    "Magnus",
    "Fredrik",
    "Daniel",
    "Andreas",
    "Jonas",
    "Omar",
    "Yusuf",
];
const LAST: &[&str] = &[
    "Andersson",
    "Johansson",
    "Karlsson",
    "Nilsson",
    "Eriksson",
    "Larsson",
    "Olsson",
    "Persson",
    "Svensson",
    "Gustafsson",
    "Pettersson",
    "Jonsson",
    "Jansson",
    "Hansson",
    "Bengtsson",
    "Jönsson",
    "Lindberg",
    "Jakobsson",
    "Magnusson",
    "Olofsson",
    "Lindström",
    "Lindqvist",
    "Lindgren",
    "Berg",
    "Axelsson",
    "Bergström",
    "Lundberg",
    "Lind",
    "Lundgren",
    "Lundqvist",
    "Mattsson",
    "Berglund",
    "Fredriksson",
    "Sandberg",
    "Henriksson",
    "Forsberg",
    "Sjöberg",
    "Wallin",
    "Ali",
    "Mohammed",
    "Engström",
    "Eklund",
    "Danielsson",
    "Lundin",
    "Håkansson",
    "Björk",
    "Bergman",
    "Gunnarsson",
    "Holm",
    "Wikström",
    "Samuelsson",
    "Isaksson",
    "Fransson",
    "Bergqvist",
    "Nyström",
    "Holmberg",
    "Arvidsson",
    "Söderberg",
    "Nyberg",
    "Hosseini",
    "Nguyen",
    "Kaya",
    "Yilmaz",
    "Virtanen",
];
const DISTRICTS: &[(&str, &str)] = &[
    ("Älvstadens scoutdistrikt", "Göteborg"),
    ("Sjöstadens scoutdistrikt", "Stockholm"),
    ("Slättbygdens scoutdistrikt", "Uppsala"),
    ("Kustlandets scoutdistrikt", "Malmö"),
    ("Norrskogens scoutdistrikt", "Umeå"),
];
const GROUP_PREFIXES: &[&str] = &[
    "Björkdalens",
    "Granbackens",
    "Ekholmens",
    "Lövsjöns",
    "Tallmons",
    "Sjövikens",
    "Hagabergs",
    "Stenkullas",
    "Myrbackas",
    "Lingonbergets",
    "Ugglebos",
    "Rävkullens",
    "Älgmossens",
    "Kvarnbäckens",
    "Solgläntans",
    "Näckrosens",
    "Fyrbåkens",
    "Mossängens",
    "Blåbärsskogens",
    "Vindöns",
    "Klippans",
    "Ljungbackens",
    "Hjortrons",
    "Sälgvikens",
];
const PATROLS: &[&str] =
    &["Ugglan", "Räven", "Haren", "Lodjuret", "Uttern", "Gäddan", "Tranan", "Igelkotten", "Grävlingen", "Mården"];
const STREETS: &[&str] = &[
    "Storgatan",
    "Skolvägen",
    "Björkvägen",
    "Granvägen",
    "Ekvägen",
    "Kyrkogatan",
    "Parkvägen",
    "Ängsvägen",
    "Tallvägen",
    "Lingonstigen",
];

fn troop_names(t: TroopType) -> &'static [&'static str] {
    match t {
        TroopType::Familj => &["Familjescouterna"],
        TroopType::Baver => &["Bävrarna", "Bäverdammen"],
        TroopType::Sparare => &["Ekorrarna", "Spårhundarna", "Myrstacken"],
        TroopType::Upptackare => &["Lodjuren", "Falkarna", "Vargarna"],
        TroopType::Aventyrare => &["Örnarna", "Björnarna", "Vildsvinen"],
        TroopType::Utmanare => &["Utmanarlaget", "Stormfåglarna"],
        TroopType::Rover => &["Roverlaget", "Vandrarna"],
        TroopType::Annan => &["Ledare och vuxna"],
    }
}

struct BodyRec {
    id: Uuid,
    kind: &'static str,
    parent: Option<Uuid>,
    path: String,
    number: i32,
    name: String,
    troop_type: Option<&'static str>,
    min_age: Option<i16>,
    max_age: Option<i16>,
    municipality: Option<&'static str>,
    weekday: Option<i16>,
    start: Option<NaiveTime>,
    duration: Option<i16>,
    location: Option<String>,
}

#[expect(clippy::struct_excessive_bools, reason = "speglar personens flaggor i tabellen")]
struct PersonRec {
    id: Uuid,
    member_no: i32,
    first: String,
    last: String,
    birth: NaiveDate,
    national_id: String,
    sex: &'static str,
    email: Option<String>,
    mobile: Option<String>,
    street: String,
    postcode: String,
    city: &'static str,
    can_help: bool,
    alt_email: Option<String>,
    scouterna_email: Option<String>,
    home_phone: Option<String>,
    mailing_to_alt: bool,
    lang: &'static str,
    status: &'static str,
    presentation: Option<&'static str>,
    note: Option<&'static str>,
    newsletter: bool,
    magazine: bool,
    spar_sync: bool,
    force_police_check: bool,
    invoice_membership: &'static str,
    invoice_events: &'static str,
    household: Option<Uuid>,
    protected: bool,
}

struct MembershipRec {
    person: Uuid,
    group: Uuid,
    status: &'static str,
    valid: PgRange<NaiveDate>,
    waiting_since: Option<NaiveDate>,
    wished: Option<&'static str>,
    end_reason: Option<&'static str>,
    fee_group: Option<&'static str>,
    confirmed_at: Option<NaiveDate>,
}

struct PlacementRec {
    person: Uuid,
    body: Uuid,
    is_leader: bool,
    valid: PgRange<NaiveDate>,
}

struct RoleRec {
    person: Uuid,
    body: Uuid,
    role: &'static str,
    valid: PgRange<NaiveDate>,
}

#[derive(Clone)]
struct GuardianRec {
    child: Uuid,
    name: String,
    email: Option<String>,
    mobile: Option<String>,
    home_phone: Option<String>,
    position: i16,
    relation: &'static str,
    account_person: Option<Uuid>,
}

struct HouseholdRec {
    id: Uuid,
    name: String,
}

struct AddressRec {
    person: Uuid,
    kind: &'static str,
    primary: bool,
    care_of: Option<String>,
    line1: String,
    postcode: Option<String>,
    city: Option<String>,
}

struct TermFeeRec {
    person: Uuid,
    group: Uuid,
    term_id: i32,
    amount_ore: i32,
    fee_group: &'static str,
    due: NaiveDate,
    kid: String,
    paid: Option<NaiveDate>,
}

struct PoliceCheckRec {
    person: Uuid,
    issued: NaiveDate,
    shown: NaiveDate,
    verified_by: Option<Uuid>,
    body: Uuid,
}

struct FollowUpRec {
    person: Uuid,
    held_on: NaiveDate,
    role: &'static str,
    body: Uuid,
    by: Option<Uuid>,
}

struct MeritRec {
    person: Uuid,
    kind: &'static str,
    name: &'static str,
    issued_by: String,
    completed_on: NaiveDate,
    place: Option<String>,
}

struct ConsentRec {
    person: Uuid,
    kind: &'static str,
    granted: bool,
    decided_by: &'static str,
}

struct CustomFieldRec {
    id: Uuid,
    body: Uuid,
    key: &'static str,
    label: &'static str,
}

struct CustomValueRec {
    field: Uuid,
    person: Uuid,
    value: &'static str,
}

struct MeetingRec {
    id: Uuid,
    troop: Uuid,
    starts_at: DateTime<Utc>,
    duration: i32,
    name: String,
    location: Option<String>,
    kind: &'static str,
}

struct AttendanceRec {
    meeting: Uuid,
    person: Uuid,
    status: &'static str,
    is_leader: bool,
}

struct AccountRec {
    id: Uuid,
    person: Uuid,
    persona: String,
}

fn open_from(from: NaiveDate) -> PgRange<NaiveDate> {
    PgRange { start: Bound::Included(from), end: Bound::Unbounded }
}

fn closed(from: NaiveDate, to: NaiveDate) -> PgRange<NaiveDate> {
    PgRange { start: Bound::Included(from), end: Bound::Excluded(to) }
}

fn ascii_slug(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'å' | 'ä' => 'a',
            'ö' => 'o',
            'é' => 'e',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        })
        .collect()
}

/// Sista söndagen i en månad (för svensk sommartid).
fn last_sunday(year: i32, month: u32) -> NaiveDate {
    let first_next =
        if month == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1) } else { NaiveDate::from_ymd_opt(year, month + 1, 1) }
            .expect("giltigt datum");
    let last = first_next.pred_opt().expect("giltigt datum");
    last - Days::new(u64::from(last.weekday().num_days_from_sunday()))
}

/// Svensk lokaltid till UTC (sommartid sista söndagen i mars–oktober).
fn stockholm(date: NaiveDate, time: NaiveTime) -> DateTime<Utc> {
    let summer = date >= last_sunday(date.year(), 3) && date < last_sunday(date.year(), 10);
    let offset = FixedOffset::east_opt(if summer { 7200 } else { 3600 }).expect("giltig offset");
    offset.from_local_datetime(&NaiveDateTime::new(date, time)).single().expect("entydig lokal tid").with_timezone(&Utc)
}

struct Gen {
    rng: Rng,
    today: NaiveDate,
    next_member_no: i32,
    next_label: u64,
    bodies: Vec<BodyRec>,
    persons: Vec<PersonRec>,
    memberships: Vec<MembershipRec>,
    placements: Vec<PlacementRec>,
    roles: Vec<RoleRec>,
    guardians: Vec<GuardianRec>,
    meetings: Vec<MeetingRec>,
    attendance: Vec<AttendanceRec>,
    accounts: Vec<AccountRec>,
    households: Vec<HouseholdRec>,
    addresses: Vec<AddressRec>,
    term_fees: Vec<TermFeeRec>,
    police_checks: Vec<PoliceCheckRec>,
    follow_ups: Vec<FollowUpRec>,
    merits: Vec<MeritRec>,
    consents: Vec<ConsentRec>,
    custom_fields: Vec<CustomFieldRec>,
    custom_values: Vec<CustomValueRec>,
}

impl Gen {
    /// Lägger till en enhet. Utan angivet nummer får den nästa nummer för sin typ
    /// (avdelningar från 5001, patruller från 12001), ungefär som id:n i Scoutnet.
    fn add_body(&mut self, kind: &'static str, parent: Option<usize>, name: String, number: Option<i32>) -> usize {
        self.next_label += 1;
        let label = format!("b{}", self.next_label);
        let (parent_id, path) = match parent {
            Some(i) => (Some(self.bodies[i].id), format!("{}.{label}", self.bodies[i].path)),
            None => (None, label),
        };
        let number = number.unwrap_or_else(|| {
            let base = match kind {
                "troop" => 5000,
                "patrol" => 12000,
                _ => 0,
            };
            let same_kind = self.bodies.iter().filter(|b| b.kind == kind).count();
            base + i32::try_from(same_kind).expect("rimligt antal enheter") + 1
        });
        self.bodies.push(BodyRec {
            id: Uuid::now_v7(),
            kind,
            parent: parent_id,
            path,
            number,
            name,
            troop_type: None,
            min_age: None,
            max_age: None,
            municipality: None,
            weekday: None,
            start: None,
            duration: None,
            location: None,
        });
        self.bodies.len() - 1
    }

    fn random_date_in_year(&mut self, year: i32) -> NaiveDate {
        let start = NaiveDate::from_ymd_opt(year, 1, 1).expect("giltigt datum");
        let d = start + Days::new(self.rng.range(0, 364));
        d.min(self.today)
    }

    /// Skapar en person med given ålder (ålder under innevarande år).
    fn add_person(&mut self, age: u64, city: &'static str, last: Option<&str>) -> usize {
        let female = self.rng.chance(50);
        let first = (*self.rng.pick(if female { GIRLS } else { BOYS })).to_owned();
        let last = last.map_or_else(|| (*self.rng.pick(LAST)).to_owned(), str::to_owned);
        let year = self.today.year() - i32::try_from(age).expect("rimlig ålder");
        let birth = self.random_date_in_year(year);
        // Testpersonnummer: jämnt födelsenummer = kvinna enligt konventionen.
        let idx = u8::try_from(self.rng.range(0, 9) * 2 + u64::from(!female)).expect("index < 20");
        let national_id = Personnummer::test_number(birth, idx).to_string();
        let member_no = self.next_member_no;
        self.next_member_no += 1;
        let adult = age >= 18;
        let email = (adult || (age >= 13 && self.rng.chance(60)))
            .then(|| format!("{}.{}.{member_no}@example.org", ascii_slug(&first), ascii_slug(&last)));
        let mobile = (adult || age >= 12).then(|| format!("070-174 06 {:02}", self.rng.range(5, 99)));
        let sex = if self.rng.chance(2) {
            "other"
        } else if female {
            "female"
        } else {
            "male"
        };
        self.persons.push(PersonRec {
            id: Uuid::now_v7(),
            member_no,
            first,
            last,
            birth,
            national_id,
            sex,
            email,
            mobile,
            street: format!("{} {}", self.rng.pick(STREETS), self.rng.range(1, 80)),
            postcode: format!("{} {:02}", self.rng.range(100, 989), self.rng.range(10, 99)),
            city,
            can_help: adult && self.rng.chance(20),
            alt_email: None,
            scouterna_email: None,
            home_phone: None,
            mailing_to_alt: false,
            lang: "sv",
            status: "active",
            presentation: None,
            note: None,
            newsletter: false,
            magazine: true,
            spar_sync: false,
            force_police_check: false,
            invoice_membership: "email",
            invoice_events: "email",
            household: None,
            protected: false,
        });
        self.persons.len() - 1
    }

    fn add_person_between(&mut self, lo: u64, hi: u64, city: &'static str, last: Option<&str>) -> usize {
        let age = self.rng.range(lo, hi);
        self.add_person(age, city, last)
    }

    fn add_guardians(&mut self, child: usize) {
        let n = if self.rng.chance(70) { 2 } else { 1 };
        let child_last = self.persons[child].last.clone();
        for i in 0..n {
            let first = *self.rng.pick(if i == 0 { GIRLS } else { BOYS });
            let email = self.rng.chance(88).then(|| {
                format!("{}.{}.{}@example.org", ascii_slug(first), ascii_slug(&child_last), self.rng.range(1, 999))
            });
            let mobile = self.rng.chance(95).then(|| format!("070-174 06 {:02}", self.rng.range(5, 99)));
            let home_phone = self.rng.chance(15).then(|| format!("031-390 06 {:02}", self.rng.range(0, 99)));
            self.guardians.push(GuardianRec {
                child: self.persons[child].id,
                name: format!("{first} {child_last}"),
                email,
                mobile,
                home_phone,
                position: i + 1,
                relation: "vårdnadshavare",
                account_person: None,
            });
        }
    }

    fn member_since(&mut self, birth: NaiveDate) -> NaiveDate {
        let earliest = birth.checked_add_days(Days::new(6 * 365)).unwrap_or(birth).min(self.today);
        let since = self.today - Days::new(self.rng.range(30, 7 * 365));
        since.max(earliest)
    }

    fn add_member(&mut self, person: usize, group: usize, status: &'static str) {
        let birth = self.persons[person].birth;
        let since = self.member_since(birth);
        self.memberships.push(MembershipRec {
            person: self.persons[person].id,
            group: self.bodies[group].id,
            status,
            valid: open_from(since),
            waiting_since: None,
            wished: None,
            end_reason: None,
            fee_group: None,
            confirmed_at: None,
        });
    }

    fn place(&mut self, person: usize, body: usize, is_leader: bool, from: NaiveDate) {
        self.placements.push(PlacementRec {
            person: self.persons[person].id,
            body: self.bodies[body].id,
            is_leader,
            valid: open_from(from),
        });
    }

    fn give_role(&mut self, person: usize, body: usize, role: &'static str) {
        let from = self.today - Days::new(self.rng.range(30, 4 * 365));
        self.roles.push(RoleRec {
            person: self.persons[person].id,
            body: self.bodies[body].id,
            role,
            valid: open_from(from),
        });
    }

    fn add_persona(&mut self, person: usize, persona: String) {
        self.accounts.push(AccountRec { id: Uuid::now_v7(), person: self.persons[person].id, persona });
    }
}

struct GroupOut {
    idx: usize,
    board: Vec<usize>,
    troops: Vec<(usize, TroopType, Vec<usize>, Vec<usize>)>,
}

#[expect(clippy::too_many_lines, reason = "en kår genereras i ett flöde, lättare att läsa i ett stycke")]
fn gen_group(g: &mut Gen, district: usize, number: i32, name: &str, city: &'static str, term: Term) -> GroupOut {
    let group = g.add_body("group", Some(district), name.to_owned(), Some(number));
    g.bodies[group].municipality = Some(city);
    let term_start = term.start();

    let mut types = vec![TroopType::Sparare, TroopType::Upptackare, TroopType::Aventyrare, TroopType::Utmanare];
    if g.rng.chance(50) {
        types.insert(0, TroopType::Baver);
    }
    if g.rng.chance(15) {
        types.insert(0, TroopType::Familj);
    }
    if g.rng.chance(50) {
        types.push(TroopType::Rover);
    }

    let mut troops = Vec::new();
    for t in types {
        let troop_name = (*g.rng.pick(troop_names(t))).to_owned();
        let troop = g.add_body("troop", Some(group), troop_name, None);
        let (lo, hi) = t.age_range().expect("åldersgren har åldersintervall");
        {
            let weekday = i16::try_from(g.rng.range(1, 4)).expect("veckodag");
            let start = if g.rng.chance(50) { (18, 0) } else { (18, 30) };
            let rec = &mut g.bodies[troop];
            rec.troop_type = Some(t.as_str());
            rec.min_age = Some(i16::from(lo));
            rec.max_age = Some(i16::from(hi));
            rec.weekday = Some(weekday);
            rec.start = NaiveTime::from_hms_opt(start.0, start.1, 0);
            rec.duration = Some(if matches!(t, TroopType::Rover | TroopType::Utmanare) { 120 } else { 90 });
            rec.location = Some(format!("Scoutstugan, {name}"));
        }
        let patrols: Vec<usize> = if matches!(t, TroopType::Upptackare | TroopType::Aventyrare) {
            let n = g.rng.range(2, 3);
            (0..n)
                .map(|i| {
                    g.add_body(
                        "patrol",
                        Some(troop),
                        format!("Patrull {}", PATROLS[usize::try_from(i).expect("liten") * 3 % PATROLS.len()]),
                        None,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut scouts = Vec::new();
        for i in 0..g.rng.range(8, 22) {
            let p = g.add_person_between(u64::from(lo), u64::from(hi), city, None);
            g.add_member(p, group, "active");
            g.place(p, troop, false, term_start.min(g.today));
            if !patrols.is_empty() {
                let patrol = patrols[usize::try_from(i).expect("liten") % patrols.len()];
                g.place(p, patrol, false, term_start.min(g.today));
            }
            g.add_guardians(p);
            scouts.push(p);
        }
        let mut leaders = Vec::new();
        for i in 0..g.rng.range(2, 4) {
            let p = g.add_person_between(18, 62, city, None);
            g.add_member(p, group, "active");
            g.place(p, troop, true, term_start.min(g.today));
            g.give_role(
                p,
                troop,
                match i {
                    0 => "avdelningsledare",
                    1 | 2 => "ledare",
                    _ => "assisterande_ledare",
                },
            );
            leaders.push(p);
        }
        troops.push((troop, t, scouts, leaders));
    }

    // Kårstyrelse och övriga vuxna.
    let adults = g.add_body("troop", Some(group), "Ledare och vuxna".into(), None);
    g.bodies[adults].troop_type = Some(TroopType::Annan.as_str());
    let mut board = Vec::new();
    for _ in 0..g.rng.range(5, 7) {
        let p = g.add_person_between(25, 65, city, None);
        g.add_member(p, group, "active");
        g.place(p, adults, false, term_start.min(g.today));
        board.push(p);
    }
    g.give_role(board[0], group, "karordforande");
    g.give_role(board[1], group, "medlemsregistrerare");
    g.give_role(board[2], group, "kassor");

    // Väntelista, nya anmälningar och tidigare medlemmar.
    for _ in 0..g.rng.range(3, 25) {
        let age = g.rng.range(6, 12);
        let p = g.add_person(age, city, None);
        let since = g.today - Days::new(g.rng.range(10, 1000));
        g.memberships.push(MembershipRec {
            person: g.persons[p].id,
            group: g.bodies[group].id,
            status: "waiting",
            valid: open_from(since),
            waiting_since: Some(since),
            wished: TroopType::for_age(u8::try_from(age).expect("liten")).map(TroopType::as_str),
            end_reason: None,
            fee_group: None,
            confirmed_at: None,
        });
        g.add_guardians(p);
    }
    for _ in 0..g.rng.range(0, 3) {
        let p = g.add_person_between(7, 14, city, None);
        let since = g.today - Days::new(g.rng.range(1, 30));
        g.memberships.push(MembershipRec {
            person: g.persons[p].id,
            group: g.bodies[group].id,
            status: "pending",
            valid: open_from(since),
            waiting_since: None,
            wished: None,
            end_reason: None,
            fee_group: None,
            confirmed_at: None,
        });
        g.add_guardians(p);
    }
    for _ in 0..g.rng.range(5, 20) {
        let p = g.add_person_between(10, 30, city, None);
        let to = g.today - Days::new(g.rng.range(60, 1500));
        let from = to - Days::new(g.rng.range(200, 2500));
        g.memberships.push(MembershipRec {
            person: g.persons[p].id,
            group: g.bodies[group].id,
            status: "ended",
            valid: closed(from, to),
            waiting_since: None,
            wished: None,
            end_reason: Some(*g.rng.pick(&["Avregistrerad p.g.a. ålder", "Slutat", "Flyttat"])),
            fee_group: None,
            confirmed_at: None,
        });
    }

    GroupOut { idx: group, board, troops }
}

fn gen_meetings(g: &mut Gen, troops: &[(usize, TroopType, Vec<usize>, Vec<usize>)], term: Term) {
    // Närvaro finns för möten före i dag. Gränsen följer datumet, inte klockan, så att samma dag
    // alltid ger samma data (annars ändras slumpföljden när dagens möten passeras).
    let now = stockholm(g.today, NaiveTime::MIN);
    let (first, last) = if term.half() == sn_domain::Half::Autumn {
        (NaiveDate::from_ymd_opt(term.year(), 8, 18), NaiveDate::from_ymd_opt(term.year(), 12, 12))
    } else {
        (NaiveDate::from_ymd_opt(term.year(), 1, 15), NaiveDate::from_ymd_opt(term.year(), 6, 5))
    };
    let (first, last) = (first.expect("datum"), last.expect("datum"));
    for (troop, t, scouts, leaders) in troops {
        let b = &g.bodies[*troop];
        let (Some(weekday), Some(start), Some(duration)) = (b.weekday, b.start, b.duration) else { continue };
        let (troop_id, location) = (b.id, b.location.clone());
        let target = Weekday::try_from(u8::try_from(weekday - 1).expect("veckodag")).expect("veckodag");
        let mut day = first;
        while day.weekday() != target {
            day = day.succ_opt().expect("datum");
        }
        let hike_week = g.rng.range(6, 10);
        let mut week = 0;
        while day <= last {
            week += 1;
            let (starts_at, minutes, name, kind) =
                if week == hike_week && !matches!(t, TroopType::Familj | TroopType::Baver) {
                    let saturday = day + Days::new(u64::from(6 - day.weekday().num_days_from_sunday().min(6)));
                    (
                        stockholm(saturday, NaiveTime::from_hms_opt(9, 0, 0).expect("tid")),
                        420,
                        "Dagshajk".to_owned(),
                        "hike",
                    )
                } else {
                    (stockholm(day, start), i32::from(duration), format!("Möte v.{}", day.iso_week().week()), "meeting")
                };
            let meeting = Uuid::now_v7();
            g.meetings.push(MeetingRec {
                id: meeting,
                troop: troop_id,
                starts_at,
                duration: minutes,
                name,
                location: location.clone(),
                kind,
            });
            if starts_at < now {
                for &p in scouts {
                    let roll = g.rng.range(0, 99);
                    let status = if roll < 78 {
                        "present"
                    } else if roll < 93 {
                        "absent"
                    } else {
                        "excused"
                    };
                    g.attendance.push(AttendanceRec { meeting, person: g.persons[p].id, status, is_leader: false });
                }
                for &p in leaders {
                    let status = if g.rng.chance(90) { "present" } else { "excused" };
                    g.attendance.push(AttendanceRec { meeting, person: g.persons[p].id, status, is_leader: true });
                }
            }
            day = day + Days::new(7);
        }
    }
}

const PRESENTATIONS: &[&str] = &[
    "Har varit scout sedan jag var liten och leder nu en avdelning.",
    "Gillar friluftsliv, paddling och vinterhajker.",
    "Ansvarar för kårens båtar och sjösäkerhet.",
    "Spelar gitarr vid lägerelden och fixar gärna hajkmat.",
];
// Interna anteckningar får aldrig innehålla hälsouppgifter.
const NOTES: &[&str] = &[
    "Har nyckel till scoutstugan",
    "Hjälper till med båtarna",
    "Föredrar kontakt via sms",
    "Kontaktperson i föräldraföreningen",
];
const BADGES: &[&str] = &["Knopar", "Kniv", "Yxa", "Eld", "Kompass", "Sjukvård", "Simborgarmärket", "Hajk"];
const COURSES: &[(&str, &str, u64)] = &[
    ("course", "Trygga möten", 90),
    ("course", "Leda scouting", 60),
    ("course", "Leda avdelning", 45),
    ("course", "Första hjälpen – HLR", 40),
    ("certification", "Båtförarintyg", 10),
];
const COURSE_PLACES: &[&str] = &["Scouternas folkhögskola", "Distriktets ledarhelg", "Digitalt"];

/// OCR-referens (KID) med Luhn-kontrollsiffra, som i Scoutnets avgiftsavier.
fn kid(member_no: i32, term_id: i32) -> String {
    let base = format!("{member_no}{term_id}");
    let sum: u32 = base
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, b)| {
            let d = u32::from(b - b'0') * if i % 2 == 0 { 2 } else { 1 };
            d / 10 + d % 10
        })
        .sum();
    format!("{base}{}", (10 - sum % 10) % 10)
}

/// Fyller i alla personfält som dagens Scoutnet har, utifrån redan genererade personer och roller.
#[expect(clippy::too_many_lines, reason = "berikningen är en linjär genomgång av fältkatalogen")]
fn enrich(g: &mut Gen, term: Term) {
    use std::collections::{HashMap, HashSet};

    let today = g.today;
    let index: HashMap<Uuid, usize> = g.persons.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let leaders: HashSet<Uuid> = g.placements.iter().filter(|p| p.is_leader).map(|p| p.person).collect();
    let role_of: HashMap<Uuid, (Uuid, &'static str)> = g.roles.iter().map(|r| (r.person, (r.body, r.role))).collect();
    let group_name: HashMap<Uuid, String> =
        g.bodies.iter().filter(|b| b.kind == "group").map(|b| (b.id, b.name.clone())).collect();
    let chair_of: HashMap<Uuid, Uuid> =
        g.roles.iter().filter(|r| r.role == "karordforande").map(|r| (r.body, r.person)).collect();
    let mut statuses: HashMap<Uuid, Vec<&'static str>> = HashMap::new();
    let mut group_of: HashMap<Uuid, (Uuid, NaiveDate)> = HashMap::new();
    for m in &g.memberships {
        statuses.entry(m.person).or_default().push(m.status);
        if m.status == "active"
            && let Bound::Included(since) = m.valid.start
        {
            group_of.insert(m.person, (m.group, since));
        }
    }

    // Personfält: kontakt, språk, preferenser, status.
    for p in &mut g.persons {
        let age = today.year() - p.birth.year();
        let adult = age >= 18;
        let functionary = leaders.contains(&p.id) || role_of.contains_key(&p.id);
        if adult && g.rng.chance(25) {
            p.alt_email = Some(format!("{}.{}@example.net", ascii_slug(&p.first), ascii_slug(&p.last)));
            p.mailing_to_alt = g.rng.chance(50);
        }
        if functionary {
            p.scouterna_email =
                Some(format!("{}.{}{}@scoutkar.example", ascii_slug(&p.first), ascii_slug(&p.last), p.member_no % 100));
        }
        if g.rng.chance(if adult { 25 } else { 10 }) {
            p.home_phone = Some(format!("031-390 06 {:02}", g.rng.range(0, 99)));
        }
        if g.rng.chance(4) {
            p.lang = "en";
        }
        p.newsletter = adult && g.rng.chance(45);
        p.magazine = g.rng.chance(90);
        p.spar_sync = g.rng.chance(30);
        if functionary && g.rng.chance(40) {
            p.presentation = Some(*g.rng.pick(PRESENTATIONS));
        }
        if g.rng.chance(3) {
            p.note = Some(*g.rng.pick(NOTES));
        }
        p.force_police_check = adult && !functionary && g.rng.chance(4);
        p.invoice_membership = match g.rng.range(0, 99) {
            0..70 => "email",
            70..82 => "efaktura",
            82..92 => "kivra",
            92..97 => "post",
            _ => "alt_email",
        };
        p.invoice_events = if g.rng.chance(85) { "email" } else { "post" };
        p.protected = !adult && group_of.contains_key(&p.id) && g.rng.chance(1);
        let st = statuses.get(&p.id).map_or(&[][..], Vec::as_slice);
        p.status = if st.contains(&"active") || st.contains(&"waiting") {
            "active"
        } else if st.contains(&"pending") {
            "new"
        } else if st.contains(&"ended") {
            "unregistered"
        } else {
            "active_non_member"
        };
    }

    // Hushåll: syskon i samma kår (samma efternamn) får gemensamt hushåll och vårdnadshavare.
    let mut buckets: HashMap<(Uuid, String), Vec<usize>> = HashMap::new();
    for (pid, (group, _)) in &group_of {
        let i = index[pid];
        let p = &g.persons[i];
        if today.year() - p.birth.year() < 18 && !leaders.contains(pid) {
            buckets.entry((*group, p.last.clone())).or_default().push(i);
        }
    }
    let mut siblings: HashSet<Uuid> = HashSet::new();
    let mut keys: Vec<_> = buckets.keys().cloned().collect();
    keys.sort();
    for key in keys {
        let mut members = buckets[&key].clone();
        members.sort_unstable();
        if members.len() < 2 || !g.rng.chance(70) {
            continue;
        }
        members.truncate(3);
        let household = Uuid::now_v7();
        g.households.push(HouseholdRec { id: household, name: format!("Familjen {}", key.1) });
        let first_id = g.persons[members[0]].id;
        let template: Vec<GuardianRec> = g.guardians.iter().filter(|gu| gu.child == first_id).cloned().collect();
        for &m in &members {
            g.persons[m].household = Some(household);
        }
        for &m in &members[1..] {
            let id = g.persons[m].id;
            siblings.insert(id);
            g.guardians.retain(|gu| gu.child != id);
            g.guardians.extend(template.iter().map(|gu| GuardianRec { child: id, ..gu.clone() }));
        }
    }

    // Adresser. Skyddad identitet: endast förmedlingsadress, aldrig folkbokföringsadress.
    for p in &g.persons {
        if p.protected {
            g.addresses.push(AddressRec {
                person: p.id,
                kind: "postal",
                primary: true,
                care_of: Some("Förmedlingsadress via Skatteverket".into()),
                line1: "Särskild postadress".into(),
                postcode: None,
                city: None,
            });
            continue;
        }
        g.addresses.push(AddressRec {
            person: p.id,
            kind: "home",
            primary: true,
            care_of: None,
            line1: p.street.clone(),
            postcode: Some(p.postcode.clone()),
            city: Some(p.city.to_owned()),
        });
        if today.year() - p.birth.year() >= 18 && g.rng.chance(6) {
            g.addresses.push(AddressRec {
                person: p.id,
                kind: "postal",
                primary: false,
                care_of: Some(format!("c/o {}", g.rng.pick(LAST))),
                line1: format!("{} {}", g.rng.pick(STREETS), g.rng.range(1, 80)),
                postcode: Some(format!("{} {:02}", g.rng.range(100, 989), g.rng.range(10, 99))),
                city: Some(p.city.to_owned()),
            });
        }
    }

    // Medlemskap: avgiftsgrupp och godkännandedatum. Terminsavgifter med OCR/KID.
    let mut base_fee: HashMap<Uuid, i32> = HashMap::new();
    for m in &mut g.memberships {
        if m.status != "active" {
            continue;
        }
        let Bound::Included(since) = m.valid.start else { continue };
        m.confirmed_at = Some((since + Days::new(g.rng.range(0, 14))).min(today));
        m.fee_group = Some(if leaders.contains(&m.person) || role_of.contains_key(&m.person) {
            "Ledare"
        } else if siblings.contains(&m.person) {
            "Syskonrabatt"
        } else {
            "Ordinarie"
        });
    }
    for m in &g.memberships {
        let (Some(fee_group), Bound::Included(since)) = (m.fee_group, m.valid.start) else { continue };
        let base =
            *base_fee.entry(m.group).or_insert_with(|| 300 + i32::try_from(g.rng.range(0, 8)).expect("litet") * 50);
        let kronor = match fee_group {
            "Ledare" => 90,
            "Syskonrabatt" => base - 100 + 90,
            _ => base + 90,
        };
        let member_no = g.persons[index[&m.person]].member_no;
        for (t, current) in [(term.previous(), false), (term, true)] {
            if since > t.end() {
                continue;
            }
            let due = t.start() + Days::new(45);
            let paid = if current { g.rng.chance(if due < today { 78 } else { 45 }) } else { g.rng.chance(93) };
            let paid_on = paid.then(|| (t.start() + Days::new(g.rng.range(5, 40))).min(today));
            g.term_fees.push(TermFeeRec {
                person: m.person,
                group: m.group,
                term_id: t.id(),
                amount_ore: kronor * 100,
                fee_group,
                due,
                kid: kid(member_no, t.id()),
                paid: paid_on,
            });
        }
    }

    // Registerutdrag, uppföljningssamtal och rollhistorik för ledare och funktionärer.
    let required: Vec<Uuid> = g
        .persons
        .iter()
        .filter(|p| leaders.contains(&p.id) || role_of.contains_key(&p.id) || p.force_police_check)
        .map(|p| p.id)
        .collect();
    for pid in required {
        let Some(&(group, _)) = group_of.get(&pid) else { continue };
        let verifier = chair_of.get(&group).copied().filter(|c| *c != pid);
        if g.rng.chance(85) {
            let shown = today - Days::new(g.rng.range(10, 1400));
            let issued = shown - Days::new(g.rng.range(1, 60));
            g.police_checks.push(PoliceCheckRec { person: pid, issued, shown, verified_by: verifier, body: group });
        }
        if leaders.contains(&pid) {
            if g.rng.chance(30) {
                let (body, role) = role_of.get(&pid).copied().unwrap_or((group, "ledare"));
                g.follow_ups.push(FollowUpRec {
                    person: pid,
                    held_on: today - Days::new(g.rng.range(30, 700)),
                    role,
                    body,
                    by: verifier,
                });
            }
            if g.rng.chance(25)
                && let Some(&(body, _)) = role_of.get(&pid)
            {
                let to = today - Days::new(g.rng.range(100, 700));
                let from = to - Days::new(g.rng.range(200, 1200));
                g.roles.push(RoleRec { person: pid, body, role: "assisterande_ledare", valid: closed(from, to) });
            }
        }
    }

    // Märken, kurser, certifieringar och utmärkelser.
    for p in &g.persons {
        let Some(&(group, since)) = group_of.get(&p.id) else { continue };
        let issuer = group_name.get(&group).cloned().unwrap_or_default();
        if leaders.contains(&p.id) || role_of.contains_key(&p.id) {
            for &(kind, name, pct) in COURSES {
                if g.rng.chance(pct) {
                    let completed_on = today - Days::new(g.rng.range(30, 2500));
                    let place = Some((*g.rng.pick(COURSE_PLACES)).to_owned());
                    g.merits.push(MeritRec {
                        person: p.id,
                        kind,
                        name,
                        issued_by: "Scouterna".into(),
                        completed_on,
                        place,
                    });
                }
            }
            if g.rng.chance(8) {
                g.merits.push(MeritRec {
                    person: p.id,
                    kind: "award",
                    name: "Scouternas förtjänstmärke",
                    issued_by: issuer.clone(),
                    completed_on: today - Days::new(g.rng.range(30, 2000)),
                    place: None,
                });
            }
            if (today - since).num_days() > 5 * 365 && g.rng.chance(30) {
                g.merits.push(MeritRec {
                    person: p.id,
                    kind: "award",
                    name: "Femårsmärket",
                    issued_by: issuer.clone(),
                    completed_on: since + Days::new(5 * 365),
                    place: None,
                });
            }
        } else if today.year() - p.birth.year() < 26 {
            let n = g.rng.range(0, 3);
            let start = usize::try_from(g.rng.range(0, 7)).expect("litet");
            for k in 0..usize::try_from(n).expect("litet") {
                let completed_on = (today - Days::new(g.rng.range(20, 1000))).max(since);
                g.merits.push(MeritRec {
                    person: p.id,
                    kind: "badge",
                    name: BADGES[(start + k) % BADGES.len()],
                    issued_by: issuer.clone(),
                    completed_on,
                    place: None,
                });
            }
        }
    }

    // Samtycken.
    for p in &g.persons {
        if !group_of.contains_key(&p.id) {
            continue;
        }
        let minor = today.year() - p.birth.year() < 18;
        let by = if minor { "guardian" } else { "self" };
        g.consents.push(ConsentRec {
            person: p.id,
            kind: "photo_publication",
            granted: g.rng.chance(82),
            decided_by: by,
        });
        if leaders.contains(&p.id) {
            g.consents.push(ConsentRec {
                person: p.id,
                kind: "publish_profile",
                granted: g.rng.chance(70),
                decided_by: "self",
            });
        }
        if role_of.contains_key(&p.id) {
            g.consents.push(ConsentRec {
                person: p.id,
                kind: "publish_roles",
                granted: g.rng.chance(90),
                decided_by: "self",
            });
        }
    }

    // Kårspecifika anpassade fält.
    let groups: Vec<Uuid> = g.bodies.iter().filter(|b| b.kind == "group").map(|b| b.id).collect();
    for body in groups {
        let shirt = Uuid::now_v7();
        let swim = Uuid::now_v7();
        g.custom_fields.push(CustomFieldRec { id: shirt, body, key: "troja", label: "Tröjstorlek" });
        g.custom_fields.push(CustomFieldRec { id: swim, body, key: "simkunnig", label: "Kan simma 200 m" });
        for p in &g.persons {
            let Some(&(group, _)) = group_of.get(&p.id) else { continue };
            let age = today.year() - p.birth.year();
            if group != body || age >= 18 || !g.rng.chance(70) {
                continue;
            }
            let size = match age {
                ..10 => *g.rng.pick(&["130", "140"]),
                10..13 => *g.rng.pick(&["150", "160"]),
                _ => *g.rng.pick(&["S", "M", "L"]),
            };
            g.custom_values.push(CustomValueRec { field: shirt, person: p.id, value: size });
            g.custom_values.push(CustomValueRec {
                field: swim,
                person: p.id,
                value: if g.rng.chance(85) { "Ja" } else { "Nej" },
            });
        }
    }
}

fn generate(cfg: &SeedConfig) -> Gen {
    let mut g = Gen {
        rng: Rng(cfg.seed),
        today: cfg.today,
        next_member_no: 9_000_001,
        next_label: 0,
        bodies: Vec::new(),
        persons: Vec::new(),
        memberships: Vec::new(),
        placements: Vec::new(),
        roles: Vec::new(),
        guardians: Vec::new(),
        meetings: Vec::new(),
        attendance: Vec::new(),
        accounts: Vec::new(),
        households: Vec::new(),
        addresses: Vec::new(),
        term_fees: Vec::new(),
        police_checks: Vec::new(),
        follow_ups: Vec::new(),
        merits: Vec::new(),
        consents: Vec::new(),
        custom_fields: Vec::new(),
        custom_values: Vec::new(),
    };
    let term = Term::containing(cfg.today);
    let org = g.add_body("organisation", None, "Scouterna (demo)".into(), Some(1));

    let mut first_group: Option<(GroupOut, String)> = None;
    let mut group_no = 1000;
    for d in 0..cfg.districts {
        let (dname, city) = DISTRICTS[d % DISTRICTS.len()];
        let dname = if d < DISTRICTS.len() { dname.to_owned() } else { format!("{dname} {}", d / DISTRICTS.len() + 1) };
        let district = g.add_body("district", Some(org), dname.clone(), Some(i32::try_from(d + 1).expect("litet")));
        g.bodies[district].municipality = Some(city);
        let mut district_chair_set = false;
        for gi in 0..cfg.groups_per_district {
            group_no += 1;
            let prefix = GROUP_PREFIXES[(d * cfg.groups_per_district + gi) % GROUP_PREFIXES.len()];
            let round = (d * cfg.groups_per_district + gi) / GROUP_PREFIXES.len();
            let name =
                if round == 0 { format!("{prefix} scoutkår") } else { format!("{prefix} scoutkår {}", round + 1) };
            let out = gen_group(&mut g, district, group_no, &name, city, term);
            gen_meetings(&mut g, &out.troops, term);
            if !district_chair_set {
                g.give_role(out.board[3], district, "distriktsordforande");
                district_chair_set = true;
                if d == 0 {
                    g.add_persona(out.board[3], format!("Distriktsordförande – {dname}"));
                }
            }
            if first_group.is_none() {
                first_group = Some((out, name));
            }
        }
    }

    // Personas för snabbinloggning i demon, alla i första kåren.
    if let Some((out, name)) = first_group {
        let admin = out.board[4];
        g.give_role(admin, org, "forbundsadmin");
        g.add_persona(admin, "Förbundsadministratör (hela Scouterna)".into());
        g.add_persona(out.board[0], format!("Kårordförande – {name}"));
        g.add_persona(out.board[1], format!("Medlemsregistrerare – {name}"));
        if let Some((troop, _, _, leaders)) = out.troops.iter().find(|t| t.1 == TroopType::Sparare) {
            g.add_persona(leaders[0], format!("Avdelningsledare – {} ({name})", g.bodies[*troop].name));
        }
        if let Some((troop, _, _, leaders)) = out.troops.iter().find(|t| t.1 == TroopType::Aventyrare)
            && let Some(&leader) = leaders.get(1)
        {
            g.add_persona(leader, format!("Ledare – {} ({name})", g.bodies[*troop].name));
        }
        if let Some((_, _, scouts, _)) = out.troops.iter().find(|t| t.1 == TroopType::Utmanare)
            && let Some(&scout) = scouts.first()
        {
            g.add_persona(scout, format!("Scout (utmanare) – {name}"));
        }
        // Vårdnadshavare som egen person (inte medlem) med konto, kopplad till två barn.
        if let Some((_, _, scouts, _)) = out.troops.iter().find(|t| t.1 == TroopType::Sparare)
            && scouts.len() >= 2
        {
            let city = g.persons[scouts[0]].city;
            let last = g.persons[scouts[0]].last.clone();
            let parent = g.add_person_between(32, 48, city, Some(&last));
            for &child in &scouts[..1] {
                let child_id = g.persons[child].id;
                if let Some(gu) = g.guardians.iter_mut().find(|gu| gu.child == child_id) {
                    gu.account_person = Some(g.persons[parent].id);
                    gu.name = format!("{} {}", g.persons[parent].first, g.persons[parent].last);
                }
            }
            g.add_persona(parent, format!("Vårdnadshavare – {name}"));
        }
        g.bodies[out.idx].municipality = Some("Göteborg");
    }
    enrich(&mut g, term);
    g
}

macro_rules! insert_rows {
    ($tx:expr, $head:literal, $rows:expr, $cols:expr, |$b:ident, $r:ident| $body:block) => {{
        let chunk = (60_000 / $cols).max(1);
        for part in $rows.chunks(chunk) {
            let mut qb = QueryBuilder::<Postgres>::new($head);
            qb.push_values(part, |mut $b, $r| $body);
            qb.build().execute(&mut *$tx).await.with_context(|| concat!("insert: ", $head))?;
        }
    }};
}

/// Realistiska tidsstämplar: personen registrerades när hen först kom in i kåren,
/// och samtycken lämnades i samband med det.
async fn realistic_timestamps(tx: &mut sqlx::Transaction<'_, Postgres>) -> anyhow::Result<()> {
    // Kommunkod (SCB) och förenings-ID för bidragsunderlag (DAK). Förenings-ID = kårnummer i demon.
    sqlx::query(
        "UPDATE body SET municipality_code = CASE municipality
             WHEN 'Göteborg' THEN '1480' WHEN 'Stockholm' THEN '0180' WHEN 'Uppsala' THEN '0380'
             WHEN 'Malmö' THEN '1280' WHEN 'Umeå' THEN '2480' END,
           association_id = CASE WHEN kind = 'group' THEN number::text END
         WHERE municipality IS NOT NULL",
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE person p SET created_at = s.first::timestamptz + interval '19 hours'
         FROM (SELECT person_id, min(coalesce(waiting_since, lower(valid))) AS first FROM membership GROUP BY person_id) s
         WHERE s.person_id = p.id",
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE consent c SET decided_at = p.created_at + make_interval(days => abs(hashtext(c.person_id::text || c.kind)) % 20)
         FROM person p WHERE p.id = c.person_id",
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Tömmer databasen och fyller den med syntetisk demodata.
#[expect(clippy::too_many_lines, reason = "en linjär lista av inserts, en per tabell")]
pub async fn run(pool: &PgPool, cfg: &SeedConfig) -> anyhow::Result<SeedSummary> {
    let g = generate(cfg);
    let mut tx = pool.begin().await?;
    sqlx::query(
        "TRUNCATE audit_event, session, account, attendance, meeting, custom_value, custom_field, consent, person_merit,
                  follow_up_talk, police_check, term_fee, person_address, guardian, role_assignment, placement, membership,
                  person, household, body CASCADE",
    )
    .execute(&mut *tx)
    .await?;

    insert_rows!(
        tx,
        "INSERT INTO body (id, kind, parent_id, path, number, name, troop_type, min_age, max_age, municipality, meeting_weekday, meeting_start, meeting_duration_min, meeting_location) ",
        g.bodies,
        14,
        |b, r| {
            b.push_bind(r.id)
                .push_bind(r.kind)
                .push_bind(r.parent)
                .push("")
                .push_bind_unseparated(&r.path)
                .push_unseparated("::ltree")
                .push_bind(r.number)
                .push_bind(&r.name)
                .push_bind(r.troop_type)
                .push_bind(r.min_age)
                .push_bind(r.max_age)
                .push_bind(r.municipality)
                .push_bind(r.weekday)
                .push_bind(r.start)
                .push_bind(r.duration)
                .push_bind(&r.location);
        }
    );
    insert_rows!(tx, "INSERT INTO household (id, name) ", g.households, 2, |b, r| {
        b.push_bind(r.id).push_bind(&r.name);
    });
    insert_rows!(tx, "INSERT INTO person (id, member_no, first_name, last_name, birth_date, national_id, sex, email, mobile_phone, can_help,
                      alt_email, scouterna_email, home_phone, mailing_to_alt_email, preferred_language, person_status, presentation, note,
                      newsletter, magazine, spar_sync, force_police_check, invoice_delivery_membership, invoice_delivery_events,
                      household_id, protected_identity) ", g.persons, 26, |b, r| {
        b.push_bind(r.id).push_bind(r.member_no).push_bind(&r.first).push_bind(&r.last).push_bind(r.birth).push_bind(&r.national_id)
            .push_bind(r.sex).push_bind(&r.email).push_bind(&r.mobile).push_bind(r.can_help)
            .push_bind(&r.alt_email).push_bind(&r.scouterna_email).push_bind(&r.home_phone).push_bind(r.mailing_to_alt)
            .push_bind(r.lang).push_bind(r.status).push_bind(r.presentation).push_bind(r.note)
            .push_bind(r.newsletter).push_bind(r.magazine).push_bind(r.spar_sync).push_bind(r.force_police_check)
            .push_bind(r.invoice_membership).push_bind(r.invoice_events).push_bind(r.household).push_bind(r.protected);
    });
    insert_rows!(
        tx,
        "INSERT INTO person_address (person_id, address_type, is_primary, care_of, line1, postcode, city) ",
        g.addresses,
        7,
        |b, r| {
            b.push_bind(r.person)
                .push_bind(r.kind)
                .push_bind(r.primary)
                .push_bind(&r.care_of)
                .push_bind(&r.line1)
                .push_bind(&r.postcode)
                .push_bind(&r.city);
        }
    );
    insert_rows!(
        tx,
        "INSERT INTO membership (person_id, group_id, status, valid, waiting_since, wished_troop_type, end_reason, fee_group, confirmed_at) ",
        g.memberships,
        9,
        |b, r| {
            b.push_bind(r.person)
                .push_bind(r.group)
                .push_bind(r.status)
                .push_bind(r.valid)
                .push_bind(r.waiting_since)
                .push_bind(r.wished)
                .push_bind(r.end_reason)
                .push_bind(r.fee_group)
                .push_bind(r.confirmed_at);
        }
    );
    insert_rows!(tx, "INSERT INTO placement (person_id, body_id, is_leader, valid) ", g.placements, 4, |b, r| {
        b.push_bind(r.person).push_bind(r.body).push_bind(r.is_leader).push_bind(r.valid);
    });
    insert_rows!(tx, "INSERT INTO role_assignment (person_id, body_id, role_key, valid) ", g.roles, 4, |b, r| {
        b.push_bind(r.person).push_bind(r.body).push_bind(r.role).push_bind(r.valid);
    });
    insert_rows!(
        tx,
        "INSERT INTO guardian (child_id, name, email, mobile_phone, home_phone, position, relation, account_person_id) ",
        g.guardians,
        8,
        |b, r| {
            b.push_bind(r.child)
                .push_bind(&r.name)
                .push_bind(&r.email)
                .push_bind(&r.mobile)
                .push_bind(&r.home_phone)
                .push_bind(r.position)
                .push_bind(r.relation)
                .push_bind(r.account_person);
        }
    );
    insert_rows!(
        tx,
        "INSERT INTO term_fee (person_id, group_id, term_id, amount_ore, fee_group, due_date, kid, paid_on) ",
        g.term_fees,
        8,
        |b, r| {
            b.push_bind(r.person)
                .push_bind(r.group)
                .push_bind(r.term_id)
                .push_bind(r.amount_ore)
                .push_bind(r.fee_group)
                .push_bind(r.due)
                .push_bind(&r.kid)
                .push_bind(r.paid);
        }
    );
    insert_rows!(
        tx,
        "INSERT INTO police_check (person_id, issued_on, shown_on, verified_by_person_id, body_id) ",
        g.police_checks,
        5,
        |b, r| {
            b.push_bind(r.person).push_bind(r.issued).push_bind(r.shown).push_bind(r.verified_by).push_bind(r.body);
        }
    );
    insert_rows!(
        tx,
        "INSERT INTO follow_up_talk (person_id, held_on, role_key, body_id, registered_by_person_id) ",
        g.follow_ups,
        5,
        |b, r| {
            b.push_bind(r.person).push_bind(r.held_on).push_bind(r.role).push_bind(r.body).push_bind(r.by);
        }
    );
    insert_rows!(
        tx,
        "INSERT INTO person_merit (person_id, kind, name, issued_by, completed_on, place) ",
        g.merits,
        6,
        |b, r| {
            b.push_bind(r.person)
                .push_bind(r.kind)
                .push_bind(r.name)
                .push_bind(&r.issued_by)
                .push_bind(r.completed_on)
                .push_bind(&r.place);
        }
    );
    insert_rows!(tx, "INSERT INTO consent (person_id, kind, granted, decided_by) ", g.consents, 4, |b, r| {
        b.push_bind(r.person).push_bind(r.kind).push_bind(r.granted).push_bind(r.decided_by);
    });
    insert_rows!(tx, "INSERT INTO custom_field (id, body_id, key, label) ", g.custom_fields, 4, |b, r| {
        b.push_bind(r.id).push_bind(r.body).push_bind(r.key).push_bind(r.label);
    });
    insert_rows!(tx, "INSERT INTO custom_value (custom_field_id, person_id, value) ", g.custom_values, 3, |b, r| {
        b.push_bind(r.field).push_bind(r.person).push_bind(r.value);
    });
    insert_rows!(
        tx,
        "INSERT INTO meeting (id, troop_id, starts_at, duration_min, name, location, kind) ",
        g.meetings,
        7,
        |b, r| {
            b.push_bind(r.id)
                .push_bind(r.troop)
                .push_bind(r.starts_at)
                .push_bind(r.duration)
                .push_bind(&r.name)
                .push_bind(&r.location)
                .push_bind(r.kind);
        }
    );
    insert_rows!(tx, "INSERT INTO attendance (meeting_id, person_id, status, is_leader) ", g.attendance, 4, |b, r| {
        b.push_bind(r.meeting).push_bind(r.person).push_bind(r.status).push_bind(r.is_leader);
    });
    insert_rows!(tx, "INSERT INTO account (id, person_id, dev_persona) ", g.accounts, 3, |b, r| {
        b.push_bind(r.id).push_bind(r.person).push_bind(&r.persona);
    });
    realistic_timestamps(&mut tx).await?;
    tx.commit().await?;
    sqlx::query("ANALYZE").execute(pool).await?;

    Ok(SeedSummary {
        bodies: g.bodies.len(),
        persons: g.persons.len(),
        memberships: g.memberships.len(),
        meetings: g.meetings.len(),
        attendance: g.attendance.len(),
        personas: g.accounts.iter().map(|a| (a.persona.clone(), a.id)).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic_and_uses_test_numbers() {
        let cfg = SeedConfig {
            districts: 1,
            groups_per_district: 2,
            seed: 7,
            today: NaiveDate::from_ymd_opt(2026, 10, 16).expect("datum"),
        };
        let a = generate(&cfg);
        let b = generate(&cfg);
        assert_eq!(a.persons.len(), b.persons.len());
        assert!(a.persons.iter().zip(&b.persons).all(|(x, y)| x.first == y.first && x.birth == y.birth));
        assert!(
            a.persons.iter().all(|p| p.national_id.parse::<Personnummer>().is_ok_and(Personnummer::is_test_number))
        );
        assert!(a.persons.iter().filter_map(|p| p.email.as_deref()).all(|e| e.ends_with("@example.org")));
        assert!(a.accounts.len() >= 6, "personas skapas");
    }

    #[test]
    fn dst_offsets() {
        let summer = stockholm(
            NaiveDate::from_ymd_opt(2026, 9, 1).expect("datum"),
            NaiveTime::from_hms_opt(18, 0, 0).expect("tid"),
        );
        assert_eq!(summer.format("%H:%M").to_string(), "16:00");
        let winter = stockholm(
            NaiveDate::from_ymd_opt(2026, 11, 3).expect("datum"),
            NaiveTime::from_hms_opt(18, 0, 0).expect("tid"),
        );
        assert_eq!(winter.format("%H:%M").to_string(), "17:00");
    }
}
