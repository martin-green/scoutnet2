"""Rökprov mot lokal server: inloggning med personas, behörighet och medlemslistan.

Kör: python scripts/smoke.py   (servern ska vara igång på http://127.0.0.1:8080 med seedad data)
"""
import base64
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
            is_json = "json" in r.headers.get("content-type", "")
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
st, kids, _ = call(reg, "GET", f"/v1/bodies/{key}/children")
check(st == 200 and sum(c["active"] for c in kids) == body["counts"]["active"]
      and sum(c["leaders"] for c in kids) == body["counts"]["leaders"],
      f"underenheter: {len(kids)} st, summan stämmer med kåren")
check(call(reg, "GET", f"/v1/bodies/{key}/descendants?sort=evil")[0] == 400, "okänd sortering i trädet ger 400")

# Sparade vyer (på servern, delbara i enheten)
kar = login("Kårordförande")
st, mine, _ = call(reg, "POST", f"/v1/bodies/{key}/views",
                   {"list": "members", "name": "Mina spårare", "query": "offset=100&troop_type=sparare&sort=name"},
                   {"x-sn-csrf": "1"})
check(st == 201 and mine["query"] == "sort=name&troop_type=sparare" and mine["mine"] and not mine["shared"],
      "spara egen vy (frågan normaliseras, paginering tas bort)")
st, shared, _ = call(reg, "POST", f"/v1/bodies/{key}/views",
                     {"list": "members", "name": "Ledare utan registerutdrag", "query": "leaders=true&police_check=missing,expired",
                      "shared": True}, {"x-sn-csrf": "1"})
st, kviews, _ = call(kar, "GET", f"/v1/bodies/{key}/views?list=members")
check(st == 200 and [v["name"] for v in kviews] == ["Ledare utan registerutdrag"] and not kviews[0]["mine"],
      "kårordförande ser den delade vyn men inte registrarens egna")
check(call(kar, "DELETE", f"/v1/views/{mine['id']}", None, {"x-sn-csrf": "1"})[0] == 404,
      "andras egna vyer går inte att ta bort (404)")
check(call(leader_probe := login("Avdelningsledare"), "GET", f"/v1/bodies/{key}/views?list=members")[0] == 404,
      "vyer i en enhet man inte ser är dolda")
check(all(call(reg, "DELETE", f"/v1/views/{v['id']}", None, {"x-sn-csrf": "1"})[0] == 204 for v in (mine, shared))
      and call(reg, "GET", f"/v1/bodies/{key}/views?list=members")[1] == [], "ta bort egna vyer")

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

# Närvaro (vecka 3)
tkey = f"troop-{troop_ctx['body_number']}"
st, mx, _ = call(leader, "GET", f"/v1/bodies/{tkey}/attendance")
past = [m for m in mx["meetings"] if m["is_past"]]
check(st == 200 and mx["can_write"] and past and len(mx["people"]) >= tpage["total"]
      and all(len(p["statuses"]) == len(mx["meetings"]) for p in mx["people"]),
      f"närvarolista {mx['term']}: {len(mx['meetings'])} möten × {len(mx['people'])} personer")
st, created, _ = call(leader, "POST", f"/v1/bodies/{tkey}/meetings",
                      {"name": "Rökprov", "date": "2027-02-02", "time": "18:00", "duration_min": 90,
                       "repeat_until": "2027-02-23", "skip_dates": ["2027-02-16"]}, {"x-sn-csrf": "1"})
check(st == 201 and [m["date"] for m in created] == ["2027-02-02", "2027-02-09", "2027-02-23"]
      and all(m["time"] == "18:00" and m["starts_at"].startswith(m["date"] + "T17:00") for m in created),
      "planera veckoserie med överhoppat lov, svensk tid → UTC")
mid = created[0]["id"]
st, md, _ = call(leader, "GET", f"/v1/meetings/{mid}")
entries = [{"person_id": r["id"], "status": "present"} for r in md["roster"]]
entries[0]["status"] = "excused"
st, saved, _ = call(leader, "PUT", f"/v1/meetings/{mid}/attendance", {"entries": entries}, {"x-sn-csrf": "1"})
check(st == 200 and saved["meeting"]["present"] == len(entries) - 1 and saved["meeting"]["excused"] == 1,
      f"registrera närvaro: {saved['meeting']['present']} närvarande, 1 förhinder")
st, again, _ = call(leader, "PUT", f"/v1/meetings/{mid}/attendance", {"entries": entries[:2]}, {"x-sn-csrf": "1"})
check(st == 200 and again["meeting"]["present"] + again["meeting"]["excused"] == 2, "helhetsuppdatering ersätter närvaron")
check(call(leader, "PUT", f"/v1/meetings/{mid}/attendance",
           {"entries": [{"person_id": "00000000-0000-0000-0000-000000000000", "status": "present"}]}, {"x-sn-csrf": "1"})[0] == 400,
      "person utanför avdelningen ger 400")
check(all(call(leader, "DELETE", f"/v1/meetings/{m['id']}", None, {"x-sn-csrf": "1"})[0] == 204 for m in created)
      and call(leader, "GET", f"/v1/meetings/{mid}")[0] == 404, "ta bort planerade möten")
st, summ, _ = call(reg, "GET", f"/v1/bodies/{key}/attendance/summary")
check(st == 200 and len(summ["troops"]) >= 4 and any(t["held"] > 0 for t in summ["troops"]),
      f"närvaroöversikt för kåren: {len(summ['troops'])} avdelningar")
check(call(reg, "PUT", f"/v1/meetings/{past[-1]['id']}/attendance", {"entries": []}, {"x-sn-csrf": "1"})[0] == 403,
      "medlemsregistrerare kan läsa men inte registrera närvaro (403)")
other_troop = next(t for t in summ["troops"] if t["troop"]["number"] != troop_ctx["body_number"])
check(call(leader, "GET", f"/v1/bodies/troop-{other_troop['troop']['number']}/attendance")[0] == 404
      and call(leader, "GET", f"/v1/meetings/{other_troop['last_meeting_id']}")[0] == 404,
      "avdelningsledare ser inte andra avdelningars närvaro")

# Bidrag och DAK (vecka 4)
st, gr, _ = call(reg, "GET", f"/v1/bodies/{key}/grants")
check(st == 200 and gr["rules"] and gr["rules"]["municipality_code"] == "1480" and not gr["problems"]
      and gr["held"] == len(gr["meetings"]) and 0 < gr["approved"] <= gr["held"]
      and gr["amount_ore"] == sum(t["amount_ore"] for t in gr["troops"]) == sum(m["amount_ore"] for m in gr["meetings"]),
      f"bidragsunderlag Göteborg: {gr['approved']}/{gr['held']} godkända, {gr['amount_ore'] / 100:.2f} kr")
check(all(m["issues"] for m in gr["meetings"] if not m["approved"]), "underkända sammankomster har förklaring")
st, xml, hdrs = call(reg, "GET", f"/v1/bodies/{tkey}/grants/dak")
text = xml.decode() if isinstance(xml, bytes) else ""
import re
pnrs = re.findall(r"<Personnummer>(\d*)</Personnummer>", text)
check(st == 200 and "attachment" in hdrs.get("content-disposition", "") and 'kommunID="1480"' in text
      and 'version="2.2"' in text and pnrs and all(len(p) == 12 and p[8:10] in ("98", "99") for p in pnrs),
      f"DAK 2.2-fil med {len(pnrs)} personer (testpersonnummer, 12 siffror)")
st, lgr, _ = call(leader, "GET", f"/v1/bodies/{tkey}/grants")
check(st == 200 and not lgr["can_export_dak"] and call(leader, "GET", f"/v1/bodies/{tkey}/grants/dak")[0] == 403,
      "avdelningsledare ser underlaget men får inte ladda ner personnummer (403)")
check(call(login("Distriktsordförande"), "GET", "/v1/bodies/district-1/grants")[0] == 400, "bidrag tas fram per kår eller avdelning")

# Kalenderprenumeration (iCal)
st, sub, _ = call(leader, "POST", f"/v1/bodies/{tkey}/calendar", None, {"x-sn-csrf": "1"})
feed_path = sub["url"].removeprefix(BASE) if st == 201 else ""
st, ics, hdrs = call(anon, "GET", feed_path)
ics_text = ics.decode() if isinstance(ics, bytes) else ""
check(st == 200 and hdrs.get("content-type", "").startswith("text/calendar") and ics_text.count("BEGIN:VEVENT") >= 5
      and "@example.org" not in ics_text and "070-" not in ics_text,
      f"iCal-flöde utan inloggning: {ics_text.count('BEGIN:VEVENT')} möten, inga personuppgifter")
st, sub2, _ = call(leader, "POST", f"/v1/bodies/{tkey}/calendar", None, {"x-sn-csrf": "1"})
check(call(anon, "GET", feed_path)[0] == 404 and call(anon, "GET", sub2["url"].removeprefix(BASE))[0] == 200,
      "ny kalenderadress ersätter den gamla")
check(call(leader, "DELETE", f"/v1/bodies/{tkey}/calendar", None, {"x-sn-csrf": "1"})[0] == 204
      and call(anon, "GET", sub2["url"].removeprefix(BASE))[0] == 404 and call(anon, "GET", "/ical/abc.ics")[0] == 404,
      "återkallad eller ogiltig kalenderadress ger 404")

# Massåtgärder (vecka 5). Torrkörning ändrar inget; flytten görs och återställs.
csrf = {"x-sn-csrf": "1"}
st, tl, _ = call(reg, "GET", f"/v1/bodies/{key}/descendants?kind=troop")
troop_ids = [n["id"] for n in tl["items"] if n.get("troop_type") not in (None, "annan")]
st, wl, _ = call(reg, "GET", f"/v1/bodies/{group_id}/members?status=waiting&sort=waiting_since&limit=2")
wait_ids = [r["id"] for r in wl["items"]]
st, dry, _ = call(reg, "POST", f"/v1/bodies/{key}/bulk",
                  {"action": "admit", "person_ids": wait_ids, "troop_id": troop_ids[0], "dry_run": True}, csrf)
_, body_after, _ = call(reg, "GET", f"/v1/bodies/{group_id}")
check(st == 200 and dry["dry_run"] and dry["changed"] == len(wait_ids) and all(r["from"] == "Väntelista" for r in dry["rows"])
      and body_after["counts"]["waiting"] == body["counts"]["waiting"],
      f"torrkörning: {dry['changed']} skulle tas in från väntelistan, inget ändras")
mover = next(r for r in page["items"] if not r["is_leader"] and r["troop"] and r["troop"]["id"] != troop_ids[-1])
orig_troop = mover["troop"]["id"]
orig_patrol = (mover.get("patrol") or {}).get("id")
st, mv, _ = call(reg, "POST", f"/v1/bodies/{key}/bulk",
                 {"action": "move", "person_ids": [mover["id"]], "troop_id": troop_ids[-1]}, csrf)
_, moved, _ = call(reg, "GET", f"/v1/people/{mover['id']}")
check(st == 200 and mv["changed"] == 1 and (moved.get("troop") or {}).get("id") == troop_ids[-1],
      f"flytta {mv['rows'][0]['name']}: {mv['rows'][0]['from']} → {mv['rows'][0]['to']}")
st, back, _ = call(reg, "POST", f"/v1/bodies/{key}/bulk",
                   {"action": "move", "person_ids": [mover["id"]], "troop_id": orig_troop, "patrol_id": orig_patrol}, csrf)
check(st == 200 and back["changed"] == 1, "flytten återställd")
st, skip, _ = call(reg, "POST", f"/v1/bodies/{key}/bulk",
                   {"action": "admit", "person_ids": [mover["id"]], "troop_id": troop_ids[0], "dry_run": True}, csrf)
check(st == 200 and skip["changed"] == 0 and skip["rows"][0]["outcome"] == "skip", "aktiv medlem kan inte tas in igen (skip)")
check(call(leader, "POST", f"/v1/bodies/{key}/bulk",
           {"action": "move", "person_ids": [mover["id"]], "troop_id": orig_troop, "dry_run": True}, csrf)[0] == 404,
      "avdelningsledare kan inte göra massåtgärder i kåren (404)")

# Vem har sett mina uppgifter (revisionslogg per person)
st, alog, _ = call(kar, "GET", f"/v1/people/{person}/audit")
check(st == 200 and any(e["action"] == "person.national_id.reveal" for e in alog["entries"]) and alog["views_by_others"] > 0,
      f"kårordförande ser loggen för en medlem: {len(alog['entries'])} händelser")
_, mlog, _ = call(kar, "GET", f"/v1/people/{mover['id']}/audit")
check(sum(e["action"] == "bulk.move" for e in mlog["entries"]) >= 2, "flyttarna syns i personens logg")
check(call(reg, "GET", f"/v1/people/{person}/audit")[0] == 404, "registrerare saknar loggbehörighet (404)")

# Scoutnet-kompatibelt API med API-nycklar (planen 6.4)
def basic(user, pw):
    return {"authorization": "Basic " + base64.b64encode(f"{user}:{pw}".encode()).decode()}


gno = grp["body_number"]
st, k1, _ = call(reg, "POST", f"/v1/bodies/{key}/api-keys",
                 {"name": "Skojjt", "scopes": ["memberlist:contact", "memberlist:ssno"]}, csrf)
check(st == 201 and k1["key"].startswith("sn2_") and k1["username"] == str(gno)
      and k1["info"]["scopes"] == ["memberlist", "memberlist:contact", "memberlist:ssno"],
      "skapa API-nyckel (visas en gång, memberlist läggs till)")
st, ml, _ = call(anon, "GET", "/api/group/memberlist", None, basic(gno, k1["key"]))
rows_ml = list((ml or {}).get("data", {}).values())
check(st == 200 and len(rows_ml) == body["counts"]["active"] and "labels" in ml
      and all(set(r["member_no"]) == {"value", "raw_value"} for r in rows_ml)
      and any("ssno" in r for r in rows_ml) and any("email" in r for r in rows_ml),
      f"Scoutnet-format via Basic: {len(rows_ml)} medlemmar med {{value, raw_value}}")
st, mlw, _ = call(anon, "GET", "/api/group/memberlist?waiting=1", None, basic(gno, k1["key"]))
check(st == 200 and len(mlw["data"]) == body["counts"]["waiting"], f"väntelistan: {len(mlw['data'])} st")
st, _, hdrs = call(anon, "GET", f"/api/group/memberlist?id={gno}&key={k1['key']}")
check(st == 200 and hdrs.get("deprecation"), "nyckel i frågan fungerar men markeras som avrådd (Deprecation)")
st, _, hdrs = call(anon, "GET", "/api/group/memberlist", None, basic(gno, "sn2_fel"))
check(st == 401 and hdrs.get("www-authenticate", "").startswith("Basic"), "fel nyckel ger 401")
check(call(anon, "GET", "/api/group/memberlist", None, basic(gno + 1, k1["key"]))[0] == 401, "nyckeln gäller bara sin kår")
check(call(kar, "POST", f"/v1/bodies/{key}/api-keys", {"name": "X", "scopes": ["memberlist:ssno"]}, csrf)[0] == 403,
      "kårordförande kan inte ge en integration personnummer (403)")
st, k2, _ = call(kar, "POST", f"/v1/bodies/{key}/api-keys", {"name": "Bara namn", "scopes": []}, csrf)
_, ml2, _ = call(anon, "GET", "/api/group/memberlist", None, basic(gno, k2["key"]))
check(st == 201 and not any("ssno" in r or "email" in r for r in ml2["data"].values()),
      "nyckel utan extra behörigheter: inga personnummer eller kontaktuppgifter")
st, keys, _ = call(reg, "GET", f"/v1/bodies/{key}/api-keys")
check(st == 200 and {k1["info"]["id"], k2["info"]["id"]} <= {k["id"] for k in keys}
      and all(k["last_used_at"] for k in keys if k["id"] == k1["info"]["id"]),
      "nyckellistan visar senaste användning")
check(call(leader, "GET", f"/v1/bodies/{key}/api-keys")[0] == 404, "avdelningsledare ser inte kårens nycklar")
check(all(call(reg, "DELETE", f"/v1/api-keys/{k['info']['id']}", None, csrf)[0] == 204 for k in (k1, k2))
      and call(anon, "GET", "/api/group/memberlist", None, basic(gno, k1["key"]))[0] == 401,
      "återkallad nyckel slutar fungera direkt")
_, plog, _ = call(kar, "GET", f"/v1/people/{person}/audit")
check(any(e["action"] == "compat.memberlist" and "«Skojjt»" in e["description"] and "personnummer" in e["description"]
          for e in plog["entries"]), "integrationens hämtningar syns i personens logg med namn")

scout = login("Scout")
st, sme, _ = call(scout, "GET", "/v1/me")
check(st == 200 and not sme["contexts"], "scout utan roll har inga kontexter")
st, own, _ = call(scout, "GET", f"/v1/people/{sme['person_id']}")
check(st == 200 and own["fees"] is not None and own["contact"] is not None and own["note"] is None,
      "scout ser sin egen profil inkl. egna avgifter, men inte interna anteckningar")
check(call(scout, "GET", f"/v1/bodies/{group_id}/members")[0] == 404, "scout kan inte lista medlemmar")
st, slog, _ = call(scout, "GET", f"/v1/people/{sme['person_id']}/audit")
check(st == 200 and isinstance(slog["entries"], list) and call(scout, "GET", f"/v1/people/{person}/audit")[0] == 404,
      f"scout ser vem som har sett hens uppgifter ({slog['views_by_others']} visningar), men inte andras logg")
st, out, _ = call(scout, "POST", "/auth/logout", None, {"x-sn-csrf": "1"})
check(st == 200 and out["logout_url"] is None and call(scout, "GET", "/v1/me")[0] == 401,
      "utloggning tar bort sessionen (personas: ingen ScoutID-utloggning)")

# Inloggning via ScoutID (OIDC). Hela flödet med Keycloak prövas i webbläsaren; här kontrolleras
# att servern startar flödet korrekt och avvisar felaktiga återkomster.
class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


def location(op, path):
    try:
        op.open(BASE + path)
    except urllib.error.HTTPError as e:
        return e.code, e.headers.get("location") or "", e.headers.get_all("set-cookie") or []
    return 200, "", []


nr = urllib.request.build_opener(NoRedirect, urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
st, cfg, _ = call(anon, "GET", "/auth/config")
if st == 200 and cfg["scoutid"]:
    code, loc, cookies = location(nr, "/auth/login?return_to=/kar/1001")
    check(code == 303 and "code_challenge_method=S256" in loc and "nonce=" in loc and "state=" in loc
          and any(c.startswith("sn_oidc=") and "HttpOnly" in c for c in cookies),
          "ScoutID-inloggning startar med PKCE, state och nonce (state-cookie HttpOnly)")
    code, loc, _ = location(nr, "/auth/callback?code=fejk&state=fel")
    check(code == 303 and loc == "/login?fel=ogiltig", "återkomst med fel state avvisas")
    code, loc, _ = location(nr, "/auth/callback?error=access_denied&state=x")
    check(code == 303 and loc == "/login?fel=avbruten", "avbruten inloggning i ScoutID hanteras")
else:
    print("HOPPAR ÖVER ScoutID-kontroller (Keycloak körs inte: docker compose up -d keycloak)")

st, spec, _ = call(anon, "GET", "/openapi.json")
check(st == 200 and len(spec["paths"]) >= 8, f"OpenAPI: {len(spec['paths'])} sökvägar")
check(call(anon, "GET", "/docs")[0] == 200, "API-dokumentation (Scalar) på /docs")

print("\nALLA OK" if ok else "\nNÅGOT FALLERADE")
sys.exit(0 if ok else 1)
