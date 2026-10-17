# Scoutnet 2

Förstudie och plan för ett modernt, API-först medlemssystem för Scouterna, skrivet i Rust. Det ska ersätta dagens [Scoutnet](https://scoutnet.se/) och integrera närvaroregistrering från [Skojjt](https://github.com/Scouterna/skojjt-v2). **Inloggning sker uteslutande via nya ScoutID (Keycloak).**

## Dokument

| Dokument | Innehåll |
|---|---|
| [docs/01-analys-av-scoutnet.md](docs/01-analys-av-scoutnet.md) | Analys av dagens Scoutnet, baserad på produktion (v5 och admin 4.26.12.1) och staging: historik, teknik, datamodell, roller och behörigheter, funktioner, API:er, ekosystem (inklusive nya ScoutID), regelverk och observerade brister (UX, prestanda, säkerhet) |
| [docs/02-plan.md](docs/02-plan.md) | Implementationsplan: mål, arkitektur, Rust-policy, domänmodell, API-design, säkerhet och inloggning via ScoutID, GDPR, UX, närvaro och bidrag, integrationer, migrering (inklusive flytt av lösenord till ScoutID), faser, risker och beslut |
| [docs/03-prototyp.md](docs/03-prototyp.md) | Skunk works-prototyp på 6 veckor: mål, demo-manus, veckoplan och status |
| [docs/04-faltkatalog.md](docs/04-faltkatalog.md) | Alla personfält i Scoutnet med motsvarighet i Scoutnet 2: API-namn, tabell, dataklass, behörighet och kompatibilitetsnamn |

## Status

Förstudie och **prototyp under arbete** (se [03-prototyp.md](docs/03-prototyp.md#status)). All data i prototypen är syntetisk.

## Kom igång (prototypen)

Krav: Rust (versionen väljs automatiskt av `rust-toolchain.toml`), Docker, Node 24 och Python 3 för rökprovet.

```powershell
docker compose up -d                              # PostgreSQL 18 på 127.0.0.1:5433
cargo run -p sn-server --bin seed                 # migrerar och fyller med syntetisk data (cirka 2 500 personer)
cd web; npm ci; npm run build; cd ..              # bygger webbappen till web/build
cargo run -p sn-server --bin sn-server            # http://127.0.0.1:8080 (API, webbapp och /docs)
```

- Öppna <http://127.0.0.1:8080> och välj en demoroll (medlemsregistrerare, avdelningsledare, scout, vårdnadshavare …). Enheter har läsbara adresser, t.ex. <http://127.0.0.1:8080/kar/1001> och <http://127.0.0.1:8080/distrikt/1/organisation>.
- API-dokumentation (OpenAPI 3.1, Scalar): <http://127.0.0.1:8080/docs>. Råspecifikationen finns på `/openapi.json`.
- Utveckling av webben med hot reload: `cd web; npm run dev` (proxar API-anrop till port 8080). Typer från API:t genereras med `npm run api:types`.
- Kvalitetskontroller: `cargo clippy --workspace --all-targets`, `cargo test --workspace`, `cd web; npm run check` och rökprovet `python scripts/smoke.py` (servern ska vara igång).
- Storlek på testdata: `cargo run -p sn-server --bin seed -- --districts 4 --groups 5 --seed 42`.

Snabbinloggningen med personas finns bara i utvecklingsläge (`SN_DEV_LOGIN=true`, standard i prototypen). I produktion sker all inloggning via ScoutID.

## Licens

[MIT](LICENSE) © Scouterna. Scouternas logotyp och scoutsymbol i `web/static/brand/` är Scouternas varumärken och omfattas inte av MIT-licensen (se [web/static/brand/README.md](web/static/brand/README.md)).

> **Obs:** Filer med inloggningsuppgifter får aldrig checkas in. De är undantagna i `.gitignore`. Dokumentationen innehåller inga personuppgifter från produktion och ska inte heller få några.
