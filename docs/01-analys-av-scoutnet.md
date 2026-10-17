# Analys av dagens Scoutnet

> Kunskapsunderlag för projektet "Scoutnet 2". Sammanställt 2026-10-16.
> Uppdaterad samma dag efter genomgång av **produktion** (scoutnet.se). Staging (demo2.custard.no) kör en äldre version.
> Implementationsplanen finns i [02-plan.md](./02-plan.md).

## Innehåll

1. [Sammanfattning](#1-sammanfattning)
2. [Källor och metod](#2-källor-och-metod)
3. [Historik och leverantör](#3-historik-och-leverantör)
4. [Teknisk arkitektur](#4-teknisk-arkitektur)
5. [Organisationsmodell](#5-organisationsmodell)
6. [Medlemsdata](#6-medlemsdata)
7. [Roller och behörigheter](#7-roller-och-behörigheter)
8. [Funktionsinventering](#8-funktionsinventering)
   - [8.4 Produktion: Scoutnet version 5 och admin](#84-produktion-scoutnet-version-5-och-admin)
9. [Befintliga API:er](#9-befintliga-apier)
10. [Ekosystemet runt Scoutnet](#10-ekosystemet-runt-scoutnet)
11. [Skojjt – närvaroregistrering](#11-skojjt--närvaroregistrering)
12. [Juridik och regelverk](#12-juridik-och-regelverk)
13. [Observerade brister](#13-observerade-brister)
14. [Slutsatser för Scoutnet 2](#14-slutsatser-för-scoutnet-2)

---

## 1. Sammanfattning

- **Scoutnet** är Scouternas medlemsregister sedan **2008**. Det utvecklades tillsammans med Norges Speiderforbund av norska **CustardLabs AS** (Fredrikstad/Oslo). Internt heter produkten **"membernet"** (`github.com/CustardLabs/membernet`, privat).
- **Produktion** består av två gränssnitt mot samma databas:
  - **"Scoutnet version 5"** på `/f/`, som är standardvyn efter inloggning. Det är en servergenererad Symfony-app med Tailwind och PWA-stöd.
  - **"Scoutnet admin"**, version **4.26.12.1**, som är det gamla gränssnittet för administration.
  
  Staging kör den äldre **4.24.2**.
- Stacken är **PHP + Symfony 1.4 + Doctrine 1.2 + PostgreSQL** (admin) plus en parallell **Symfony 7**-app (v5). Den körs idag på PHP 8.4 och PostgreSQL 18. Bakgrundsjobb körs med **Gearman** och massutskick av e-post med **Mailman**. Admin-gränssnittet bygger på **Prototype.js/Scriptaculous och jQuery 1.12.4**. v5 använder **Webpack Encore, Tailwind och jQuery 3.6.3**.
- **Inloggning** sker fortfarande med Scoutnets egen användarnamn/lösenord-sida. 2FA finns som tillval i v5-profilen. ScoutID ligger utanpå som separat SSO för *andra* tjänster.
- Systemet är **mycket brett**. Det omfattar medlemsregister, organisationsträd i 10 nivåer, cirka 50 behörighetsnycklar, väntelista, självregistrering, arrangemang/kurser med formulärbyggare och incheckning, termins­fakturering med fyra faktureringsmetoder, OCR, återbetalning till kår och distrikt, statsbidrag, registerutdrag, märken/utmärkelser/CV, e-postlistor, SMS, filarkiv, enkäter och rapportcenter.
- **Skala:** cirka **83 000 medlemmar** (2024), **1 000+ kårer** i cirka 280 kommuner, plus historik sedan 2008.
- **API:erna är fragmenterade.** Det finns per endpoint och per organisationsenhet unika API-nycklar (Basic Auth eller `?id=&key=` i URL), ett separat JWT-baserat "Auth API" och endast delvis dokumenterade svarsformat. Volontärer har fått dokumentera och reverse-engineera API:et ([Scouterna/scoutnet-api](https://github.com/Scouterna/scoutnet-api)).
- **Ett stort volontärekosystem** fyller luckor: ScoutID (SSO, sedan oktober 2026 på Keycloak i Scouternas AKS), Google- och Office365-synk, Skojjt (närvaro), mocknet (testserver), j26-cache med flera.
- **Huvudsakliga brister:**
  - föråldrad UI med två parallella gränssnitt
  - tunga sidor utan riktig filtrering och sortering
  - flaskhalsar i behörighetsmodellen (vanliga ledare kan inte lägga till eller ta bort scouter)
  - inga vårdnadshavarkonton (anhöriga är fortfarande platta fält i produktion)
  - 2FA är frivilligt och ScoutID används inte för Scoutnet-inloggningen
  - svag API-säkerhet (nycklar i URL, tokens som aldrig går ut)
  - brister i webbsäkerheten (se 13.3, detaljer i konfidentiell bilaga)
  - långsamma admin-sidor (2–3 s serversvarstid)
  - närvaro och kommunala bidrag som till stor del sköts utanför systemet

## 2. Källor och metod

| Källa | Vad som gjordes |
|---|---|
| **Produktion** `https://scoutnet.se/` (inloggad som kårfunktionär, inte superadmin) | Genomgång av v5 (`/f/`) och Scoutnet admin: navigering, formulärfält, filter, rollistor, endpoints under Webbkoppling, AJAX-anrop, serversvarstider (TTFB), HTTP-headers, cookies och tredjepartsskript. Endast läsande sidvisningar. Inga formulär skickades, inga nycklar genererades och ingenting laddades ned. **Inga personuppgifter har förts in i dokumentationen.** |
| Staging `https://demo2.custard.no/` (inloggad som superadmin, **äldre version 4.24.2**) | Navigeringskartläggning av ~40 sidor, formulärfält, tabellkolumner, rapportlista, admin-funktioner, behörighetskatalog, mätning av svarstider, granskning av HTTP-headers och laddade skript. Endast läsande GET-anrop. Inga data ändrades. Gav insyn i superadmin-funktioner (Ekonomi, Administration) som inte syns för en kårroll. |
| Custards egen systemdokumentation (`/admin/doc` på staging, 14 PDF:er) | Architecture, Installation, Configuration, API, API Setup, Auth API, Roles, Police Check, Invoicing System, Mailinglists, Projects, Reports, Appendix A. **Sammanfattas här, kopieras inte** (upphovsrätt). |
| [Scouterna/scoutnet-api](https://github.com/Scouterna/scoutnet-api) | Hela OpenAPI 3.1-specen (9 endpoints, 25 komponenter), klient och issues. |
| [Scouterna/skojjt-v2](https://github.com/Scouterna/skojjt-v2) och [skojjt](https://github.com/Scouterna/skojjt) | Datamodell, exporter, Scoutnet-integration, bidragsregler, issues och CHANGELOG. |
| [Scouterna/scoutid-keycloak](https://github.com/Scouterna/scoutid-keycloak), `-provider`, `-theme`, `-admin` och `-helm` | Nya ScoutID: arkitektur, autentiseringsflöde mot Scoutnet, claims och scopes, sessioner, klientregistrering, kända begränsningar och öppna ärenden. |
| [support.scouterna.se – Scoutnet](https://support.scouterna.se/support/solutions/75000002977) | Samtliga 94 artiklar i 8 mappar. |
| Webben: scouterna.se, Wayback Machine (custard.no), GitHub-org Scouterna (68 repon), IMY, Polisen, MUCF | Historik, ekosystem, regelverk. |

De fullständiga råunderlagen (engelska, med källhänvisningar per påstående) ligger i sessionsarkivet och kan läggas till i `docs/research/` vid behov.

## 3. Historik och leverantör

- **2008:** Scoutnet skapas i samarbete med Norges Speiderforbund. Ursprungligen hanterade det bara medlemsavgifter och statsbidrag och växte sedan.
- **2014:** CustardLabs AS bildas formellt. Grundaren var en av de ursprungliga utvecklarna. Kunder enligt Custard (2014): Scouterna och Norges Speiderforbund. Norge använder idag "Min speiding" på en separat drift (annan OS- och patchnivå).
- **2019:** Custards jobbannons nämner "First generation Symfony & Doctrine" och "New generation Symfony & Doctrine" och planer på "modernised UI and full-blooded API".
- **cirka 2023:** "Scoutnet version 5" lanseras som PWA för medlemmar och ledare (närvaro, märkestagning). Den körs parallellt med "Scoutnet admin" mot samma databas och migrerar funktion för funktion.
- Inga publika upphandlingsdokument hittades. **Avtalsvillkor om dataägande och dataexport är okända och måste utredas tidigt** (se planens risker).

## 4. Teknisk arkitektur

Enligt Custards egen dokumentation och observationer på staging och i produktion:

| Område | Fakta |
|---|---|
| Språk/ramverk | PHP. Symfony 1.4 + Doctrine 1.2 (ORM och DBAL), med en "parallel Symfony 7 system". Minsta krav är PHP 8.4. |
| Databas | PostgreSQL (18 på de flesta instanser), tillägget `unaccent`. Vissa frågor är PostgreSQL-specifika. |
| Webbserver | Apache med prefork-MPM (icke-trådad). `Server: Apache/2.4.58 (Ubuntu)` i både produktion och staging. |
| Bakgrundsjobb | Gearman. Alla rapporter körs som bakgrundsjobb som genererar filer. |
| E-post | Mailman på servernivå. Varje e-postlista får en egen adress. |
| Frontend (admin) | PHP-mallar (serverrenderat), Prototype.js/Scriptaculous (legacy), jQuery 1.12.4, jQuery UI, tablekit, tablesorter, chosen, wz_tooltip. Freshworks supportwidget laddas på alla sidor. |
| Frontend (v5, `/f/`) | Servergenererade Twig-sidor (inget SPA och inget JSON-API under sidan), Webpack Encore-bundles (`/build/*.js`), Tailwind CSS, jQuery 3.6.3 och flera bibliotek från publika CDN:er, Freshworks. PWA-manifest och "Lägg till Scoutnet på hemskärm". Gamla rutter mappas till v5 via `/f/from/<…>`. |
| Inloggning | Egen formulärinloggning på `/f/login` (användarnamn = e-post, personnummer eller medlemsnummer; lösenord; "kom ihåg mig"; CSRF-token) och "Glömt lösenord". Frivillig 2FA via kod (profilinställning). Ingen inloggning via ScoutID, BankID eller passkeys. |
| Konfiguration | `app.yml`-kaskad. API-endpoints aktiveras i konfigurationen. Produkter (avgiftstyper) kan **bara ändras via migrering**, inte i UI. |
| Integrationer | Hogia (transaktioner via SFTP), Itella (tryckfil), Visma (CSV), OCR-filer (bankgiro), Excel-export, Kivra/e-faktura (enligt användardokumentationen), Mailman, Gearman, iCal-flöden, SJ-rabatter (medlemsförmåner). |
| Formatering | BBCode för användarinmatad text. |
| Spår av teknisk skuld | `max_input_vars = 3000` krävs "because of the poorly designed Statsbidrag forms". Prototype.js och två jQuery-versioner används parallellt i två gränssnitt. Rapportparametrar skickas som ramverksspecifika, otypade strukturer i URL:er. |

## 5. Organisationsmodell

Alla enheter är generiska **"bodies"** med typ:

```
organisation ─┬─ region
              ├─ nätverk (network)
              ├─ distrikt ── kår
              ├─ samverkansorganisation (corps) ── samv.org.-distrikt (corpsdistrict) ── kår
              └─ kår (direktansluten)
kår ─┬─ sektion (geografisk plats, adresser)
     └─ avdelning (troop, kopplad till sektion) ── patrull
arrangemang (project) – egen rollkontext, kan ha underarrangemang
```

- **Kårtyp:** Scoutkår, Sjöscoutkår, Hub-kår.
- **Avdelningstyp i produktion:** Familjeavdelning, Bäveravdelning, Spåraravdelning, Upptäckaravdelning, Äventyraravdelning, Utmanarlag, Roverlag, Annan (med min- och maxålder per avdelning). Staging använder äldre namn (Bäverscouter, Spårarscouter …).
- **Avdelningens mötesinformation** (v5): vanliga veckodagar, starttid, sluttid och lokal. Det är samma standardvärden som Skojjt behöver.
- **Kårens attribut:**
  - namn, org.nr, kårnummer, kårordförande, grundad, status, "faktureras ej", "har medlemmar" och statsbidrag (ja/nej)
  - kommun, län, bankkonto och hemsida
  - e-post för faktura och medlemsfrågor, Facebook, Instagram och Twitter
  - märkningen "Hållbar Scoutkår" med giltighetsdatum
  - adresser med typ och **giltighetsdatum** (postadress, scoutlokal, faktura, besök)
  - **sektioner** med egen adress, län och kommun (alla 290 kommuner)
  - kontaktuppgifter och notifieringsinställningar
- **Regioner:** Norr/Mitt, Stockholm, Södra, Västra, Östra.
- **Organisationsträdet i produktion:** cirka 25 distrikt, flera samverkansorganisationer (corps) och nätverk under "Scouterna".
- **Samverkansorganisationer:** Equmeniascout, Nykterhetsrörelsens Scoutförbund, Salt (EFS) med flera. Arrangemang kan filtreras på organisation: Scouterna, Scouternas folkhögskola, Övriga scoutföreningar, Gäster.
- **Åldersgrupper** administreras centralt. Arrangemangsfiltret i produktion har Familjescout, Spårare, Upptäckare, Äventyrare, Utmanare, Rover och Vuxna/ledare.

## 6. Medlemsdata

> Alla personfält med mappning till Scoutnet 2 (API-namn, tabell, dataklass, behörighet och kompatibilitetsnamn) finns i [04-faltkatalog.md](./04-faltkatalog.md).

**Person/profil** (formulärfält i produktion, v5 `/f/profile/edit`, och på staging):

- personnummer (födelsedatum och fyra sista siffrorna i separata fält), kön (4 val: Okänt/Man/Kvinna/Annat), förnamn, efternamn, smeknamn och födelsedatum
- **språk** (svenska/engelska)
- primär e-post, alternativ e-post, **"Scouterna.se e-post"**, mobil och hemtelefon (landskod per nummer)
- **Anhörig 1 och 2** (namn, e-post, mobil, hemtelefon). Detta är **fortfarande platta kontaktfält i produktion**, inte konton eller relationer. v5 visar dem som "Närmast anhöriga".
- "Jag kan hjälpa till!", noteringar, bild och presentation
- flera adresser med typ, prioritet och giltighet
- länkade familjemedlemmar (hushåll, familjerabatt)
- nyhetsbrev, tidningen Scout och faktureringskanal per kontext (medlemskap eller arrangemang: primär e-post, annan e-post eller vanlig post), plus e-postinställningar per adress
- "Automatisk uppdatering av personuppgifter från SPAR". I produktion visas detta som ett samtyckesval i profilen. På staging står det "för närvarande ej aktiverat".
- lösenordsbyte och **2FA-aktivering** (v5)
- **digitalt medlemskort** per kår, medlemsintyg som PDF, fakturahistorik per termin med fördelning mellan förbund, distrikt och kår, och medlemsförmåner ("Mina rabatter", t.ex. SJ)

**Medlemsstatus (8 st):** Aktiv, Ny – inte klar för fakturering, Avregistrerad, Automatiskt avregistrerad, Avliden, Aktiv icke-medlem, Anonymiserad, Inaktiv – låst.
Dessutom finns membership-status per kår: godkänd, väntelista och väntar på godkännande. **Avregistreringsorsaker:** utan angiven orsak, ålder, obetald avgift, korrigering.

**Kopplade poster på profilen:**

- Scoutnet-historik, betalstatus, fakturor och medlemsintyg
- godkända medlemskap, publicerade roller (styr vad som exponeras via API) och publicering av personlig information
- registerutdrag (utfärdat datum, visad den, visat för) och "tvinga registerutdrag"
- uppföljningssamtal och rollhistorik
- CV/loggbok (kurser, certifieringar) samt utmärkelser och märken
- arrangemang (kommande och tidigare)
- **anslutna appar** (utfärdade API-tokens som kan återkallas)
- "Ladda ner lagrad medlemsdata" (GDPR-export)
- "Konvertera medlem till enhet"

**Anpassade fält:** `custom_data` vid registrering och anpassade fält i självregistreringsformulär (kryssruta, en rad, flera rader).

**API-fältnamn** (memberlist): `member_no, first_name, last_name, ssno, date_of_birth, status, created_at, confirmed_at, group, unit, unit_type, unit_role, group_role, patrol, sex, address_co, address_1, address_2, postcode, town, country, email, contact_*` (alt_email, mobile_phone, home_phone, mothers_name, email_mum, mobile_mum, …), `prev_term, current_term, current_term_due_date, kid, roles, waiting_since, awaiting_approval, note, nickname`. Varje fält har formen `{value, raw_value}`.

## 7. Roller och behörigheter

- **Roller definieras per kontext** (organisation, region, nätverk, distrikt, samv.org.-distrikt, samverkansorganisation, kår, avdelning, patrull och arrangemang). En roll har:
  - rollnyckel (språkoberoende, används av API)
  - svenska och engelska namn och flaggan "gömd"
  - CV-synlighet och CV-berättigande
  - uppföljningssamtal (inga/valfritt/obligatoriskt)
  - **maximal uppdragslängd**
  - flaggan "tillåt på enheter utan medlemmar"
  - publicering via API (titel, plats)
  - anpassade fält
- **Arv:** roller propagerar **bara** om samma roll finns på lägre nivå. Det finns en funktion för att "kopiera roll till annan nivå", vilket är svårt att förstå och lätt att göra fel.
- **Behörighetsnycklar** (urval, cirka 50 st):
  - `EditInfo`, `SeeMembers`, `SeeExtraUserInfo`, `SeePrivilegedUserInfo`, `ManageMembers`, `UnregisterMembers`, `EditMembersStatuses`, `ManageRoles`
  - `SeeEconomy`, `ManageEconomy`, `AssignHouseholdDiscounts`, `ManageHouseholds`
  - `SeeAllChildren`, `ManageChildren`, `ManageEntities`, `SeeEntities`
  - `ViewReports`, `CreateProjects`, `ManageProjectAttendance`, `SeeProjectAttendeeLists`, `ViewFinalisedProjectReports`
  - `ManageProtectedMailingLists`, `SeeProtectedMailingLists`, `ManagePrivateMailingLists`
  - `ManageAwards`, `ViewAwards`, `ManageAchievements`, `ViewAchievements`, `ManageOwnAchievements`, `ViewBodyMembersCVs`, `ManageBodyMembersCVs`
  - `PoliceCheckRequired`, `ManageMembersPoliceChecks`, `SeeMembersPoliceChecks`
  - `ConfidentialityAgreementRequired`, `ManageConfidentialityAgreements`, `SeeConfidentialityAgreements`
  - `ShouldHaveMFA`, `RequiresMFA`
  - `SeeAllMembers`, `ManageApiKeys`
  - `ManageStatsbidragApplications`, `ViewStatsbidragApplications`, `AdministrateLockedStatsbidragApplications`
  - `ManageFiles`, `ViewFiles`
  - `AccessSSOLeaderTrainerPages`
- **Rollkatalogen** innehåller över 150 roller. Exempel på kårnivå:
  - styrelse: kårordförande, vice ordförande, 2:e vice, sekreterare, kassör, vice kassör, ledamot, suppleant
  - funktionärer: medlemsregistrerare, bidragsombud, köansvarig, nyanmälningar, IT-ansvarig, kårutbildare, revisor, valberedning, firmatecknare, arkivansvarig, materielansvarig, stugbokning, webbansvarig, grenansvariga per åldersgren
  - avdelning: avdelningsledare, vice, administratör, ledare, assisterande ledare
  - patrull: patrulledare
- Katalogen innehåller också **skräpposter** som "8db301", "ad5322", "New role" och "Ny roll" ×16, plus blandat svenska och engelska nycklar.
- **Registerutdrag:**
  - Roller med `PoliceCheckRequired` kräver uppvisat utdrag. Ett utdrag som visats på kårnivå gäller för kåren och lägre nivåer.
  - Utdraget blir ogiltigt när datumet passerat, **12 månader** efter att en kårroll tagits bort, **15 månader** för övriga roller och när arrangemanget slutar för arrangemangsroller.
  - Endast kårordförande eller registerutdragsadmin får registrera. Innehållet får aldrig lagras.
- **Praktiska flaskhalsar** (användardokumentationen): vanliga *Ledare* kan inte lägga till eller ta bort scouter, utan det måste medlemsregistreraren göra. Dubbletter och personer som redan finns i en annan kår måste mejlas till förbundskansliet.

## 8. Funktionsinventering

### 8.1 Navigering (Scoutnet admin)

Toppmeny:

- **Mina sidor**
- **Översikt** (organisationsträd och enheter)
- **Organisation**
- **Arrangemang**
- **Rapportcenter**
- **Ekonomi**
- **Administration**

Därtill Bokmärken (stjärna på varje sida) och global sökning (autocomplete från 3 tecken, standard- och adressökning).

**Kårsidan** har flikarna Detaljer, Funktionärer, E-postlistor, Medlemmar, Ekonomi, Arrangemang, Rapporter, Webbkoppling, Arkiv och Statsbidrag.
Medlemsfliken har vyerna *Alla medlemmar*, *Medlemmar som inte godkänts* (väntelista och väntar på avregistrering), *Hantera medlemmar* (bulk), *Tidigare medlemmar* och *Bjud in medlemmar*.

### 8.2 Moduler

| Modul | Innehåll (observerat och dokumenterat) |
|---|---|
| Medlemsregister | Profil, adresser, kontakter, familj/hushåll, status, historik, GDPR-export, dubbletthantering (personnummer, förnamn, e-post), import och avregistrering från Excel-fil, "Rensa medlemsdata", särskilda medlemsstatusar. |
| Bulkhantering | Flytta till annan kår, sätt medlemsavgift, upphäv medlemskap, tvinga och ångra registerutdrag, flytta till avdelning och patrull. Valbara kolumner (medlemsnr, namn, födelsedatum, avdelning, patrull, bekräftat per termin, förstakår, avgift). |
| Väntelista och självregistrering | Publikt formulär per kår (iframe, egen CSS, valbara kontaktfält, anpassade fält, val av land, önskad avdelning, notifiering till roller och e-postadresser, kårspecifik GDPR-text, **Google Analytics-ID**). Destination: "väntar på godkännande" eller väntelista. |
| Roller | Tilldela roll per enhet, rollhistorik, maximal uppdragslängd, uppföljningssamtal. |
| Arrangemang | Typer (projekt, hajk, läger, kurs, möte med flera) och mallar. Individuell anmälan via formulärbyggare med villkorsstyrda frågor, kåranmälan, lägg till utan formulär, filimport. Avgifter, tillval och utrustning, kårfrågor, kupongkoder, arrangemangsteam med roller. **Incheckning/utcheckning** (närvarande/frånvarande). Slutrapportering (uppdaterar CV), kopiera, fäst, publicera. Rapporter per arrangemang, egna API-nycklar. GDPR-gallring av svar 90 dagar efter kurs. |
| Närvaro (v5) | "Möte" som arrangemangstyp med enkel anmälan (RSVP), ledaren noterar närvaro, DAK-export, en notering per dag för läger. |
| Kurser (Folkhögskolan) | Ett 21 artiklar stort flöde ovanpå arrangemang: målgrupp, kostnadsställe, kursteam, katalogpublicering, intyg, uppföljningssamtal och ackreditering. |
| E-postlistor | Statiska och dynamiska (filter på roller, nivåer, märken, ålder m.m.), fördefinierade eller egna, egen e-postadress per lista, svarsadress, export (xls/csv), skräppostfilter, konton som får skicka. |
| SMS | Konfigurerbara SMS-leverantörer (admin). Användardokumentation saknas. |
| Ekonomi | Fakturering per **termin** (avgiftsberäkning, godkännande, utskrift, export). **4 faktureringsmetoder** (`BodyFee::INVOICING_METHOD_1–4`). Produkter och produktlänkning (t.ex. tidning, båtförsäkring per medlem, betalas av medlem eller kår). Lokala avgiftsgrupper (cirka 9–10 per kår), familjerabatt och delterminsavgift. OCR-import och OCR-matchning, manuell betalning, import av betalningar från xls, transaktionslista, **återbetalning** (backpayments) till kår och distrikt, kårfakturor (konsolidering), kupongkoder, väntande avgifter och fellogg. Leveranskanal: e-faktura > Kivra > e-post > post. Avgiftsändringar är **låsta till januari och juli–augusti**. |
| Statsbidrag | Plugin: ansökningsomgångar per år, texter per steg, ansökan på kår-, avdelnings- eller patrullnivå, bekräfta medlemskap, e-signering eller PDF, utbetalning. |
| Registerutdrag | Se avsnitt 7. Automatiska påminnelser. |
| Utbildning och CV | Kurstyper, certifieringstyper, CV per medlem, rapporter över kurser och certifieringar. |
| Märken och utmärkelser | Märkestyp, kategori, mål. Märkestagning i v5 (hela märket eller delmoment). |
| Arkiv | Filer per enhet ("första versionen", ingen taggning). |
| Enkäter | Anpassade undersökningar med period, gäller för, administratörsgodkännande och tillåt flera. |
| Resurser | Resurstyper och resursattribut (t.ex. stugor och båtar), tillgänglighetsgraderingar. |
| Systemadmin | Systemkonfiguration, mallar och översättning (e-post, intyg, systemmeddelanden), automatiska e-postmeddelanden (triggers), uppgiftskö och schema, cache, superadministratörer, inaktiva kårer, kårer med färre än 3 betalande, historiska enheter, e-postning av medlemsstatistik till kår- och distriktsordförande, utvecklingsfiler och databasbackuper. |

### 8.3 Rapportcenter (komplett lista på staging)

- **Medlemskap:** medlemslista kår (med betalstatus), registerutdrag-lista, CV-lista, kurser/certifieringar/utmärkelser, medlemslista organisation/distrikt, samverkansorganisation, rollhistorik, väntelista, automatiska e-postmeddelanden.
- **Organisation:** aktiva och inaktiva kårer, adresslista.
- **Ekonomi:** betalande medlemmar per termin, medlemmar utan avgift, kår- och distriktsbetalningar per termin, antal betalande (distrikt och samv.org.), betalningar per arrangemang, alla arrangemangsbetalningar per datum.
- **Statistik:** avslutade medlemskap per år, kåromsättning per år, varaktighet för medlemskap.
- **Arrangemang:** anmälda kårer, aktivitetsval, individuella svar, kåranmälan namnlista, slutförd deltagarlista, arrangemangsrollhistorik, närvaro.
- **Medlemsföreningar** (statsbidrag).

Alla rapporter körs som bakgrundsjobb och resultatet laddas ned som fil.

### 8.4 Produktion: Scoutnet version 5 och admin

Genomgången gjordes med en kårfunktionärs behörighet och var bara läsande. Inga personuppgifter har förts in här.

**v5 (`/f/`), standardvyn efter inloggning:**

| Del | Innehåll |
|---|---|
| Meny | Startsida, mina kårer (Översikt, avdelningar, Arkiv), Scouterna (organisationsträd), Arrangemang (Översikt, Skapa arrangemang), Märken (Sök efter märke, Mina märken), Profil (Översikt, Medlemskap, Loggbok, Mina rabatter), länk till "Scoutnet admin", Logga ut |
| Startsida | Välkomsttext, "Vad händer?" (karusell med kommande arrangemang), "Mina medlemskap" med **digitalt medlemskort**, uppmaning att installera som PWA |
| Organisation | Sidor per nivå: organisation, region, distrikt, corps, nätverk, kår, avdelning och patrull. Visar generell info, avdelningar och patruller, ledare med telefon och e-post, medlemsnamn och **iCal-flöde per enhet**. |
| Redigera kår eller avdelning | Grunduppgifter, kontaktkanaler, adresser och bild. För avdelningar även **mötesinformation** (veckodagar, tider, lokal). |
| Arkiv | Filer per kår (lista och uppladdning) |
| Arrangemang | Lista med filter: typ (Arrangemang, Hajk, Kurs, Läger, **Möte**, Nationell Jamboree, Tävling, Ytterligare formulär, Övernattning), lämplig för (åldersgrupp), år från och till, arrangörsnivå, anmälningsstatus och organisation. iCal-export av egna arrangemang. |
| Skapa arrangemang | Arrangör, mall, typ, ledare, namn, status, ingress, start och slut, **återkommande** (veckodagar, period), plats (adressökning, karta, koordinater), **"Närvarohantering med enkel ja/nej-anmälan"**, iCal-synlighet (alla eller endast medlemskalendrar), **"Inkludera i DAK-export"** (en notering per dag) |
| Märken | Katalog med filter (typ, åldersgrupp, programområde, kategori, aktiv/utgången), kravtext per märke, **märkestagning** (välj scouter och godkänn), Mina märken |
| Profil | Översikt, redigera (se avsnitt 6), medlemskap (digitalt kort, intyg, fakturor per termin), **Loggbok** (deltagande, kurser och certifieringar som Trygga möten, roller, utmärkelser), Mina rabatter |

**Scoutnet admin 4.26.12.1 (`/`), kårperspektiv:**

- **Menyn** för en kårroll innehåller Mina sidor, Översikt, Organisation, Arrangemang, Rapportcenter och "Scoutnet" (tillbaka till v5). Ekonomi och Administration syns bara för förbundsroller.
- **Kårsidans flikar** är desamma som på staging. Medlemsfliken har vyerna Alla, Ej godkända, Hantera, Tidigare och Bjud in. Ekonomifliken har Utbetalningar, Fakturor och Avgifter. Det finns också en statistikvy med ålder × kön, där könsstatistiken bara är binär.
- **Bulkåtgärder** för en kårroll: sätt medlemsavgift, upphäv medlemskap, tvinga eller ångra registerutdrag, flytta till avdelning och patrull. Flytt till annan kår är förbehållen förbundet.
- **Väntelistan** har kolumnerna medlemsnummer, namn, födelseår, registrerad, väntat sedan, önskade avdelningar och "kan hjälpa till". Separata listor visar medlemmar som väntar på godkännande och medlemmar som väntar på avregistrering.
- **Rolllistan** på kårnivå har 78 valbara roller.
- **Rapportcenter** för en kårroll har medlemslista kår, registerutdrag, CV, kurser/certifieringar/utmärkelser, rollhistorik, organisation och arrangemang.
- **Webbkoppling** har samma endpoints som staging plus **"Get a list of resources"** och en egen endpoint per e-postlista. Kårer använder e-postlistor som API-urval, t.ex. för Google-synk. Både "HTTP-autentisering" och "Alternativ metod" (nyckel i URL) erbjuds.
- **Självregistreringen** har fortfarande egen CSS och fält för Google Analytics-ID. Kontaktfält och notifieringsroller är konfigurerbara.

**Skillnader mot staging (4.24.2):** v5 är huvudgränssnittet och har märkestagning, möten med ja/nej-anmälan, DAK-flagga, återkommande arrangemang, digitalt medlemskort, loggbok, rabatter, 2FA och mötesinformation per avdelning. Sessionscookien har skyddsflaggor på v5-inloggningen. Avdelningstyperna har nya namn, och endpointen för resurser har tillkommit. Superadmin-funktionerna (Ekonomi, Administration) kunde bara studeras på staging.

## 9. Befintliga API:er

### 9.1 "Body API" (maskin till maskin)

- **En API-nyckel per endpoint och per enhet.** Den genereras eller regenereras på enhetens sida (fliken Webbkoppling). Autentisering sker via Basic Auth (`id:key`) **eller som query-parametrar `?id=…&key=…`**.
- Endpoints för kår:
  - visa kårinfo (getter-baserad dynamisk serialisering som styrs av konfiguration)
  - kårens arrangemang
  - registrera på väntelista
  - uppdatera medlemskap (väntestatus, avdelning, patrull)
  - detaljerad medlemslista (csv/xls/json)
  - medlemslistor baserade på e-postlistor
  - lista över resurser (endast i produktion)
- Endpoints för organisation: `organisation/group/all` (med `updated_since` och `district_ids[]`), `organisation/published_roles/all`, `project/get/published`, `admin/triggered_emails` och `body_key_list` (endast interna system).
- Endpoints för arrangemang: `project/get/participants`, `project/get/groups`, `project/get/questions` och `project/checkin`.
- **Egenheter:**
  - tomma objekt returneras som `[]`
  - 401 utan body
  - två olika statusvokabulärer (`1/2/4` mot `confirmed/waiting/cancelled`)
  - två olika adressformat
  - explicita `null` i stället för utelämnade fält
  - validering som styrs av konfiguration (kan inte uttryckas i schemat)
  - `viewGroupProjects` returnerar i praktiken `[]` för de flesta kårer
  - typos i publika operationId:n
  - inget officiellt sandbox-API

### 9.2 "Auth API" (användarinloggning för appar)

- `POST /api/authenticate` med användarnamn och lösenord ger en JWT. **Med `app_id` går token aldrig ut**, utan blir det 10 minuter.
- Endpoints: `get/profile`, `refresh_token`, `get/user_access_list`, `get/user_roles`, `get/profile_image`, `check_permission`, `get/memberships`, `get/memberlist/:type/:id`, `get/projects/available` och `get/projects/registered`.
- Återkallelse sker per token av medlemmen, eller globalt genom att byta serverns privata nyckel eller trunkera tabellen.

## 10. Ekosystemet runt Scoutnet

| Projekt | Syfte | Signal |
|---|---|---|
| ScoutID (`scoutid-keycloak`, `-provider`, `-theme`, `-admin`, `-helm`) | SSO för Scouternas tjänster. **Nya ScoutID** körs på Keycloak 26.5 (realm `scoutid`, `id.scouterna.se`) i produktion sedan cirka 2026-10-01, på Scouternas AKS (Sweden Central, ArgoCD, CloudNativePG). Gamla ScoutID (SimpleSAMLphp på `scoutid.se`) lever kvar för SAML-tjänster och vissa OIDC-klienter. Detaljer finns i [planen, avsnitt 7.2](./02-plan.md#72-inloggning-via-scoutid-keycloak). | Keycloak lagrar inga lösenord. Varje inloggning kontrolleras mot Scoutnets `/api/authenticate`, och profil och roller hämtas via `/api/get/profile` och `/api/get/user_roles`. Rollclaimet trunkeras vid 2048 tecken. MFA, passkeys och BankID saknas. **Scoutnet är fortfarande källa för inloggningsuppgifter.** |
| Google-Scoutnet-synk, Office365-Scoutnet-synk | Konton, grupper och listor till Google Workspace och M365. | Saknas i kärnan. Långvariga issues (dubbletter, funktionsadresser, regeltyper). |
| skojjt, skojjt-v2, skojjt-3 | Närvaro, DAK, Sensus, märken. | Kärnans närvarofunktion har varit otillräcklig. |
| Klako/mocknet | Mock av Scoutnet-API:et. | Inget officiellt sandbox-API finns. |
| j26-scoutnet-cache, j26-scoutid-sync | Cache och synk för Jamboree 2026. | API:et klarar inte topplast utan cache. |
| discord-scoutid-linked-role | Discord-roller från arrangemangsdata. | OIDC fungerar. Arrangemangsdata används brett. |
| ScoutOrg, Prognos, WP-widgets (arkiverade) | Statistik, prognos per avdelning, WordPress-visning. | Behov av prognoser och publik data saknas i kärnan. |

## 11. Skojjt – närvaroregistrering

- **v2:** C# 14 / .NET 10, Blazor Server + MudBlazor, EF Core, PostgreSQL, ScoutID (SAML i produktion, OIDC framåt), Azure App Service.
- **Datamodell:**
  - `Semester` (VT/HT, id = år·10 + HT)
  - `ScoutGroup` (Scoutnet-id, föreningsID, kommun-ID, 4 Scoutnet-nycklar, bidragsinställningar)
  - `Person` (medlemsnr, personnummer med checksumma, föräldrakontakter, `MemberYears`, mjuk borttagning)
  - `Troop` per termin (standardtid 18:30 och 90 min, typ vanlig eller läger, låsning efter rapport)
  - `TroopPerson` (ledarflagga och patrull per avdelningsmedlemskap)
  - `Meeting` (datum, tid, längd, plats, hajk-flagga, **max ett per dag**)
  - `MeetingAttendance`
  - märken med delmoment (scout eller ledare) och ångra-historik
- **Exporter:**
  - **DAK XML** (Digitalt Aktivitetskort)
  - DAK-import med konfliktlösning
  - DAK för alla avdelningar som ZIP
  - JSON
  - **Excel enligt Göteborgs och Stockholms mallar** (Stockholm har hårda gränser på 36 möten och 48 personer)
  - **Göteborg aktivitetsbidrag-CSV**
  - **Lägerbidrag** (Göteborg 2–15 dagar och 7–25 år plus max ⅓ äldre; Stockholm 2–7 dagar och 7–20 år)
  - **Sensus**: direktsynk till e-tjänstens närvarolista via reverse-engineerat API
- **Bidragsregelmotor** som versionerad data per kommun och år. Exempel Göteborg 2026: 7–25 år, ledare minst 13 år, minst 3 deltagare och 1 ledare, minst 60 min, 9,89 kr per flicka och 8,02 kr per pojke och sammankomst.
- **Lärdomar:**
  - begränsningen till ett möte per dag är den mest efterfrågade ändringen
  - personer får aldrig hårdraderas (bryter historik)
  - märkesmigrering kräver referensintegritet
  - många ScoutID-claims gav HTTP 431 (för stora headers) i Skojjts SAML-flöde. Nya ScoutID har motsvarande gräns: rollattributet trunkeras vid 2048 tecken.
  - medlemmar utan svenskt personnummer faller tyst bort ur bidragsunderlag
  - import från Scoutnet är manuell, eftersom det saknas webhooks

## 12. Juridik och regelverk

| Område | Krav |
|---|---|
| GDPR | Scouterna och kåren är personuppgiftsansvariga. Krav på dataminimering, ändamålsbegränsning och registerutdrag enligt art. 15. Rättslig grund för arkivering är **berättigat intresse** (inte arkivlagen), och det räcker inte för känsliga uppgifter. Riksarkivet avråder tills vidare från gallring av föreningsarkiv. |
| Personnummer | Får behandlas när det är klart motiverat (säker identifiering, bidragsunderlag). Ska maskas och åtkomstbegränsas. Samordningsnummer och utländska medlemmar utan personnummer måste stödjas. |
| Skyddad identitet | Folkbokföringsadressen får inte registreras. Använd Skatteverkets förmedlingsadress. Strikt need-to-know. IMY har bötfällt idrottsförbund för exponering. Idag hanteras det manuellt och utan stöd i systemet. |
| Känsliga uppgifter | Allergier och hälsa i arrangemangsformulär (gallras 90 dagar efter kurs idag). Kräver samtycke och starkt skydd. |
| Registerutdrag | Krav för alla från 15 år med regelbunden kontakt med barn (stämmobeslut 2020). Gäller 6 månader från utfärdande vid uppvisande och ska förnyas minst vart 3:e år. Innehållet får **inte** lagras. Kontroll av äkthet görs via Polisens tjänst. Kombineras med kursen Trygga möten. |
| Statsbidrag (MUCF) | Bidragsgrundande medlemmar är 6–25 år. Minst 60 % av medlemsföreningen ska vara i åldersspannet och föreningen ska ha minst 5 medlemmar. Ögonblicksbild 31 december. Krav på årsmöte och dokumentation. |
| Kommunalt aktivitetsstöd | Kommunspecifika regler och format (DAK, Excel, CSV). Ålders-, deltagar-, ledar- och tidsgränser. Belopp kan variera med kön. Ändras årligen. |
| Studieförbund | Sensus och andra vill ha närvarolistor. Det finns inga officiella API:er. |
| Medlemsavgift | Förbundsavgift 180 kr per år (2026) inklusive försäkring (Svedea via Willis Towers Watson) och tidningen Scout, plus kåravgift. Avgifterna fördelas till kår, distrikt och förbund. |

## 13. Observerade brister

### 13.1 Användbarhet och design

1. **Två parallella gränssnitt** (v5 och admin) med dubbla instruktioner för samma uppgift.
2. **Djupa flik-i-flik-strukturer** och "Alternativ"-menyer. Funktioner är svåra att hitta. "Webbkoppling" betyder olika saker på olika ställen.
3. **Medlemslistan har bara 4 kolumner** och saknar filter.
   - **Produktion:** en kår med 266 medlemmar ger 668 kB HTML med en `<select>` och en "Uppdatera"-knapp per rad. Bulkvyn är ytterligare 599 kB.
   - **Staging:** samma mönster. 236 medlemmar och cirka 620 kB.
   
   Bulkhantering finns i en separat vy.
4. Rapporter är **filnedladdningar via bakgrundsjobb** i stället för interaktiva, filtrerbara vyer.
5. Rollkatalogen innehåller testskräp och blandar svenska och engelska nycklar.
6. Inga vårdnadshavarkonton. Anhöriga är platta fält även i v5. Familjer som delar e-post kolliderar vid inloggning.
7. Avgiftsändringar är låsta till vissa månader. Produkter kan bara ändras via kodmigrering.
8. BBCode, föråldrade kontroller och en supportwidget som täcker innehåll.
9. **Två gränssnitt i produktion:** v5 och admin har olika design och navigering. Ofta länkar v5 vidare till admin (`/me#…`, `/organisation/…/home`) för funktioner som saknas i v5, till exempel märkesöversikt, CV, medlemshantering och rapporter.
10. **Närvaro i v5** bygger på arrangemang av typen "Möte" med ja/nej-anmälan. Det finns ingen egen terminsvy per avdelning (jämför Skojjt).

### 13.2 Prestanda

Mätt med Navigation Timing (TTFB, serverns svarstid) och en inloggad användare.

| Sida | Produktion | Staging |
|---|---|---|
| Admin startsida (`/`) | **2,9 s** | – |
| Admin kårsida (`/organisation/group/home/{id}`) | **2,3 s** (47 kB) | cirka 1,9 s |
| Admin Min sida (`/me`) | **2,7 s** | – |
| Admin arrangemang | 1,1 s | – |
| Medlemslista (AJAX, 266 medlemmar) | 0,57 s, 668 kB | – |
| v5 startsida och profil | 0,3–0,4 s | – |
| v5 medlemskap (fakturor) | 0,9 s | – |
| v5 märkeskatalog | 2,0 s | – |
| v5 redigera kår | 0,6 s (231 kB HTML) | – |

Tredjepartswidgeten (Freshworks) gör att sidorna inte når "network idle" förrän efter flera sekunder. Apache prefork och synkron PHP används, och tunga uppgifter skickas till Gearman.

### 13.3 Säkerhet

Granskningen (enbart läsande, inga försök till exploatering) gav **13 observationer** om dagens Scoutnet. De flesta är bekräftade i produktion. **Detaljerna publiceras inte** i det här öppna repot förrän de har rapporterats till Scouterna och Custard och har åtgärdats, enligt principen om ansvarsfullt avslöjande. De finns i en konfidentiell bilaga utanför versionshanteringen (`docs/private/`).

Kategorier på övergripande nivå, som underlag för kraven på Scoutnet 2:

| Kategori | Antal | Krav i Scoutnet 2 (se planen 7.5–7.6) |
|---|---|---|
| Webbsäkerhet: HTTP-headers, cookies, föråldrade JavaScript-bibliotek | 3 | Strikta headers, en enda härdad sessionscookie, moderna och uppdaterade beroenden |
| Tredjepartskod och spårning på sidor med personuppgifter | 1 | Inga tredjepartsskript i inloggad app, ingen spårning |
| API-nycklar och tokens (hantering och livslängd) | 2 | OAuth2 via ScoutID, kortlivade tokens, inga nycklar i URL |
| Inloggning och MFA | 2 | Endast ScoutID, MFA-krav per roll |
| Indatahantering och överexponering av data | 3 | Typade parametrar, explicita DTO:er, fältfiltrering per dataklass |
| Publika formulär och prenumerationslänkar | 2 | Teman utan fri CSS, slumpmässiga och återkallbara tokens |

### 13.4 Integration och arkitektur

- Per-endpoint-nycklar, inga scopes, inga webhooks (polling via `updated_since`) och ingen sandbox.
- Inkonsekventa scheman. Dokumentationen sköts av volontärer.
- Leverantörsberoende: stängd källkod, privat repo och drift hos leverantören.
- Monolit med 18 år gammal ramverkskärna. Systemet "plugin-isar" bara delvis.

## 14. Slutsatser för Scoutnet 2

1. **Funktionsytan är mycket stor.** En ersättare måste prioriteras hårt och levereras i etapper, med parallelldrift och ett kompatibilitets-API.
2. **Den största vinsten för användarna** ligger i en enhetlig UX, kraftfulla listor, självservice för ledare, vårdnadshavarkonton och integrerad närvaro med automatiserat bidragsunderlag.
3. **Den största vinsten för säkerheten** ligger i att all inloggning går via **nya ScoutID (Keycloak)** med MFA-krav per roll, passkeys och BankID, i kortlivade och scope-begränsade tokens, i fältnivåskydd för känsliga uppgifter, i loggning av läsningar, i strikta headers och i att inga tredjepartsskript används.
4. **Datamodellen är redan väl förstådd** genom body-trädet, roller per kontext, medlemskap per termin och Skojjts närvaromodell. Den kan förbättras med temporala tabeller, konsekventa enum-värden och förstklassiga relationer för vårdnadshavare och hushåll.
5. **Scouterna har redan en modern plattform** (AKS, Keycloak/ScoutID, CloudNativePG, ArgoCD) som den nya lösningen bör byggas på. ScoutID-providern är byggd för att hämta inloggning, profil och roller från ett medlemssystem via tre API-anrop. Scoutnet 2 kan därför ta över som datakälla utan att ScoutID behöver byggas om.
6. **Kritiskt beroende:** att få ut fullständiga data (databasdump och dokumentation av scheman) från Custard. Det behöver klargöras avtalsmässigt innan migreringen planeras i detalj.
7. **v5 visar riktningen** (mobil, märkestagning, möten, digitalt kort) men lever i två system och gamla arkitekturer. Scoutnet 2 bör ta tillvara det som fungerar i v5, alltså återkommande möten, DAK-flaggan, digitalt medlemskort, loggbok och rabatter, i ett och samma gränssnitt.
