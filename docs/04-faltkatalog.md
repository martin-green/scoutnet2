# Fältkatalog – alla personfält i Scoutnet och Scoutnet 2

> Version 0.1 · 2026-10-17 · Underlag: Scoutnets API-specifikation ([Scouterna/scoutnet-api](https://github.com/Scouterna/scoutnet-api)), Skojjts import, v5-profilen och admin i produktion (genomgång utan att spara persondata) samt supportdokumentationen.
> Hör till [01-analys-av-scoutnet.md](01-analys-av-scoutnet.md) och [02-plan.md](02-plan.md). Prototypens schema finns i [`0002_person_fields.sql`](../crates/sn-server/migrations/0002_person_fields.sql).

Katalogen har tre syften:

1. **Fullständighet** – alla uppgifter som Scoutnet har om en person finns med i Scoutnet 2, eller så står det uttryckligen varför de inte gör det.
2. **Behörighet och dataklass per fält** – vem som får se vad avgörs fält för fält, inte sida för sida.
3. **Kompatibilitet** – kolumnen *Scoutnet-namn* är det namn som kompatibilitetslagret (prototypvecka 5) ska använda, så att Skojjt, Webbkoppling-integrationer och kårernas egna skript fortsätter att fungera.

## Dataklasser och behörigheter

| Dataklass | Exempel | Krävd behörighet i API:t | Kommentar |
|---|---|---|---|
| **Grund** | namn, medlemsnummer, ålder, avdelning, roller | `members_read` | Allt som behövs för att leda en avdelning. |
| **Kontakt** | e-post, telefon, anhörigas kontaktuppgifter, utskicksval | `members_contact_read` | Ledare behöver kunna nå vårdnadshavare. |
| **Person** | adress, maskerat personnummer, registerutdrag, uppföljningssamtal | `personal_read` | Adress döljs vid skyddad identitet (då lagras bara förmedlingsadress). |
| **Särskilt skyddad** | fullständigt personnummer | `national_id_read` + färsk inloggning (step-up) | Varje visning loggas separat. |
| **Ekonomi** | avgiftsgrupp, terminsavgifter, KID/OCR, betalstatus | `economy_read` | Kassör, kårordförande, medlemsregistrerare. |
| **Intern** | noteringar | `members_manage` | Får aldrig innehålla hälsouppgifter (art. 9 GDPR). |

Personen själv ser alltid sina egna uppgifter, inklusive avgifter och registerutdrag, men inte interna noteringar. Filter och sortering på ekonomi- och registerutdragsfält räknas bara fram inom delträd där användaren har behörighet, så att dolda värden inte kan härledas via sökningar.

Status i tabellerna: ✅ finns i prototypen (schema, API och UI) · 🟡 schema/API finns, UI eller redigering saknas · 📅 planerad (vecka anges) · ⛔ medvetet bortvalt.

## 1. Identitet och grunduppgifter

| Fält (sv) | Scoutnet-namn | Scoutnet 2 API (`PersonDetail`) | Tabell.kolumn | Klass | Status |
|---|---|---|---|---|---|
| Medlemsnummer | `member_no` | `member_no` | `person.member_no` | Grund | ✅ |
| Förnamn | `first_name` | `first_name` | `person.first_name` | Grund | ✅ |
| Efternamn | `last_name` | `last_name` | `person.last_name` | Grund | ✅ |
| Smeknamn | `nickname` / `nick_name` | `nickname` | `person.nickname` | Grund | ✅ |
| Födelsedatum | `date_of_birth` | `birth_date`, `age` | `person.birth_date` | Grund | ✅ |
| Personnummer | `ssno` | `national_id_masked`; hela via `POST /v1/people/{id}/reveal-national-id` | `person.national_id` (+ `national_id_kind`) | Särskilt skyddad | ✅ |
| Samordningsnummer | (i `ssno`) | `national_id_kind = samordningsnummer` | `person.national_id_kind` | Särskilt skyddad | ✅ |
| Kön (4 val) | `sex` (0 okänt, 1 man, 2 kvinna, 3 annat) | `sex` (`unknown`/`male`/`female`/`other`) | `person.sex` | Grund | ✅ |
| Medlemsstatus (8 val) | `status` | `person_status` | `person.person_status` | Grund | ✅ |
| Språk | Profil → Språk | `preferred_language` (`sv`/`en`) | `person.preferred_language` | Grund | ✅ |
| Skyddad identitet | (admin) | `protected_identity` | `person.protected_identity` | Person | ✅ |
| Bild | Profil → Bild | `photo_url` | `person.photo_url` | Grund | 🟡 uppladdning 📅 |
| Presentation | Profil → Presentation | `presentation` | `person.presentation` | Grund | ✅ |
| Registrerad | `created_at` | `created_at` | `person.created_at` | Grund | ✅ |
| Uppgifter bekräftade | `confirmed_at` (person) | `confirmed_at` | `person.confirmed_at` | Grund | ✅ |
| Kan hjälpa till | `contact_leader_interest` / "Jag kan hjälpa till!" | `can_help` | `person.can_help` | Grund | ✅ |

Medlemsstatusens åtta värden motsvarar Scoutnets: Aktiv (`active`), Ny – inte klar för fakturering (`new`), Avregistrerad (`unregistered`), Automatiskt avregistrerad (`auto_unregistered`), Avliden (`deceased`), Aktiv icke-medlem (`active_non_member`), Anonymiserad (`anonymized`) och Inaktiv – låst (`locked`).

## 2. Kontaktuppgifter

Scoutnet lagrar kontaktuppgifter dels som platta fält, dels som en `contact_list` med numeriska `contact_type_id`. Scoutnet 2 använder typade kolumner, eftersom typerna är få och stabila.

| Fält (sv) | Scoutnet-namn | Scoutnet 2 API | Tabell.kolumn | Klass | Status |
|---|---|---|---|---|---|
| E-post | `email` | `contact.email` | `person.email` | Kontakt | ✅ |
| Alternativ e-post | `contact_alt_email` | `contact.alt_email` | `person.alt_email` | Kontakt | ✅ |
| Scouterna.se-e-post | Profil → Scouterna.se e-post | `contact.scouterna_email` | `person.scouterna_email` | Kontakt | ✅ |
| Mobil | `contact_mobile_phone` | `contact.mobile_phone` | `person.mobile_phone` | Kontakt | ✅ |
| Hemtelefon | `contact_home_phone` / `contact_telephone_home` | `contact.home_phone` | `person.home_phone` | Kontakt | ✅ |
| Utskick till alternativ e-post | e-postinställningar | `preferences.mailing_to_alt_email` | `person.mailing_to_alt_email` | Kontakt | ✅ |
| Detaljerade e-postinställningar | Profil → E-postinställningar | – | – | Kontakt | 📅 v5 (per utskickstyp) |

## 3. Adresser

Scoutnet har en primär adress (`address_co`, `address_1`, `address_2`, `postcode`, `town`, `country`) och en `address_list` vid registrering. Scoutnet 2 har en tabell med typ och giltighetstid.

| Fält (sv) | Scoutnet-namn | Scoutnet 2 API (`addresses[]`) | Tabell.kolumn | Klass | Status |
|---|---|---|---|---|---|
| Adresstyp | (implicit) | `address_type` (`home`, `postal`, `invoice`, `temporary`, `other`) | `person_address.address_type` | Person | ✅ |
| Primär | – | `is_primary` | `person_address.is_primary` | Person | ✅ |
| c/o | `address_co` | `care_of` | `person_address.care_of` | Person | ✅ |
| Adressrad 1–3 | `address_1`, `address_2` | `line1`, `line2`, `line3` | `person_address.line1..3` | Person | ✅ |
| Postnummer | `postcode` | `postcode` | `person_address.postcode` | Person | ✅ |
| Ort | `town` | `city` | `person_address.city` | Person | ✅ |
| Land | `country` | `country_code` (ISO 3166-1) | `person_address.country_code` | Person | ✅ |
| Giltig från/till | – | `valid_from`, `valid_to` | `person_address.valid_from/valid_to` | Person | ✅ (ny) |
| Automatisk uppdatering från SPAR | Profil → SPAR (samtycke) | `preferences.spar_sync` | `person.spar_sync` | Kontakt | 🟡 SPAR-integration 📅 efter prototypen |

Vid skyddad identitet lämnar API:t bara ut adresser av typen `postal`, alltså förmedlingsadressen. Personnumret visas inte heller maskerat.

## 4. Anhöriga, vårdnadshavare och hushåll

Scoutnet har platta fält för "mamma" och "pappa" (`contact_mothers_name`, `contact_email_mum`, `contact_mobile_mum`, `contact_telephone_mum` med motsvarande `*_dad`). I v5-profilen kallas de "Anhörig 1/2". Scoutnet 2 har en lista utan könsbundna roller och utan begränsning till två anhöriga.

| Fält (sv) | Scoutnet-namn | Scoutnet 2 API (`guardians[]`) | Tabell.kolumn | Klass | Status |
|---|---|---|---|---|---|
| Ordning (anhörig 1, 2 …) | mum = 1, dad = 2 | `position` | `guardian.position` | Grund | ✅ |
| Namn | `contact_mothers_name` / `contact_fathers_name` | `name` | `guardian.name` | Grund | ✅ |
| Relation | – | `relation` | `guardian.relation` | Grund | ✅ |
| E-post | `contact_email_mum` / `_dad` | `email` | `guardian.email` | Kontakt | ✅ |
| Mobil | `contact_mobile_mum` / `_dad` | `mobile_phone` | `guardian.mobile_phone` | Kontakt | ✅ |
| Hemtelefon | `contact_telephone_mum` / `_dad` | `home_phone` | `guardian.home_phone` | Kontakt | ✅ |
| Har eget konto | – | `has_account` | `guardian.account_person_id` | Grund | ✅ (ny: vårdnadshavare loggar in med ScoutID) |
| Hushåll / familj | "Länka familjemedlemmar", familjerabatt | `household { id, name, members[] }` | `household`, `person.household_id` | Grund | ✅ |

Hushållsmedlemmar listas bara om användaren själv får se dem, eller om det är användarens eget hushåll. Kompatibilitetslagret mappar `guardians[position=1]` till `contact_*_mum` och `guardians[position=2]` till `contact_*_dad`.

## 5. Medlemskap

| Fält (sv) | Scoutnet-namn | Scoutnet 2 API (`membership`) | Tabell.kolumn | Klass | Status |
|---|---|---|---|---|---|
| Kår | `group` | `membership.group` | `membership.group_id` | Grund | ✅ |
| Avdelning | `unit`, `unit_type` | `troop { name, troop_type, troop_type_label }` | `placement` → `body` | Grund | ✅ |
| Patrull | `patrol` | `patrol` | `placement` → `body` | Grund | ✅ |
| Roll i avdelning | `unit_role` | `is_leader`, `roles[]` | `placement.is_leader`, `role_assignment` | Grund | ✅ |
| Roll i kår | `group_role`, `roles` | `roles[]` | `role_assignment` | Grund | ✅ |
| Medlemskapsstatus | `status` (1 väntelista, 2 väntar på godkännande, 4 godkänd) | `membership.status` (`waiting`, `pending`, `active`) | `membership.status` | Grund | ✅ |
| Medlem sedan | (startdatum) | `membership.member_since` | `lower(membership.valid)` | Grund | ✅ |
| Godkänd av kåren | `confirmed_at` (medlemskap) | `membership.confirmed_at` | `membership.confirmed_at` | Grund | ✅ |
| På väntelista sedan | `waiting_since` | `membership.waiting_since` | `membership.waiting_since` | Grund | ✅ |
| Önskad avdelningstyp | `registration_preferences` | `membership.wished_troop_type` | `membership.wished_troop_type` | Grund | ✅ |
| Väntar på godkännande | `awaiting_approval` | `membership.status = pending` | `membership.status` | Grund | ✅ |
| Förstakår (primärt medlemskap) | Profil → Förstakår | (primärt medlemskap visas) | `membership.is_primary` | Grund | 🟡 byte av förstakår 📅 v5 |
| Avslutsorsak | (Tidigare medlemmar) | – | `membership.end_reason` | Grund | 🟡 |
| Placeringshistorik | – | `placements[]` (från–till) | `placement.valid` | Grund | ✅ |

## 6. Avgifter och fakturering

| Fält (sv) | Scoutnet-namn | Scoutnet 2 API | Tabell.kolumn | Klass | Status |
|---|---|---|---|---|---|
| Avgiftsgrupp | `feegroup_id` | `membership.fee_group`, `fees[].fee_group` | `membership.fee_group`, `term_fee.fee_group` | Ekonomi | ✅ |
| Föregående termin | `prev_term` | `fees[1]` | `term_fee` | Ekonomi | ✅ |
| Innevarande termin | `current_term` | `fees[0]`; i listan `fee_status` | `term_fee` | Ekonomi | ✅ |
| Förfallodag | `current_term_due_date` | `fees[].due_date` | `term_fee.due_date` | Ekonomi | ✅ |
| KID/OCR | `kid` | `fees[].kid` | `term_fee.kid` | Ekonomi | ✅ |
| Belopp | Medlemskap → fakturor | `fees[].amount_ore` | `term_fee.amount_ore` | Ekonomi | ✅ |
| Betald (datum) | Betalstatus | `fees[].paid_on`, `fees[].status` (`paid`/`unpaid`/`overdue`) | `term_fee.paid_on` | Ekonomi | ✅ |
| Fördelning förbund/distrikt/kår | Medlemskap → faktura | – | – | Ekonomi | 📅 efter prototypen |
| Fakturaleverans medlemskap | Profil → Fakturaleverans | `preferences.invoice_delivery_membership` | `person.invoice_delivery_membership` | Kontakt | ✅ |
| Fakturaleverans arrangemang | Profil → Fakturaleverans | `preferences.invoice_delivery_events` | `person.invoice_delivery_events` | Kontakt | ✅ |
| Rabatter (t.ex. SJ) | Profil → Mina rabatter | – | – | Grund | ⛔ i prototypen |

Fakturaleverans har, utöver Scoutnets tre val (primär e-post, annan e-post, vanlig post), även `efaktura` och `kivra`, som är planerade val i Scoutnet 2.

## 7. Ledaruppdrag: registerutdrag och uppföljning

| Fält (sv) | Scoutnet-namn | Scoutnet 2 API | Tabell.kolumn | Klass | Status |
|---|---|---|---|---|---|
| Tvinga registerutdrag | `force_police_check` | `police_check.forced` | `person.force_police_check` | Person | ✅ |
| Registerutdrag krävs | (härlett) | `police_check.required` | vy `police_check_status` | Person | ✅ |
| Utfärdat | Registerutdrag → datum | `police_check.issued_on` | `police_check.issued_on` | Person | ✅ |
| Uppvisat | Registerutdrag → uppvisat | `police_check.shown_on` | `police_check.shown_on` | Person | ✅ |
| Giltigt till | (3 år) | `police_check.valid_until`, i listan `police_check_valid_until` | vy `police_check_status` | Person | ✅ |
| Status | Rapport "Registerutdrag" | `police_check.status` (`missing`, `expired`, `expiring`, `valid`, `not_required`) | härlett | Person | ✅ |
| Kontrollerat av | Registerutdrag → registrerad av | `police_check.history[].verified_by` | `police_check.verified_by_person_id` | Person | ✅ |
| Uppföljningssamtal | Uppföljningssamtal (datum, roll, registrerad av) | `follow_ups[]` | `follow_up_talk` | Person | ✅ |

Scoutnet 2 registrerar bara *att* ett utdrag har visats, och när. Innehållet registreras aldrig. Utdraget krävs automatiskt för ledare och rollinnehavare. Medlemslistan har filtret "Registerutdrag saknas/går ut", som ersätter dagens rapport.

## 8. Roller, utbildning och meriter (Loggbok/CV)

| Fält (sv) | Scoutnet-namn | Scoutnet 2 API | Tabell.kolumn | Klass | Status |
|---|---|---|---|---|---|
| Aktuella roller | `roles`, Funktionärer | `roles[]` | `role_assignment` (gäller idag) | Grund | ✅ |
| Rollhistorik | Rapport "Rollhistorik" | `role_history[]` | `role_assignment` (avslutade) | Grund | ✅ |
| Utmärkelser | Profil → Utmärkelser | `merits[kind=award]` | `person_merit` | Grund | ✅ |
| Märken | Märken → Mina märken | `merits[kind=badge]` | `person_merit` | Grund | ✅ (märkeskatalog 📅) |
| Kurser | Loggbok → kurser (t.ex. Trygga möten) | `merits[kind=course]` | `person_merit` | Grund | ✅ |
| Certifieringar | Loggbok → certifieringar | `merits[kind=certification]` | `person_merit` | Grund | ✅ |
| Deltagande i arrangemang | Loggbok → arrangemang | – | – | Grund | ⛔ arrangemang ingår inte i prototypen |

## 9. Samtycken, prenumerationer och publicering

| Fält (sv) | Scoutnet-namn | Scoutnet 2 API | Tabell.kolumn | Klass | Status |
|---|---|---|---|---|---|
| Nyhetsbrev | `newsletter` | `preferences.newsletter` | `person.newsletter` | Kontakt | ✅ |
| Medlemstidningen | `magazine` | `preferences.magazine` | `person.magazine` | Kontakt | ✅ |
| Publicering av bilder | (kårens rutin) | `consents[kind=photo_publication]` | `consent` | Grund | ✅ (ny: ledare ser direkt) |
| Publicering av personlig information | Profil → Publicering | `consents[kind=publish_profile]` | `consent` | Grund | ✅ |
| Publicera roller | Profil → Publicerade roller | `consents[kind=publish_roles]` | `consent` | Grund | ✅ |
| Vem beslutade och när | – | `consents[].decided_by`, `decided_at` | `consent` | Grund | ✅ (ny: spårbarhet) |

Samtycken syns för alla som får se personen, eftersom ledare måste veta om en bild får publiceras.

## 10. Kårens egna fält och anteckningar

| Fält (sv) | Scoutnet-namn | Scoutnet 2 API | Tabell.kolumn | Klass | Status |
|---|---|---|---|---|---|
| Anpassade fält | `custom_data` (`{ field_id: svar }`) | `custom_fields[] { key, label, body_name, value }` | `custom_field`, `custom_value` | Grund | ✅ (definiera fält 📅 v5) |
| Noteringar | `note` | `note` | `person.note` | Intern | ✅ |

Anpassade fält är knutna till den enhet som definierade dem, vanligen kåren. Fritext får inte användas för hälsouppgifter. Allergier och liknande hanteras i arrangemangsanmälan med särskild gallring.

## 11. Konto, säkerhet och tjänster

| Fält (sv) | Scoutnet | Scoutnet 2 | Status |
|---|---|---|---|
| Lösenord, 2FA | Eget lösenord + valfri 2FA | ScoutID (Keycloak) med MFA, step-up för känsliga åtgärder ([plan 7.2](02-plan.md)) | 📅 v2 |
| Anslutna appar | Profil → Anslutna appar | OAuth-klienter i ScoutID, med samtycke per app | 📅 efter prototypen |
| Digitalt medlemskort | Medlemskap → digitalt kort | Signerat kort (QR + JWS) | 📅 efter prototypen |
| Medlemsintyg (PDF) | Medlemskap → medlemsintyg | Genereras från API:t | 📅 efter prototypen |
| Revisionslogg "vem har sett mina uppgifter" | – | `audit_event` (varje personvisning loggas redan) | 🟡 vy v5 |

## 12. Medlemslistans kolumner och filter

Medlemslistan (`GET /v1/bodies/{id}/members`) har följande fält per rad. Användaren väljer vilka kolumner som visas.

| Kolumn | API-fält | Sortering | Filter | Klass |
|---|---|---|---|---|
| Namn | `first_name`, `last_name` | `name`, `first_name`, `last_name` | `q` (namn eller medlemsnummer) | Grund |
| Medlemsnummer | `member_no` | `member_no` | `q` | Grund |
| Ålder / född | `age`, `birth_date` | `age`, `birth_date` | `born_from`, `born_to` | Grund |
| Kön | `sex` | – | `sex` | Grund |
| Avdelning / patrull | `troop`, `patrol` | `troop`, `patrol` | `troop_id`, `troop_type` | Grund |
| Ledare | `is_leader` | – | `leaders` | Grund |
| Medlemsstatus | `person_status` | – | – | Grund |
| Medlemskapsstatus | `status` | – | `status` (`active`, `waiting`, `pending`) | Grund |
| Väntat sedan / önskad gren | `waiting_since`, `wished_troop_type` | `waiting_since` | – | Grund |
| E-post, alt. e-post | `email`, `alt_email` | – | – | Kontakt |
| Mobil, hemtelefon | `mobile_phone`, `home_phone` | – | – | Kontakt |
| Vårdnadshavare | `guardian_count`, `guardians_missing_email` | – | `missing_guardian_email` | Grund |
| Personnummer (maskerat) | `national_id_masked` | – | – | Person |
| Närvaro innevarande termin | `attendance_pct` | `attendance` | – | Grund |
| Registerutdrag | `police_check`, `police_check_valid_until` | `police_check` | `police_check` | Person |
| Terminsavgift | `fee_status`, `fee_due_date` | `fee` | `fee_status` | Ekonomi |
| Avgiftsgrupp | `fee_group` | – | – | Ekonomi |
| Medlem sedan | `member_since` | `member_since` | – | Grund |

Svaret innehåller `redacted_fields`, som listar de fält som har dolts på någon rad. UI:t visar då 🔒 och döljer filter som rollen ändå inte kan använda.
