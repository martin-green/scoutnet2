"""Prestandamätning mot en server med stor syntetisk data (planen: 100 000 personer).

Förbered (separat databas, så att demodatan inte påverkas):
  docker exec <pg> psql -U scoutnet2 -c "CREATE DATABASE scoutnet2_perf OWNER scoutnet2"
  $env:DATABASE_URL = "postgres://scoutnet2:scoutnet2-dev-only@127.0.0.1:5434/scoutnet2_perf"
  cargo run --release -p sn-server --bin seed -- --districts 28 --groups 28
  $env:SN_BIND = "127.0.0.1:8091"; cargo run --release -p sn-server --bin sn-server

Kör: python scripts/perf.py [--base http://127.0.0.1:8091] [--n 40]
"""
import argparse
import base64
import http.cookiejar
import json
import statistics
import sys
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor

ap = argparse.ArgumentParser()
ap.add_argument("--base", default="http://127.0.0.1:8091")
ap.add_argument("--n", type=int, default=40, help="antal anrop per scenario")
ap.add_argument("--concurrency", type=int, default=16)
args = ap.parse_args()
BASE = args.base


def client():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


def call(op, method, path, body=None, headers=None):
    req = urllib.request.Request(BASE + path, data=json.dumps(body).encode() if body is not None else None, method=method)
    req.add_header("content-type", "application/json")
    req.add_header("accept-encoding", "identity")
    for k, v in (headers or {}).items():
        req.add_header(k, v)
    with op.open(req) as r:
        raw = r.read()
        return r.status, (json.loads(raw) if raw and "json" in r.headers.get("content-type", "") else raw)


def login(prefix):
    op = client()
    _, personas = call(op, "GET", "/auth/dev/personas")
    p = next(p for p in personas if p["persona"].startswith(prefix))
    call(op, "POST", "/auth/dev/login", {"account_id": p["account_id"]}, {"x-sn-csrf": "1"})
    return op


def pct(values, p):
    s = sorted(values)
    return s[min(len(s) - 1, round(p / 100 * (len(s) - 1)))]


results = []


def measure(name, op, path, headers=None, n=None):
    n = n or args.n
    for _ in range(3):
        call(op, "GET", path, headers=headers)
    client_ms, server_ms, size = [], [], 0
    for _ in range(n):
        t = time.perf_counter()
        _, data = call(op, "GET", path, headers=headers)
        client_ms.append((time.perf_counter() - t) * 1000)
        if isinstance(data, dict) and "took_ms" in data:
            server_ms.append(data["took_ms"])
        if isinstance(data, dict):
            size = data.get("total", len(data.get("items", data.get("data", data.get("entries", [])))))
    row = {
        "name": name,
        "p50": statistics.median(client_ms),
        "p95": pct(client_ms, 95),
        "max": max(client_ms),
        "server_p95": pct(server_ms, 95) if server_ms else None,
        "rows": size,
    }
    results.append(row)
    srv = f"  (server p95 {row['server_p95']:.1f} ms)" if row["server_p95"] is not None else ""
    print(f"{name:<58} p50 {row['p50']:6.1f}  p95 {row['p95']:6.1f}  max {row['max']:6.1f} ms  [{size}]{srv}")


admin = login("Förbundsadministratör")
reg = login("Medlemsregistrerare")
kar = login("Kårordförande")
dist = login("Distriktsordförande")
leader = login("Avdelningsledare")

_, ame = call(admin, "GET", "/v1/me")
org = next(c for c in ame["contexts"] if c["body_kind"] == "organisation")
_, root = call(admin, "GET", f"/v1/bodies/{org['body_id']}")
print(f"Data: {root['counts']['active']} aktiva medlemmar, {root['counts']['waiting']} på väntelistan i {org['body_name']}\n")

_, rme = call(reg, "GET", "/v1/me")
group = next(c for c in rme["contexts"] if c["body_kind"] == "group")
gkey = f"group-{group['body_number']}"
_, dme = call(dist, "GET", "/v1/me")
district = next(c for c in dme["contexts"] if c["body_kind"] == "district")
_, lme = call(leader, "GET", "/v1/me")
troop = next(c for c in lme["contexts"] if c["body_kind"] == "troop")
tkey = f"troop-{troop['body_number']}"
_, gpage = call(reg, "GET", f"/v1/bodies/{gkey}/members?limit=1")
person = gpage["items"][0]["id"]
okey = org["body_id"]

print("Enskilda resurser")
measure("Inloggad användare (/v1/me)", reg, "/v1/me")
measure("Enhetsöversikt kår", reg, f"/v1/bodies/{gkey}")
measure("Personkort (alla fältgrupper, loggas)", reg, f"/v1/people/{person}")
measure("Revisionslogg för en person", kar, f"/v1/people/{person}/audit")

print("\nKår (~130 medlemmar)")
measure("Medlemslista, 100 rader", reg, f"/v1/bodies/{gkey}/members")
measure("Filter gren + scouter, sortering på närvaro", reg,
        f"/v1/bodies/{gkey}/members?troop_type=sparare,upptackare&leaders=false&sort=-attendance")
measure("Väntelista sorterad på väntetid", reg, f"/v1/bodies/{gkey}/members?status=waiting&sort=waiting_since")
measure("Närvaromatris avdelning (termin)", leader, f"/v1/bodies/{tkey}/attendance")
measure("Närvarosammanfattning kår", kar, f"/v1/bodies/{gkey}/attendance/summary")
measure("Bidragsunderlag kår (termin)", kar, f"/v1/bodies/{gkey}/grants")
measure("Statistik kår (modul)", kar, f"/v1/bodies/{gkey}/statistics")

print(f"\nDistrikt ({district['body_name']})")
measure("Medlemslista distrikt, 100 rader", dist, f"/v1/bodies/{district['body_id']}/members")
measure("Distrikt: ledare utan giltigt registerutdrag", dist,
        f"/v1/bodies/{district['body_id']}/members?leaders=true&police_check=missing,expired&sort=police_check")

print("\nHela förbundet")
measure("Medlemslista förbund, första sidan (100)", admin, f"/v1/bodies/{okey}/members")
measure("Förbund: sök på efternamn", admin, f"/v1/bodies/{okey}/members?q=Lindstr")
measure("Förbund: sök på medlemsnummer", admin, f"/v1/bodies/{okey}/members?q=9012345")
measure("Förbund: spårare födda 2017–2018, sortering namn", admin,
        f"/v1/bodies/{okey}/members?troop_type=sparare&born_from=2017&born_to=2018&sort=name")
measure("Förbund: sortering på ålder, sida 51 (offset 5000)", admin,
        f"/v1/bodies/{okey}/members?sort=-birth_date&offset=5000")
measure("Förbund: ledare med registerutdrag som saknas/går ut", admin,
        f"/v1/bodies/{okey}/members?leaders=true&police_check=missing,expired,expiring&sort=police_check")
measure("Förbund: obetald avgift, sortering på avgift", admin, f"/v1/bodies/{okey}/members?fee_status=unpaid,overdue&sort=fee")
measure("Förbund: väntelista sorterad på väntetid", admin, f"/v1/bodies/{okey}/members?status=waiting&sort=waiting_since")
measure("Organisationsträd, två nivåer", admin, f"/v1/bodies/{okey}/descendants?depth=2")
measure("Alla avdelningar sorterade på antal scouter", admin,
        f"/v1/bodies/{okey}/descendants?kind=troop&sort=-scouts&limit=100")
measure("Statistik för hela förbundet (modul)", admin, f"/v1/bodies/{okey}/statistics", n=max(5, args.n // 3))

print("\nIntegrationer")
_, key = call(reg, "POST", f"/v1/bodies/{gkey}/api-keys", {"name": "perf", "scopes": ["memberlist:contact"]}, {"x-sn-csrf": "1"})
basic = {"authorization": "Basic " + base64.b64encode(f"{group['body_number']}:{key['key']}".encode()).decode()}
measure("Scoutnet-kompatibel memberlist, hela kåren", client(), "/api/group/memberlist", basic)
call(reg, "DELETE", f"/v1/api-keys/{key['info']['id']}", headers={"x-sn-csrf": "1"})

# Samtidiga användare: blandade listor från många sessioner.
paths = [
    (reg, f"/v1/bodies/{gkey}/members"),
    (leader, f"/v1/bodies/{tkey}/attendance"),
    (dist, f"/v1/bodies/{district['body_id']}/members"),
    (admin, f"/v1/bodies/{okey}/members?q=Ander"),
    (kar, f"/v1/bodies/{gkey}/attendance/summary"),
]
total = args.n * 10


def one(i):
    op, path = paths[i % len(paths)]
    t = time.perf_counter()
    call(op, "GET", path)
    return (time.perf_counter() - t) * 1000


t0 = time.perf_counter()
with ThreadPoolExecutor(args.concurrency) as ex:
    lat = list(ex.map(one, range(total)))
wall = time.perf_counter() - t0
print(f"\nSamtidighet: {total} blandade anrop med {args.concurrency} parallella klienter på {wall:.1f} s "
      f"= {total / wall:.0f} anrop/s, p50 {statistics.median(lat):.1f} ms, p95 {pct(lat, 95):.1f} ms")

slow = [r for r in results if r["p95"] > 250]
print("\nAlla scenarier har p95 under 250 ms." if not slow else f"\nÖver 250 ms (p95): {[r['name'] for r in slow]}")
sys.exit(0)
