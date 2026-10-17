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

## Demo-manus (utkast)

| # | Scen | Visar |
|---|---|---|
| 1 | Logga in som **medlemsregistrerare** i en kår | ScoutID-inloggning, startsida med att göra-lista |
| 2 | Medlemslistan: filtrera "Spårare födda 2017–2018 som saknar e-post hos vårdnadshavare", sortera och spara som vy | Kraftfull lista, URL-delbara vyer, svarstid |
| 3 | Byt till **avdelningsledare**: samma lista visar bara den egna avdelningen, och personnumret är maskat | Behörighet och dataminimering |
| 3b | Som **kårordförande**: filtret "Registerutdrag saknas/går ut" på ledarna, öppna personkortet och se registerutdrag, uppföljningssamtal och avgifter i flikar | Alla Scoutnets fält, men begripligt ordnade |
| 3c | Som **distriktsordförande**: *Organisation* → "Visa ned till avdelningar", sedan *Lista* → alla spåraravdelningar i distriktet sorterade på antal scouter. Klicka vidare via brödsmulorna. | Överblick över många enheter, läsbara och delbara adresser |
| 4 | Mobil: öppna dagens möte och bocka närvaro med tre tryck | Närvaro, PWA |
| 5 | Ta ut DAK-fil och bidragsunderlag för Göteborg med valideringsmeddelanden | Bidragsmotor |
| 6 | Öppna `/docs` och kör samma anrop via API:t | API först |
| 7 | Prestanda: 100 000 syntetiska personer och p95 i loggen | Skalbarhet |

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

- **Backend:** Rust 1.99 (edition 2024), axum, tokio, sqlx (PostgreSQL 18), utoipa (OpenAPI 3.1) och Scalar. Crates: `sn-domain` (rena domäntyper) och `sn-server` (API, behörighet, seed).
- **Databas:** PostgreSQL 18 i Docker: `ltree` för organisationsträdet, `pg_trgm` för sök och den inbyggda `uuidv7()`.
- **Frontend:** SvelteKit (Svelte 5, TypeScript) som SPA, med Vite-proxy mot API:t i utveckling.
- **Inloggning:** sessionscookie (BFF). Personas i utvecklingsläge (`SN_DEV_LOGIN=true`), OIDC mot lokal Keycloak eller ScoutID (vecka 2).

## Köra lokalt

Se [README](../README.md#kom-igång-prototypen).
