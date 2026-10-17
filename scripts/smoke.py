"""Rökprov mot lokal server: inloggning med personas, behörighet och medlemslistan.

Kör: python scripts/smoke.py   (servern ska vara igång på http://127.0.0.1:8080 med seedad data)
"""
import http.cookiejar
import json
import sys
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
ok = True


def client():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


def call(op, method, path, body=None, headers=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, method=method)
    req.add_header("content-type", "application/json")
    for k, v in (headers or {}).items():
        req.add_header(k, v)
    try:
        with op.open(req) as r:
            raw = r.read()
            is_json = r.headers.get("content-type", "").startswith("application/")
            return r.status, (json.loads(raw) if raw and is_json else raw), {k.lower(): v for k, v in r.headers.items()}
    except urllib.error.HTTPError as e:
        raw = e.read()
        return e.code, (json.loads(raw) if raw else None), {k.lower(): v for k, v in e.headers.items()}


def login(persona_prefix):
    op = client()
    _, personas, _ = call(op, "GET", "/auth/dev/personas")
    p = next(p for p in personas if p["persona"].startswith(persona_prefix))
    st, _, _ = call(op, "POST", "/auth/dev/login", {"account_id": p["account_id"]}, {"x-sn-csrf": "1"})
    assert st == 204, st
    return op


def check(cond, msg):
    global ok
    print(("OK   " if cond else "FAIL ") + msg)
    ok &= bool(cond)


anon = client()
check(call(anon, "GET", "/v1/me")[0] == 401, "anonym får 401 på /v1/me")
check(call(anon, "POST", "/auth/dev/login", {"account_id": "00000000-0000-0000-0000-000000000000"})[0] == 403,
      "POST utan CSRF-header nekas")

reg = login("Medlemsregistrerare")
st, me, hdrs = call(reg, "GET", "/v1/me")
check(st == 200 and me["contexts"], f"registrerare /v1/me, kontexter: {[c['body_name'] for c in me['contexts']]}")
check(hdrs.get("x-content-type-options") == "nosniff" and hdrs.get("x-frame-options") == "DENY", "säkerhetsheaders finns")
group_id = next(c["body_id"] for c in me["contexts"] if c["body_kind"] == "group")
st, page, _ = call(reg, "GET", f"/v1/bodies/{group_id}/members?limit=500")
check(st == 200 and page["total"] > 50, f"registrerare ser kårens {page['total']} aktiva medlemmar ({page['took_ms']:.1f} ms)")
check(all(r["email"] is not None for r in page["items"] if r["is_leader"]), "registrerare ser ledares e-post")
check(any((r["national_id_masked"] or "").endswith("-****") for r in page["items"]), "personnummer maskat i listan")
troops = {r["troop"]["name"] for r in page["items"] if r["troop"]}
check(len(troops) >= 4, f"flera avdelningar: {sorted(troops)}")

st, body, _ = call(reg, "GET", f"/v1/bodies/{group_id}")
check(st == 200 and body["counts"]["waiting"] > 0,
      f"kår {body['name']}: {body['counts']}, brödsmulor {[a['name'] for a in body['ancestors']]}")
st, waiting, _ = call(reg, "GET", f"/v1/bodies/{group_id}/members?status=waiting&sort=waiting_since")
check(st == 200 and waiting["total"] == body["counts"]["waiting"], f"väntelista {waiting['total']} st sorterad på väntetid")
st, filt, _ = call(reg, "GET", f"/v1/bodies/{group_id}/members?troop_type=sparare,upptackare&leaders=false&sort=-birth_date")
check(st == 200 and all(r["troop"]["troop_type"] in ("sparare", "upptackare") for r in filt["items"]),
      f"filter spårare+upptäckare: {filt['total']} st")
dates = [r["birth_date"] for r in filt["items"]]
check(dates == sorted(dates, reverse=True), "sortering -birth_date fungerar")
st, missing, _ = call(reg, "GET", f"/v1/bodies/{group_id}/members?missing_guardian_email=true")
check(st == 200 and all(r["guardians_missing_email"] for r in missing["items"]),
      f"saknar e-post hos vårdnadshavare: {missing['total']} st")
check(call(reg, "GET", f"/v1/bodies/{group_id}/members?sort=evil;drop")[0] == 400, "okänd sorteringsnyckel ger 400")

person = page["items"][0]["id"]
st, detail, _ = call(reg, "GET", f"/v1/people/{person}")
check(st == 200 and "national_id_read" in detail["permissions"],
      f"persondetalj: {len(detail['guardians'])} vårdnadshavare, närvaro {detail['attendance']}")
st, pnr, _ = call(reg, "POST", f"/v1/people/{person}/reveal-national-id", None, {"x-sn-csrf": "1"})
check(st == 200 and pnr["national_id"][9:12] >= "980", "registrerare kan visa testpersonnummer (loggas)")

# Alla personfält (docs/04-faltkatalog.md)
leaders = [r for r in page["items"] if r["is_leader"]]
check(all(r["police_check"] in ("missing", "expired", "expiring", "valid") for r in leaders),
      f"registerutdrag krävs för ledare: {sorted({r['police_check'] for r in leaders})}")
check(any(r["fee_status"] for r in page["items"]) and any(r["mobile_phone"] for r in page["items"]),
      "listan har avgiftsstatus och mobilnummer")
st, pc, _ = call(reg, "GET", f"/v1/bodies/{group_id}/members?police_check=missing,expired&sort=police_check")
check(st == 200 and pc["total"] > 0 and all(r["police_check"] in ("missing", "expired") for r in pc["items"]),
      f"filter registerutdrag saknas/utgånget: {pc['total']} st")
st, fee, _ = call(reg, "GET", f"/v1/bodies/{group_id}/members?fee_status=unpaid,overdue&sort=fee")
check(st == 200 and all(r["fee_status"] in ("unpaid", "overdue") for r in fee["items"]),
      f"filter obetald avgift: {fee['total']} st")
check(call(reg, "GET", f"/v1/bodies/{group_id}/members?fee_status=gratis")[0] == 400, "ogiltigt avgiftsfilter ger 400")
groups = ("contact", "addresses", "preferences", "fees", "police_check", "follow_ups", "membership", "consents", "merits")
lid = leaders[0]["id"]
st, full, _ = call(reg, "GET", f"/v1/people/{lid}")
check(st == 200 and all(full.get(k) is not None for k in groups) and full["police_check"]["required"],
      f"registrerare ser alla fältgrupper för ledare ({len(full['fees'])} avgifter, {len(full['merits'])} meriter)")
fam = next((r for r in page["items"] if not r["is_leader"] and r["guardian_count"] > 0
            and (call(reg, "GET", f"/v1/people/{r['id']}")[1] or {}).get("household")), None)
if fam:
    _, famd, _ = call(reg, "GET", f"/v1/people/{fam['id']}")
    check(famd["household"]["members"] and famd["guardians"][0]["position"] == 1,
          f"hushåll {famd['household']['name']}: {len(famd['household']['members'])} till")
else:
    check(False, "hittade inget hushåll i kåren")

# Läsbara nycklar (group-1001) och organisationsöversikten
grp = next(c for c in me["contexts"] if c["body_kind"] == "group")
key = f"group-{grp['body_number']}"
st, by_key, _ = call(reg, "GET", f"/v1/bodies/{key}")
check(st == 200 and by_key["id"] == group_id and by_key["number"] == grp["body_number"], f"uppslag med nyckel {key}")
check([call(reg, "GET", f"/v1/bodies/{bad}")[0] for bad in ("group-01001", "kar-1001", "group-x")] == [404] * 3,
      "ogiltiga nycklar ger 404")
check(all(a["number"] > 0 for a in by_key["ancestors"]) and not any(a["accessible"] for a in by_key["ancestors"]),
      "brödsmulor har nummer, och registraren får inte öppna distrikt/förbund")
st, tree, _ = call(reg, "GET", f"/v1/bodies/{key}/descendants?depth=2")
troops_in_tree = [n for n in tree["items"] if n["kind"] == "troop"]
check(st == 200 and troops_in_tree and all(n["depth"] in (1, 2) for n in tree["items"])
      and sum(n["counts"]["scouts"] + n["counts"]["leaders"] for n in troops_in_tree) == body["counts"]["active"],
      f"organisationsträd: {len(tree['items'])} enheter, antal stämmer med kåren ({tree['took_ms']:.1f} ms)")
st, lst, _ = call(reg, "GET", f"/v1/bodies/{key}/descendants?kind=troop&sort=-scouts")
scouts = [n["counts"]["scouts"] for n in lst["items"]]
check(st == 200 and scouts == sorted(scouts, reverse=True) and all(n["path"] == [] for n in lst["items"]),
      "listvy: avdelningar sorterade på antal scouter")
check(call(reg, "GET", f"/v1/bodies/{key}/descendants?sort=evil")[0] == 400, "okänd sortering i trädet ger 400")

leader = login("Avdelningsledare")
st, lme, _ = call(leader, "GET", "/v1/me")
troop_ctx = next(c for c in lme["contexts"] if c["body_kind"] == "troop")
check(call(leader, "GET", f"/v1/bodies/{group_id}/members")[0] == 404, "avdelningsledare kan inte lista hela kåren (404)")
st, tpage, _ = call(leader, "GET", f"/v1/bodies/{troop_ctx['body_id']}/members")
check(st == 200 and all(r["troop"]["id"] == troop_ctx["body_id"] for r in tpage["items"]),
      f"avdelningsledare ser {tpage['total']} i {troop_ctx['body_name']}")
other = next(r["id"] for r in page["items"] if r["troop"] and r["troop"]["id"] != troop_ctx["body_id"])
check(call(leader, "GET", f"/v1/people/{other}")[0] == 404, "avdelningsledare kan inte se scout i annan avdelning")
check(call(leader, "POST", f"/v1/people/{tpage['items'][0]['id']}/reveal-national-id", None, {"x-sn-csrf": "1"})[0] == 403,
      "avdelningsledare får inte visa personnummer")
check(all(r["fee_status"] is None for r in tpage["items"]) and "fee_status" in tpage["redacted_fields"],
      "avdelningsledare ser inte avgifter (maskas)")
check(call(leader, "GET", f"/v1/bodies/{key}/descendants")[0] == 404, "avdelningsledare ser inte kårens organisationsträd")
st, tb, _ = call(leader, "GET", f"/v1/bodies/troop-{troop_ctx['body_number']}")
check(st == 200 and [a["accessible"] for a in tb["ancestors"]] == [False] * len(tb["ancestors"]),
      f"avdelning via nyckel troop-{troop_ctx['body_number']}, överliggande enheter ej klickbara")
st, leak, _ = call(leader, "GET", f"/v1/bodies/{troop_ctx['body_id']}/members?fee_status=paid,unpaid,overdue")
check(st == 200 and leak["total"] == 0, "avgiftsfilter läcker inget utan ekonomibehörighet")
st, ld, _ = call(leader, "GET", f"/v1/people/{tpage['items'][0]['id']}")
check(st == 200 and ld["fees"] is None and ld["note"] is None and ld["police_check"] is not None,
      "avdelningsledare: registerutdrag ja, avgifter och anteckning nej")

scout = login("Scout")
st, sme, _ = call(scout, "GET", "/v1/me")
check(st == 200 and not sme["contexts"], "scout utan roll har inga kontexter")
st, own, _ = call(scout, "GET", f"/v1/people/{sme['person_id']}")
check(st == 200 and own["fees"] is not None and own["contact"] is not None and own["note"] is None,
      "scout ser sin egen profil inkl. egna avgifter, men inte interna anteckningar")
check(call(scout, "GET", f"/v1/bodies/{group_id}/members")[0] == 404, "scout kan inte lista medlemmar")
check(call(scout, "POST", "/auth/logout", None, {"x-sn-csrf": "1"})[0] == 204 and call(scout, "GET", "/v1/me")[0] == 401,
      "utloggning tar bort sessionen")

st, spec, _ = call(anon, "GET", "/openapi.json")
check(st == 200 and len(spec["paths"]) >= 8, f"OpenAPI: {len(spec['paths'])} sökvägar")
check(call(anon, "GET", "/docs")[0] == 200, "API-dokumentation (Scalar) på /docs")

print("\nALLA OK" if ok else "\nNÅGOT FALLERADE")
sys.exit(0 if ok else 1)
