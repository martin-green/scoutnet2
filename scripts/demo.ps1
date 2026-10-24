<#
.SYNOPSIS
    Startar demomiljön för Scoutnet 2-prototypen på en dator.

.DESCRIPTION
    1. Startar PostgreSQL och den lokala ScoutID (Keycloak) i Docker.
    2. Bygger servern (release) och webbappen.
    3. Fyller demodatabasen med färsk syntetisk data (cirka 2 500 personer).
    4. Med -Perf: fyller databasen scoutnet2_perf med cirka 100 000 personer (bara första gången)
       och startar en andra server på http://127.0.0.1:8091 för prestandademon.
    5. Startar Scoutnet 2 på http://127.0.0.1:8090. Avsluta med Ctrl+C.

.EXAMPLE
    ./scripts/demo.ps1              # demo med standarddata
    ./scripts/demo.ps1 -Perf        # även 100 000 personer på port 8091
    ./scripts/demo.ps1 -SkipBuild -KeepData   # snabb omstart utan bygge och utan ny data
#>
param(
    [switch]$Perf,
    [switch]$SkipBuild,
    [switch]$KeepData
)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$devUrl = 'postgres://scoutnet2:scoutnet2-dev-only@127.0.0.1:5434/scoutnet2'
$perfUrl = 'postgres://scoutnet2:scoutnet2-dev-only@127.0.0.1:5434/scoutnet2_perf'
$server = Join-Path (Get-Location) 'target\release\sn-server.exe'
$seed = Join-Path (Get-Location) 'target\release\seed.exe'

function Step([string]$text) { Write-Host "`n==> $text" -ForegroundColor Cyan }
function Assert-Ok([string]$what) { if ($LASTEXITCODE -ne 0) { throw "$what misslyckades (kod $LASTEXITCODE)" } }
function Sql([string]$db, [string]$sql) { (docker compose exec -T db psql -U scoutnet2 -d $db -tAc $sql | Out-String).Trim() }

Step 'Startar PostgreSQL och ScoutID (Keycloak) i Docker'
docker compose up -d --wait db; Assert-Ok 'docker compose up db'
docker compose up -d keycloak; Assert-Ok 'docker compose up keycloak'

if (-not $SkipBuild) {
    Step 'Bygger servern (release)'
    cargo build --release -p sn-server --bins; Assert-Ok 'cargo build'
    Step 'Bygger webbappen'
    Push-Location web
    try {
        if (-not (Test-Path node_modules)) { npm ci; Assert-Ok 'npm ci' }
        npm run build; Assert-Ok 'npm run build'
    } finally { Pop-Location }
}

if (-not $KeepData) {
    Step 'Fyller demodatabasen med syntetisk data'
    $env:DATABASE_URL = $devUrl
    & $seed; Assert-Ok 'seed'
}

$perfProc = $null
if ($Perf) {
    if ((Sql 'scoutnet2' "SELECT 1 FROM pg_database WHERE datname = 'scoutnet2_perf'") -ne '1') {
        Sql 'scoutnet2' 'CREATE DATABASE scoutnet2_perf OWNER scoutnet2' | Out-Null
    }
    $persons = Sql 'scoutnet2_perf' "SELECT CASE WHEN to_regclass('person') IS NULL THEN 0 ELSE (SELECT count(*) FROM person) END"
    if ([int]$persons -lt 90000) {
        Step 'Fyller scoutnet2_perf med cirka 100 000 personer (under en minut)'
        $env:DATABASE_URL = $perfUrl
        & $seed --districts 28 --groups 28; Assert-Ok 'seed (perf)'
        Sql 'scoutnet2_perf' 'VACUUM ANALYZE' | Out-Null
    }
    Step 'Startar prestandaservern på http://127.0.0.1:8091 (100 000 personer, bara demoroller)'
    $env:DATABASE_URL = $perfUrl
    $env:SN_BIND = '127.0.0.1:8091'
    $env:SN_OIDC_ISSUER = ' '   # blanksteg stänger av ScoutID (redirect-adressen är registrerad för port 8090)
    $env:RUST_LOG = 'warn'
    $perfProc = Start-Process -FilePath $server -PassThru -NoNewWindow
}

$env:DATABASE_URL = $devUrl
$env:SN_BIND = '127.0.0.1:8090'
Remove-Item Env:SN_OIDC_ISSUER, Env:RUST_LOG -ErrorAction SilentlyContinue

Step 'Väntar på ScoutID (Keycloak)'
$ready = $false
for ($i = 0; $i -lt 40 -and -not $ready; $i++) {
    try {
        $ready = (Invoke-WebRequest -UseBasicParsing -TimeoutSec 3 http://127.0.0.1:8180/realms/scoutid/.well-known/openid-configuration).StatusCode -eq 200
    } catch { Start-Sleep -Seconds 3 }
}
if (-not $ready) { Write-Warning 'Keycloak svarar inte. Demorollerna fungerar ändå, men inte ScoutID-inloggningen.' }

Step 'Startar Scoutnet 2 på http://127.0.0.1:8090'
Write-Host 'Logga in med en demoroll, eller med ScoutID (testkonton i deploy/keycloak/README.md, lösenord scout).'
if ($perfProc) { Write-Host 'Prestandademo: http://127.0.0.1:8091 och python scripts/perf.py' }
try {
    & $server
} finally {
    if ($perfProc) { Stop-Process -Id $perfProc.Id -ErrorAction SilentlyContinue }
}
