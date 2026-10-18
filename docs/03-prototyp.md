# Prototyp: Scoutnet 2 på 6 veckor

> Skunk works-prototyp för att **visa hur ett nytt Scoutnet kan se ut och fungera**. Den ska demonstreras på nästa E-tjänstmöte.
> Start 2026-10-16. Demo vecka 6 (cirka 2026-11-27).
> Prototypen följer riktningen i [02-plan.md](./02-plan.md): Rust, API först, ScoutID och PostgreSQL. Den skär medvetet hörn där det inte syns i demon.

## Mål med demon

Publiken ska efter 20 minuter säga *"det där vill vi ha"*. Därför visar demon:

1. **Snabbt och enhetligt:** samma kraftfulla lista överallt. Filtrera, sortera och välj kolumner, med svar på under 100 ms även för 100 000 personer.
2. **Rätt person ser rätt saker:** en avdelningsledare ser sin avdelning och en medlemsregistrerare ser sin kår. Personnummer maskas om man inte har särskild behörighet.
3. **Närvaro inbyggt:** planera terminens möten, bocka närvaro i mobilen och få bidragsunderlag (DAK) direkt.
4. **API först:** allt som UI:t gör finns i en publik OpenAPI-dokumentation (`/docs`).
5. **ScoutID:** inloggning via nya ScoutID (Keycloak). I prototypen används en lokal Keycloak som efterliknar ScoutID:s claims, och dessutom snabbinloggning med personas för demon.

## Avgränsningar (medvetet utanför prototypen)

Följande ingår inte: fakturering och betalningar (terminsavgifternas status visas, men fakturor skapas inte), statsbidrag, arrangemang och kurser, e-postutskick, migrering från gamla Scoutnet, produktionsdrift, fältkryptering, Cedar (behörigheten görs i en enkel Rust-modul med samma semantik) och fullständig tillgänglighetsgranskning.

**All data är syntetisk.** Personnumren följer Skatteverkets konvention för testpersonnummer (födelsenummer 980–999). Inga riktiga personuppgifter förekommer.

**Varumärke:** prototypen använder Scouternas officiella logotyp från [grafisk profil](https://www.scouterna.se/scout-ledare-kar/kommunicera/grafisk-profil/) (`web/static/brand/`). Logotypen är Scouternas varumärke och omfattas inte av MIT-licensen.

## Status

| Vecka | Klart |
|---|---|
| 1 | Skelett, domäntyper, schema, syntetisk data (cirka 2 500 personer), API för organisation, medlemmar och personer, OpenAPI med Scalar, personas, behörighet per enhet med fältmaskning, UI med skal, översikt, medlemslista, väntelista och Min profil. Rökprov med 36 kontroller. |
| 1+ | **Alla personfält som Scoutnet har** ([fältkatalog](./04-faltkatalog.md)): kontakt, adresser med typ, anhöriga i ordning, hushåll, medlemsstatus, avgiftsgrupp och terminsavgifter med KID, registerutdrag, uppföljningssamtal, meriter, samtycken, utskicks- och fakturaval, kårens egna fält, noteringar och rollhistorik. Personkortet har flikar. Medlemslistan har filtren "Registerutdrag saknas/går ut" och "Obetald avgift". Behörigheten `economy_read` är ny. Scouternas officiella logotyp används. |
| 1++ | **Läsbara adresser och organisationsöversikt:** `/kar/1001`, `/avdelning/5003` och `/distrikt/1` i stället för UUID (alla enheter har nummer, unikt per typ). API:t godtar `group-1001`. Gamla `/enhet/{uuid}`- och Skojjt-adresser (`/sk/…`) skickas vidare. Klickbara brödsmulor på alla enhetssidor. Ny vy *Organisation* med träd (fäll ut till kårer, avdelningar, patruller eller personer, med antal per enhet) och sorterbar lista med summor. Rökprov med 44 kontroller. |
| 2 | **Inloggning via ScoutID** (OIDC authorization code + PKCE, BFF) mot en lokal Keycloak med realmen `scoutid` som efterliknar nya ScoutID. Kontot kopplas via `sub` och första gången via `scoutnet_member_no`. `auth_time` och `acr` från ScoutID styr step-up. Utloggning även i ScoutID. Okänt ScoutID ger ett begripligt meddelande, och medlemmar utan konto får ett vid första inloggningen. **Sparade vyer på servern**, egna eller delade med alla som ser listan i enheten. Lokal git med första commits. |
| 3 | **Närvaro:** terminsplanering (veckoserier med överhoppade lov, svensk tid), närvarolista per avdelning och termin (matris), registrering i mobilen med ett tryck per scout och autosparning, förhinder, ändra och ta bort möten, översikt per kår med "möten som saknar närvaro" och genväg från översikten. Läs- och skrivbehörighet skiljs åt (registrerare läser, ledare registrerar). |
| 4 | **Bidrag:** Göteborgs regler för aktivitetsbidrag 2026 som data. Varje sammankomst kontrolleras (ålder, deltagare, ledare, längd) med förklaring, och bidraget beräknas per kår och avdelning. **DAK 2.2-export** per avdelning, med samma struktur som Skojjt, kräver behörighet att se personnummer och färsk inloggning. **iCal-prenumeration** med återkallbar token utan personuppgifter. Rökprov med 69 kontroller. |
| 5 | **Massåtgärder:** markera i medlemslistan eller väntelistan (shift-klick för intervall) och flytta till en avdelning eller patrull, eller ta in från väntelistan. Förhandsgranskning per rad innan något ändras. **Vem har sett mina uppgifter:** varje medlem ser i Min profil vem som har öppnat personkortet, visat personnumret, flyttat personen eller hämtat uppgifterna via en integration. Kårordförande ser samma logg i personkortet. **Integrationer:** API-nycklar per kår med behörigheter (kontaktuppgifter och personnummer var för sig) och `GET /api/group/memberlist` i Scoutnets format, så att Skojjt och andra integrationer kan byta adress utan omskrivning. Rökprov med 90 kontroller. |
| 6 | **Prestanda med 100 000 personer:** alla mätta anrop har p95 under 250 ms, och allt på kår- och avdelningsnivå under 50 ms ([mätresultat](./05-prestanda.md)). Läsmodellen `current_placement` (aktuell avdelning och patrull, uppdateras av triggers) ersätter två uppslag per person. Filter på en enhet kan nu använda index, och listor över hela förbundet räknar bara fram tunga fält för sidans rader. Den gamla frågan tog 1,9 s för en kår med 119 medlemmar, den nya tar 20 ms. |
| 7 | **Moduler** ([06-moduler.md](./06-moduler.md)): Scoutnet 2 är en kärna och moduler som slås på och av per förbund, distrikt eller kår. Ett val ärvs nedåt, förbund och distrikt kan låsa, och beroenden kontrolleras (Bidrag kräver Närvaro). Närvaro, Bidrag och Integrationer är flyttade till moduler, och **Statistik** är en ny modul (medlemsutveckling, ålder och kön, scouter per ledare, närvaro per gren, väntelista). Menyn byggs av modulerna. En avstängd modul ger 404 `module_disabled`, och gamla länkar visar en förklaring. Fakturering, E-postlistor och Aktiviteter visas som planerade. Rökprov med 109 kontroller. |

## Demo-manus (utkast)

| # | Scen | Visar |
|---|---|---|
| 1 | Logga in som **avdelningsledare** via ScoutID (lokal Keycloak, medlemsnummer 9000033) | ScoutID-inloggning, man kommer tillbaka till rätt sida, startsida med "Registrera närvaro" |
| 2 | Medlemslistan: filtrera "Spårare födda 2017–2018 som saknar e-post hos vårdnadshavare", sortera och spara som delad vy | Kraftfull lista, URL-delbara vyer, svarstid |
| 3 | Byt till **avdelningsledare**: samma lista visar bara den egna avdelningen, och personnumret är maskat | Behörighet och dataminimering |
| 3b | Som **kårordförande**: filtret "Registerutdrag saknas/går ut" på ledarna, öppna personkortet och se registerutdrag, uppföljningssamtal och avgifter i flikar | Alla Scoutnets fält, men begripligt ordnade |
| 3c | Som **distriktsordförande**: *Organisation* → "Visa ned till avdelningar", sedan *Lista* → alla spåraravdelningar i distriktet sorterade på antal scouter. Klicka vidare via brödsmulorna. | Överblick över många enheter, läsbara och delbara adresser |
| 4 | Mobil: från översikten "Registrera närvaro", tryck på scouterna, klart (sparas automatiskt). Planera vårterminen med lovveckor överhoppade. Prenumerera på mötena i kalendern. | Närvaro, mobil, iCal |
| 5 | Som **medlemsregistrerare**: *Bidrag* för kåren, se varför Familjescouterna får underkända sammankomster, ladda ner DAK-filen | Bidragsregler som data, DAK |
| 5b | Som **medlemsregistrerare**: *Väntelista*, markera de tre som väntat längst, *Ta in i avdelning* → Spårhundarna. Förhandsgranskningen visar vad som händer, sedan genomför. | Massåtgärder med torrkörning, i stället för en person i taget |
| 5c | *Integrationer*: skapa en nyckel för "Skojjt närvaro" med kontaktuppgifter, kör `curl` med nyckeln och visa att svaret har samma format som dagens Scoutnet. Återkalla nyckeln. | Befintliga integrationer kan flytta utan omskrivning, och nycklar har behörigheter |
| 5d | Logga in som **scouten** (9000080): *Min profil* → "Vem har sett mina uppgifter" visar registrarens visning och integrationens hämtning med namn | Insyn för medlemmen (GDPR), revisionslogg |
| 6 | Öppna `/docs` och kör samma anrop via API:t | API först |
| 7 | Prestanda: kör `python scripts/perf.py` mot 100 000 syntetiska personer. Som **förbundsadministratör**: hela förbundets medlemslista, sök på namn och organisationsträdet. | Skalbarhet: kår under 25 ms, hela förbundet 120–200 ms ([mätresultat](./05-prestanda.md)) |
| 8 | Som **kårordförande**: *Moduler*, stäng av Bidrag. Menyn ändras för kåren och dess avdelningar, och en gammal länk till Bidrag visar en förklaring. Försök stänga av Närvaro: "Bidrag kräver Närvaro". Slå på Bidrag igen. | Scoutnet 2 byggs av moduler som kåren väljer |
| 8b | Som **distriktsordförande**: *Moduler* för distriktet, stäng av och lås Statistik. Visa att kåren ser "Låst av Älvstadens scoutdistrikt". Ta bort låset och öppna *Statistik* för distriktet och en kår. | Arv och låsning i organisationen, och en ny modul byggd på en dag |

## Demo-miljö och generalrepetition

Allt körs lokalt på demodatorn med ett kommando:

```powershell
./scripts/demo.ps1 -Perf
```

Skriptet gör följande:
- startar PostgreSQL och ScoutID (Keycloak) i Docker
- bygger servern (release) och webbappen
- fyller demodatabasen med färsk syntetisk data
- startar Scoutnet 2 på <http://127.0.0.1:8080>

Med `-Perf` startar det också en server med 100 000 personer på <http://127.0.0.1:8081>, med bara demoroller. Den databasen seedas bara första gången. För en snabb omstart utan bygge och ny data: `./scripts/demo.ps1 -SkipBuild -KeepData`.

Checklista, dagen före och en timme före:

1. Kör `./scripts/demo.ps1 -Perf` med nätverk. Första gången hämtas Docker-avbilderna och beroendena, sedan behövs inget nätverk.
2. I ett annat fönster: `python scripts/smoke.py` (109 kontroller, alla ska vara OK) och `python scripts/perf.py` (alla scenarier under 250 ms).
3. Logga in en gång med ScoutID som 9000033 för att se att Keycloak svarar. Testkonton finns i [deploy/keycloak/README.md](../deploy/keycloak/README.md).
4. Gå igenom manuset ovan med varje roll. Seeden ger samma personas och medlemsnummer varje dag, men övriga namn kan variera med datumet.
5. Projektor: webbläsarens zoom på 125–150 %, ljust läge och ett eget webbläsarfönster utan andra flikar.
6. Reserv: en skärminspelning av hela manuset, om datorn eller Docker strular.

## Veckoplan

| Vecka | Leverans |
|---|---|
| 1 | Repo och skelett, domäntyper (personnummer), databasschema, syntetisk data, API för organisation och medlemmar med filter, sortering och paginering, OpenAPI-dokumentation, snabbinloggning med personas, behörighet per kår och avdelning, första UI: skal och medlemslista |
| 2 | Persondetalj (vårdnadshavare, roller, historik), väntelista, kontextväljare, sparade vyer. Lokal Keycloak med en realm som efterliknar ScoutID-claims (`scoutnet_member_no`), OIDC-inloggning via BFF. |
| 3 | Närvaro: terminsplanering (återkommande möten, flera per dag), mobilvy för närvaro, statistik per person och avdelning |
| 4 | Bidrag: DAK-export, regler för Göteborg som data och validering per sammankomst. iCal med slumpmässig token. |
| 5 | Bulkåtgärder (flytta avdelning) med torrkörning, ta in från väntelista, revisionslogg ("vem har sett mina uppgifter"), kompatibilitets-endpointen `group/memberlist` |
| 6 | Finputs, prestandademo med 100 000 personer, demo-miljö, demo-manus, generalrepetition |

## Teknik i prototypen

- **Backend:** Rust 1.99 (edition 2024), axum, tokio, sqlx (PostgreSQL 18), utoipa (OpenAPI 3.1) och Scalar. Crates: `sn-domain` (rena domäntyper) och `sn-server` (API, behörighet, seed). Funktionsområdena ligger som moduler i `sn-server/src/modules/` med ett gemensamt kontrakt ([06-moduler.md](./06-moduler.md)).
- **Databas:** PostgreSQL 18 i Docker: `ltree` för organisationsträdet, `pg_trgm` för sök och den inbyggda `uuidv7()`. Läsmodellen `current_placement` hålls aktuell av triggers. Anslutningarna planerar varje fråga med de verkliga parametrarna (`plan_cache_mode = force_custom_plan`), eftersom samma fråga kan gälla en patrull eller hela förbundet.
- **Frontend:** SvelteKit (Svelte 5, TypeScript) som SPA, med Vite-proxy mot API:t i utveckling.
- **Inloggning:** sessionscookie (BFF). OIDC (authorization code + PKCE) mot ScoutID, i utveckling en lokal Keycloak i docker compose ([deploy/keycloak](../deploy/keycloak/README.md)). Personas för snabbinloggning i utvecklingsläge (`SN_DEV_LOGIN=true`).

## Köra lokalt

Se [README](../README.md#kom-igång-prototypen).
