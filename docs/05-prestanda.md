# Prestanda med 100 000 personer

> Mätt 2026-10-18 (prototypvecka 6). Skript: [`scripts/perf.py`](../scripts/perf.py).

## Mål

Från planen ([02-plan.md](./02-plan.md)):
- p95 under 100 ms för en enskild resurs
- p95 under 250 ms för en filtrerad lista med högst 500 rader
- en lista som "känns omedelbar"

## Testdata och miljö

- **Syntetisk data:** `seed --districts 28 --groups 28`, vilket gav:
  - 99 622 personer, varav 77 672 aktiva medlemmar och 10 833 på väntelistan
  - 9 555 enheter (812 kårer, 4 835 avdelningar)
  - 67 866 möten och 638 904 närvaroposter

  Seeden tar 43 s.
- **Server:** release-bygge av `sn-server` mot PostgreSQL 18 i Docker, med standardinställningar (`shared_buffers` 128 MB, `work_mem` 4 MB). Utvecklingsdator med AMD Ryzen 9 5900X (12 kärnor) och 64 GB minne. Docker har 24 CPU och 31 GB.
- **Mätning:** varje scenario körs 3 gånger som uppvärmning och sedan 30 gånger i följd. Tiden mäts i klienten och omfattar HTTP, JSON och behörighetskontroll. *Server* är serverns egen `took_ms`. Samtidighet: 300 blandade anrop från 16 parallella klienter.

## Resultat

| Scenario | Rader | p50 ms | p95 ms |
|---|---:|---:|---:|
| Inloggad användare (`/v1/me`) | | 3 | 4 |
| Enhetsöversikt kår (med antal) | | 11 | 13 |
| Personkort, alla fältgrupper (loggas) | | 32 | 39 |
| Revisionslogg för en person | | 13 | 19 |
| **Kår:** medlemslista, 100 rader | 119 | 20 | 25 |
| Kår: filter gren + scouter, sortering på närvaro | 31 | 17 | 23 |
| Kår: väntelista sorterad på väntetid | 23 | 13 | 16 |
| Avdelning: närvaromatris för terminen | | 22 | 40 |
| Kår: närvarosammanfattning | | 9 | 9 |
| Kår: bidragsunderlag för terminen | | 13 | 22 |
| Kår: statistik (modulen Statistik) | | 23 | 28 |
| **Distrikt:** medlemslista, 100 rader | 2 934 | 36 | 45 |
| Distrikt: ledare utan giltigt registerutdrag | | 35 | 41 |
| **Förbund:** medlemslista, första sidan | 77 672 | 121 | 141 |
| Förbund: sök på efternamn | 1 210 | 36 | 43 |
| Förbund: sök på medlemsnummer | 1 | 25 | 32 |
| Förbund: spårare födda 2017–2018, sortering på namn | 11 808 | 73 | 101 |
| Förbund: sortering på ålder, sida 51 | 77 672 | 125 | 142 |
| Förbund: ledare vars registerutdrag saknas eller går ut | 4 748 | 124 | 144 |
| Förbund: obetald avgift, sortering på avgift | 17 191 | 180 | 209 |
| Förbund: väntelista sorterad på väntetid | 10 833 | 48 | 57 |
| Organisationsträd, två nivåer (812 enheter med antal) | 812 | 158 | 197 |
| Alla avdelningar sorterade på antal scouter | 4 835 | 140 | 176 |
| Förbund: statistik (medlemsutveckling, ålder, grenar, närvaro, väntelista) | | 176 | 188 |
| Scoutnet-kompatibel `memberlist`, hela kåren | 119 | 26 | 42 |
| **Samtidighet:** 16 klienter, blandade listor | 300 anrop | 68 | 148 |

Genomströmning: **210 anrop per sekund** på en server. Alla scenarier har p95 under 250 ms. Allt en kår eller avdelning gör ligger under 50 ms. Raderna om statistik mättes 2026-10-18 efter att modulerna infördes, med samma metod.

## Vad som ändrades i vecka 6

Den första mätningen visade att tiderna växte med **hela databasen**, inte med enhetens storlek:

| Scenario | Före | Efter |
|---|---:|---:|
| Kår: medlemslista (119 personer) | 1 907 ms | 20 ms |
| Kår: översikt med antal | 832 ms | 11 ms |
| Distrikt: medlemslista | 1 813 ms | 36 ms |
| Organisationsträd för hela förbundet | 514 ms | 158 ms |

Orsaker och åtgärder:

1. **Ett filter som inte kunde använda index.** Vyn `member_current` räknade fram personens aktuella avdelning och patrull med två `LATERAL`-uppslag per person. Filtret `effective_path <@ enhet` gällde det framräknade värdet, så Postgres gick igenom alla 100 000 personer innan det filtrerade.
   - **Åtgärd:** läsmodellen `current_placement`, med en rad per person som hålls aktuell av triggers på `placement` ([migration 0009](../crates/sn-server/migrations/0009_current_placement.sql)). Dessutom ett redundant filter på kårens sökväg (`group_scope`) som kan använda GiST-indexet på `body.path`. En kontroll mot den gamla logiken gav 0 avvikelser.
2. **Generiska planer.** sqlx återanvänder förberedda frågor. För hierarkifilter skiljer sig radantalen enormt mellan en patrull och hela förbundet, och den generiska planen (nästlade loopar) blev fel för stora enheter.
   - **Åtgärd:** kårens räckvidd skickas som parameter, och anslutningarna har `plan_cache_mode = force_custom_plan`. Planeringen kostar 1–5 ms.
3. **Tunga fält räknades för alla rader.** Närvaro, registerutdrag, avgift och vårdnadshavare räknades för varje träff, även när bara 100 rader skulle visas.
   - **Åtgärd:** medlemslistan körs i två steg. Urvalet med filter, sortering, sida och totalt antal räknar bara fram det som filter eller sortering behöver. Resten räknas för sidans rader.
   - Avgiftsuppslaget är en vanlig join, så att Postgres kan hash-joina vid stora urval.
4. **Antal per enhet** (översikt, underenheter och träd) räknas på id via `current_placement`. Sökvägarna slås upp sist, i stället för att gå via personerna.

## Moduler

Moduler ([06-moduler.md](./06-moduler.md)) kontrolleras vid varje anrop till en modul: är modulen på för enheten? Det är två små frågor (enhetens sökväg och valen längs sökvägen), och skillnaden syns inte i mätningen: kårens medlemslista och närvaro har samma tider som före modulerna.

Statistik för hela förbundet tog först **1 335 ms**. Tre ändringar gav **188 ms** (p95):

- **Ett svep.** Totaler, åldrar och grenar räknas i en genomgång av medlemmarna.
- **Grupperade giltighetstider.** Medlemsutvecklingen grupperar medlemskapen på giltighetstid innan varje terminsstart prövas, i stället för att sortera 430 000 rader.
- **Samtidiga delfrågor.** De oberoende delarna körs samtidigt på olika anslutningar (`tokio::try_join!`).

## Kvar att göra inför produktion

- Mät med produktionslik konfiguration (`shared_buffers`, `work_mem`, anslutningspool, repliker för läsning) och med verkliga fördelningar från Scoutnet. Stora kårer har över 500 medlemmar.
- Listor över hela förbundet (120–200 ms) används av få. Om de behöver bli snabbare: en materialiserad sorteringsnyckel, eller nyckelbaserad paginering i stället för `OFFSET`.
- p95 och p99 per endpoint i drift via OpenTelemetry (planen, avsnitt 1: mätbara mål), inte bara i ett skript.
- Belastningstest med realistiska toppar, till exempel terminsstart och när många registrerar närvaro samtidigt efter möten en vardagskväll.
