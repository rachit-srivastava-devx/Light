#!/usr/bin/env python3
"""Compute one day's work metrics into JSONL + a markdown report.

Inputs (all optional; missing ones are reported as not measured, never as zero):
  daily-briefs/events/raw/<day>-calendar.json
  daily-briefs/events/raw/<day>-slack.json
  ~/.claude/projects/*/*.jsonl                  Claude Code transcripts
  git repos listed in REPOS

Outputs:
  daily-briefs/events/metrics/<day>.jsonl       canonical machine record
  daily-briefs/metrics/<day>.md                 human report

Every metric carries evidence references. See daily-var/runs/SCHEMA/brief.md.
Usage: compute-day.py [YYYY-MM-DD]
"""
import json, os, sys, glob, re, subprocess, statistics, datetime
from zoneinfo import ZoneInfo

IST = ZoneInfo("Asia/Kolkata")
HOME = os.path.expanduser("~")
BASE = os.path.join(HOME, "youtube/Principal Engineering")
BRIEFS = os.path.join(BASE, "daily-briefs")
RAW = os.path.join(BRIEFS, "events/raw")
REPOS = [
    ("divvy", os.path.join(HOME, "Developer/divvy")),
    ("divvy-harness", os.path.join(HOME, "Developer/divvy-harness")),
    ("blueprints", os.path.join(BASE, "blueprints")),
    ("firstmate", os.path.join(BASE, "references/firstmate")),
    ("sparse-mamba-moe-llm", os.path.join(BASE, "sparse-mamba-moe-llm")),
]
FOCUS_GAP_MIN = 20      # a gap longer than this ends a focus block
BURST_GAP_MIN = 30      # a gap longer than this starts a new conversation burst
DEEP_BLOCK_MIN = 45     # a block at least this long counts as deep work
INSTANT_NOTICE_MIN = 5  # created within this of its start = instant meeting
SHORT_NOTICE_MIN = 60

day = sys.argv[1] if len(sys.argv) > 1 else datetime.datetime.now(IST).strftime("%Y-%m-%d")
D0 = datetime.datetime.strptime(day, "%Y-%m-%d").replace(tzinfo=IST)
D1 = D0 + datetime.timedelta(days=1)
records, metrics, notes = [], [], []


def ev(**kw):
    kw["type"] = "event"
    records.append(kw)
    return kw


def metric(name, value, unit="count", evidence=None, confirmed=True, detail=None):
    m = {"type": "metric", "day": day, "name": name, "value": value, "unit": unit,
         "evidence": evidence or []}
    if not confirmed:
        m["confirmed"] = False
    if detail is not None:
        m["detail"] = detail
    metrics.append(m)
    return m


def parse_iso(s):
    try:
        return datetime.datetime.fromisoformat(str(s).replace("Z", "+00:00")).astimezone(IST)
    except Exception:
        return None


def parse_local(s):
    try:
        return datetime.datetime.strptime(str(s), "%Y-%m-%d %H:%M:%S").replace(tzinfo=IST)
    except Exception:
        return None


def mins(a, b):
    return (b - a).total_seconds() / 60.0


def overlap_min(a1, a2, b1, b2):
    lo, hi = max(a1, b1), min(a2, b2)
    return max(0.0, (hi - lo).total_seconds() / 60.0)


def load(path):
    try:
        with open(path, encoding="utf-8") as fh:
            return json.load(fh)
    except Exception:
        return None


# ---------------------------------------------------------------- transcripts
prompts, sessions, tool_errors, interrupts, restarts, switches = [], {}, [], [], [], []
META_PREFIXES = ("[Request interrupted", "<command-name>", "<local-command-stdout>",
                 "<local-command-caveat>", "<command-message>", "Caveat:",
                 "This session is being continued", "[Your previous response had no visible",
                 "A session-scoped Stop hook is now active")


def text_of(c):
    if isinstance(c, str):
        return c
    if isinstance(c, list):
        return "\n".join(str(i.get("text", "")) for i in c if isinstance(i, dict) and i.get("type") == "text")
    return ""


for path in glob.glob(os.path.join(HOME, ".claude/projects/*/*.jsonl")):
    if datetime.datetime.fromtimestamp(os.path.getmtime(path), IST) < D0:
        continue
    sid = os.path.basename(path)[:8]
    proj = os.path.basename(os.path.dirname(path)).replace("-Users-rachitsrivastava-", "").strip("-")
    s_first = s_last = None
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            for raw in fh:
                raw = raw.strip()
                if not raw:
                    continue
                try:
                    rec = json.loads(raw)
                except Exception:
                    continue
                t = parse_iso(rec.get("timestamp"))
                if not t or not (D0 <= t < D1):
                    continue
                s_first = min(s_first or t, t)
                s_last = max(s_last or t, t)
                content = (rec.get("message") or {}).get("content")
                if rec.get("type") != "user":
                    continue
                if isinstance(content, list) and any(
                        isinstance(i, dict) and i.get("type") == "tool_result" for i in content):
                    for i in content:
                        if isinstance(i, dict) and i.get("type") == "tool_result" and i.get("is_error"):
                            txt = i.get("content")
                            if isinstance(txt, list):
                                txt = " ".join(str(x.get("text", "")) for x in txt if isinstance(x, dict))
                            txt = " ".join(str(txt or "").split())
                            tool_errors.append({"t": t, "sid": sid, "text": txt})
                    continue
                body = text_of(content).strip()
                if body.startswith("[Request interrupted"):
                    interrupts.append({"t": t, "sid": sid})
                elif body.startswith("This session is being continued"):
                    restarts.append({"t": t, "sid": sid})
                elif body.startswith("<local-command-stdout>") and "Set model" in body:
                    switches.append({"t": t, "sid": sid})
                elif not body.startswith(META_PREFIXES) and body:
                    prompts.append({"t": t, "sid": sid, "proj": proj,
                                    "first_line": body.split("\n")[0][:120], "words": len(body.split())})
    except Exception:
        continue
    if s_first:
        sessions[sid] = {"first": s_first, "last": s_last, "proj": proj}

prompts.sort(key=lambda p: p["t"])
have_transcripts = bool(prompts)
if not have_transcripts:
    notes.append("No Claude Code prompts found for this day — focus, rework and multitasking metrics are **not measured**.")

# ------------------------------------------------------------- focus blocks
blocks = []
for p in prompts:
    if blocks and mins(blocks[-1]["last"], p["t"]) <= FOCUS_GAP_MIN:
        blocks[-1]["last"] = p["t"]
        blocks[-1]["n"] += 1
    else:
        blocks.append({"first": p["t"], "last": p["t"], "n": 1})
for b in blocks:
    b["min"] = max(1.0, mins(b["first"], b["last"]))
    ev(kind="focus_block", ts=b["first"].isoformat(), source="claude-code",
       duration_min=round(b["min"], 1), prompts=b["n"],
       evidence=[f"transcript:#{b['first']:%H:%M:%S}"])

gaps = []
for a, b in zip(prompts, prompts[1:]):
    g = mins(a["t"], b["t"])
    if g > FOCUS_GAP_MIN:
        gaps.append({"from": a["t"], "to": b["t"], "min": g})

switch_count = sum(1 for a, b in zip(prompts, prompts[1:]) if a["proj"] != b["proj"])

# ------------------------------------------------------------------ calendar
cal = load(os.path.join(RAW, f"{day}-calendar.json"))
meetings, instant, short_notice, organized, inbound, rsvp_missing = [], [], [], [], [], []
if cal is None:
    notes.append("No calendar pull for this day — meeting and interruption metrics are **not measured**.")
else:
    for e in cal.get("events", []):
        st, en, cr = parse_iso(e.get("start")), parse_iso(e.get("end")), parse_iso(e.get("created"))
        if not st or not (D0 <= st < D1):
            continue
        notice = mins(cr, st) if cr else None
        me = cal.get("me", "")
        is_mine = (e.get("organizer") or "") == me
        m = {"id": e["id"], "summary": e.get("summary", ""), "start": st, "end": en,
             "notice": notice, "org": e.get("organizer", ""), "mine": is_mine,
             "rsvp": e.get("my_rsvp"), "external": bool(e.get("external"))}
        meetings.append(m)
        (organized if is_mine else inbound).append(m)
        if notice is not None and notice <= INSTANT_NOTICE_MIN:
            instant.append(m)
        elif notice is not None and notice <= SHORT_NOTICE_MIN:
            short_notice.append(m)
        if m["rsvp"] in ("needsAction", None):
            rsvp_missing.append(m)
        ev(kind="meeting", subkind=("instant" if notice is not None and notice <= INSTANT_NOTICE_MIN
                                   else "short_notice" if notice is not None and notice <= SHORT_NOTICE_MIN
                                   else "planned"),
           ts=st.isoformat(), source="calendar",
           direction="outbound" if is_mine else "inbound", actor=m["org"],
           duration_min=round(mins(st, en), 1) if en else None,
           notice_min=round(notice, 1) if notice is not None else None,
           title=m["summary"], rsvp=m["rsvp"], external=m["external"],
           evidence=[f"gcal:{m['id']}"])

# meetings colliding with focus
collisions, lost, recovery, multitask = [], 0.0, [], 0
for m in meetings:
    if not m["end"]:
        continue
    ov = sum(overlap_min(b["first"], b["last"], m["start"], m["end"]) for b in blocks)
    inside = [p for p in prompts if m["start"] <= p["t"] <= m["end"]]
    multitask += len(inside)
    enclosing = [g for g in gaps if g["from"] <= m["start"] and g["to"] >= m["end"]]
    if ov > 0 or enclosing:
        collisions.append({"m": m, "ov": ov, "gap": enclosing[0] if enclosing else None})
        lost += ov if ov else (enclosing[0]["min"] if enclosing else 0)
    nxt = [p for p in prompts if p["t"] > m["end"]]
    if nxt:
        recovery.append(mins(m["end"], nxt[0]["t"]))

# --------------------------------------------------------------------- slack
sl = load(os.path.join(RAW, f"{day}-slack.json"))
msgs, lat_mine, lat_theirs, bursts_me, bursts_them = [], [], [], 0, 0
sync_in, sync_out, people, per_person_in = [], [], {}, {}
promises, knowledge, blocked = [], [], []
if sl is None:
    notes.append("No Slack pull for this day — direction, latency and collaboration metrics are **not measured**.")
else:
    cov = sl.get("coverage", {})
    if not cov.get("outbound_complete", True) or not cov.get("inbound_complete", True):
        notes.append(f"Slack pull is partial — {cov.get('note', 'see raw file')}")
    for m in sl.get("messages", []):
        t = parse_local(m.get("ts"))
        if not t:
            continue
        m["_t"] = t
        msgs.append(m)
        who, dirn, ch = m.get("who", "?"), m.get("dir"), m.get("ch", "?")
        if dirn == "in":
            people[who] = people.get(who, 0) + 1
            per_person_in[who] = per_person_in.get(who, 0) + 1
        if m.get("sync_request"):
            (sync_in if dirn == "in" else sync_out).append(m)
        if m.get("promise"):
            promises.append(m)
        if m.get("knowledge_share"):
            knowledge.append(m)
        if m.get("blocked"):
            blocked.append(m)
        ev(kind="message", ts=t.isoformat(), source="slack",
           direction="inbound" if dirn == "in" else "outbound", actor=who, channel=ch,
           words=len(str(m.get("text", "")).split()), text=m.get("text", ""),
           sync_request=bool(m.get("sync_request")),
           evidence=[f"slack:{ch}@{m['ts']}"])

    msgs.sort(key=lambda x: x["_t"])
    by_ch = {}
    for m in msgs:
        by_ch.setdefault(m.get("ch", "?"), []).append(m)
    for ch, lst in by_ch.items():
        prev = None
        for m in lst:
            if prev is None or mins(prev["_t"], m["_t"]) > BURST_GAP_MIN:
                if m["dir"] == "out":
                    bursts_me += 1
                else:
                    bursts_them += 1
            prev = m
        # reply latency: first opposite-direction message after each message
        for i, m in enumerate(lst):
            for nxt in lst[i + 1:]:
                if nxt["dir"] != m["dir"]:
                    d = (nxt["_t"] - m["_t"]).total_seconds()
                    if 0 < d <= 6 * 3600:
                        (lat_mine if m["dir"] == "in" else lat_theirs).append(d)
                    break

# ----------------------------------------------------------------------- git
commits, repos_touched, self_fix = [], set(), []
for name, path in REPOS:
    if not os.path.exists(os.path.join(path, ".git")):
        continue
    try:
        out = subprocess.run(
            ["git", "-C", path, "log", "--all", "--no-merges",
             f"--since={day} 00:00", f"--until={day} 23:59",
             "--pretty=format:%H|%h|%aI|%s"],
            capture_output=True, text=True, timeout=30).stdout
    except Exception:
        continue
    for line in out.splitlines():
        parts = line.split("|", 3)
        if len(parts) < 4:
            continue
        _full, short, iso, subj = parts
        t = parse_iso(iso)
        if not t or not (D0 <= t < D1):
            continue
        commits.append({"repo": name, "sha": short, "t": t, "subj": subj})
        repos_touched.add(name)
        ev(kind="commit", ts=t.isoformat(), source="git", repo=name, sha=short,
           title=subj, evidence=[f"git:{name}@{short}"])
commits.sort(key=lambda c: c["t"])
for i, c in enumerate(commits):
    if re.match(r"^fix\b", c["subj"], re.I) and any(
            e["repo"] == c["repo"] and e["t"] < c["t"] for e in commits[:i]):
        self_fix.append(c)

# ------------------------------------------------------------------- metrics
if have_transcripts:
    active = mins(prompts[0]["t"], prompts[-1]["t"])
    deep = [b for b in blocks if b["min"] >= DEEP_BLOCK_MIN]
    metric("focus.blocks", len(blocks), evidence=[f"transcript:#{b['first']:%H:%M}" for b in blocks[:8]])
    metric("focus.longest_min", round(max(b["min"] for b in blocks), 1), "minutes")
    multi = [b for b in blocks if b["n"] > 1]
    metric("focus.single_prompt_blocks", len(blocks) - len(multi),
           detail="one-off prompts with >20 min either side; they are touches, not sessions")
    if multi:
        metric("focus.median_min", round(statistics.median([b["min"] for b in multi]), 1), "minutes",
               detail="median over multi-prompt blocks only")
    metric("focus.total_min", round(sum(b["min"] for b in blocks), 1), "minutes")
    metric("focus.deep_ratio", round(sum(b["min"] for b in deep) / max(1, sum(b["min"] for b in blocks)), 2), "ratio")
    focused = sum(b["min"] for b in blocks)
    metric("focus.fragmentation", round(len(blocks) / max(1e-9, focused / 60), 2),
           "blocks/focused-hour", detail="higher means the same focused time is chopped finer")
    metric("focus.project_switches", switch_count,
           detail="counts changes of Claude Code project dir only; work on repos opened from one "
                  "project dir (e.g. divvy from Principal Engineering) is invisible here")
    metric("rework.agent_interrupts", len(interrupts), evidence=[f"transcript:{i['sid']}#{i['t']:%H:%M:%S}" for i in interrupts])
    metric("rework.tool_errors", len(tool_errors), evidence=[f"transcript:{e['sid']}#{e['t']:%H:%M:%S}" for e in tool_errors[:8]])
    metric("rework.timeouts", sum(1 for e in tool_errors if "timed out" in e["text"].lower()))
    metric("rework.blocked_commands", sum(1 for e in tool_errors if "blocked" in e["text"].lower()))
    metric("rework.context_restarts", len(restarts))
    metric("rework.model_switches", len(switches))
    metric("load.active_start", prompts[0]["t"].strftime("%H:%M"), "clock")
    metric("load.active_end", prompts[-1]["t"].strftime("%H:%M"), "clock")
    metric("load.span_hours", round(active / 60, 1), "hours")
    night_end = D0 + datetime.timedelta(hours=6)
    metric("load.night_min", round(sum(
        overlap_min(b["first"], b["last"], D0, night_end) for b in blocks), 1), "minutes",
        detail="block time overlapping 00:00-06:00")
    metric("load.longest_session_min", round(max(
        mins(s["first"], s["last"]) for s in sessions.values()), 1), "minutes")
    metric("interrupt.multitask_prompts", multitask,
           evidence=[f"gcal:{c['m']['id']}" for c in collisions])

if cal is not None:
    metric("interrupt.instant_meetings", len(instant), evidence=[f"gcal:{m['id']}" for m in instant],
           detail=[{"title": m["summary"], "at": m["start"].strftime("%H:%M"),
                    "notice_min": round(m["notice"], 1), "by": m["org"]} for m in instant])
    metric("interrupt.short_notice_meetings", len(short_notice), evidence=[f"gcal:{m['id']}" for m in short_notice])
    metric("interrupt.meetings_in_focus", len(collisions), evidence=[f"gcal:{c['m']['id']}" for c in collisions])
    metric("interrupt.focus_minutes_lost", round(lost, 1), "minutes",
           evidence=[f"gcal:{c['m']['id']}" for c in collisions])
    if recovery:
        metric("interrupt.recovery_median_min", round(statistics.median(recovery), 1), "minutes")
    metric("direction.meetings_organized", len(organized))
    metric("direction.meetings_inbound", len(inbound), evidence=[f"gcal:{m['id']}" for m in inbound])
    metric("direction.rsvp_missing", len(rsvp_missing), evidence=[f"gcal:{m['id']}" for m in rsvp_missing])
    metric("load.meetings_out_of_hours", sum(1 for m in meetings if m["start"].hour >= 20 or m["start"].hour < 9),
           evidence=[f"gcal:{m['id']}" for m in meetings if m["start"].hour >= 20 or m["start"].hour < 9])
    metric("output.client_facing_meetings", sum(1 for m in meetings if m["external"]))

if sl is not None:
    metric("direction.msgs_out", sum(1 for m in msgs if m["dir"] == "out"))
    metric("direction.msgs_in", sum(1 for m in msgs if m["dir"] == "in"))
    metric("direction.bursts_opened_by_me", bursts_me)
    metric("direction.bursts_opened_by_them", bursts_them)
    metric("direction.sync_requests_received", len(sync_in),
           evidence=[f"slack:{m['ch']}@{m['ts']}" for m in sync_in],
           detail=[{"at": m["ts"][11:16], "who": m["who"], "text": m["text"][:60]} for m in sync_in])
    metric("direction.sync_requests_made", len(sync_out))
    if lat_mine:
        metric("latency.mine_median_s", round(statistics.median(lat_mine)), "seconds")
        metric("latency.mine_p90_s", round(sorted(lat_mine)[int(0.9 * (len(lat_mine) - 1))]), "seconds")
    if lat_theirs:
        metric("latency.theirs_median_s", round(statistics.median(lat_theirs)), "seconds")
    if lat_mine and lat_theirs:
        metric("latency.reciprocity", round(statistics.median(lat_theirs) / max(1, statistics.median(lat_mine)), 2), "ratio")
    metric("collab.distinct_people", len({m["who"] for m in msgs if m["dir"] == "in"}))
    if per_person_in:
        top = max(per_person_in.items(), key=lambda kv: kv[1])
        metric("collab.top_counterparty", top[0], "person", detail={"inbound_msgs": top[1]})
    metric("interrupt.by_person", {m["who"]: sum(1 for x in sync_in if x["who"] == m["who"]) for m in sync_in},
           "map", evidence=[f"slack:{m['ch']}@{m['ts']}" for m in sync_in])
    metric("promise.made", len(promises), evidence=[f"slack:{m['ch']}@{m['ts']}" for m in promises], confirmed=False)
    metric("communication.knowledge_shares", len(knowledge),
           evidence=[f"slack:{m['ch']}@{m['ts']}" for m in knowledge])
    metric("collab.blocked_on_others", len(blocked),
           evidence=[f"slack:{m['ch']}@{m['ts']}" for m in blocked], confirmed=False)

metric("output.commits", len(commits), evidence=[f"git:{c['repo']}@{c['sha']}" for c in commits])
metric("output.repos_touched", len(repos_touched), detail=sorted(repos_touched))
metric("rework.self_fix_commits", len(self_fix), evidence=[f"git:{c['repo']}@{c['sha']}" for c in self_fix],
       confirmed=False)

# ------------------------------------------------------------------- outputs
os.makedirs(os.path.join(BRIEFS, "events/metrics"), exist_ok=True)
os.makedirs(os.path.join(BRIEFS, "metrics"), exist_ok=True)
jsonl = os.path.join(BRIEFS, f"events/metrics/{day}.jsonl")
with open(jsonl, "w", encoding="utf-8") as fh:
    for r in records + metrics:
        fh.write(json.dumps(r, default=str) + "\n")

M = {m["name"]: m for m in metrics}


def g(name, dash="not measured"):
    return M[name]["value"] if name in M else dash


L = [f"# Metrics — {D0:%A, %B %-d %Y}\n\n",
     f"Computed {datetime.datetime.now(IST):%-d %b %H:%M} IST from ",
     f"{'transcripts, ' if have_transcripts else ''}",
     f"{'calendar, ' if cal else ''}{'Slack, ' if sl else ''}git. ",
     f"Canonical record: `events/metrics/{day}.jsonl` ({len(records)} events, {len(metrics)} metrics). ",
     "Definitions in `SCHEMA.md`.\n\n"]
if notes:
    L.append("> **Coverage.** " + " ".join(notes) + "\n\n")

L.append("## Interruption\n\n")
L.append(f"| Metric | Value |\n|---|---|\n")
for n, label in [("interrupt.instant_meetings", "Instant meetings (≤5 min notice)"),
                 ("interrupt.short_notice_meetings", "Short-notice meetings (≤60 min)"),
                 ("direction.sync_requests_received", "Slack call/huddle asks received"),
                 ("direction.sync_requests_made", "Slack call asks I made"),
                 ("interrupt.meetings_in_focus", "Meetings landing inside a focus block"),
                 ("interrupt.focus_minutes_lost", "Focus minutes lost to meetings"),
                 ("interrupt.recovery_median_min", "Median recovery after a meeting"),
                 ("interrupt.multitask_prompts", "Prompts issued during a meeting")]:
    L.append(f"| {label} | {g(n)} |\n")
if "interrupt.instant_meetings" in M and M["interrupt.instant_meetings"].get("detail"):
    L.append("\n")
    for d in M["interrupt.instant_meetings"]["detail"]:
        L.append(f"- **{d['at']}** · {d['title']} · {d['notice_min']:.0f} min notice · by {d['by']}\n")

L.append("\n## Direction\n\n| Metric | Value |\n|---|---|\n")
for n, label in [("direction.meetings_organized", "Meetings I organised"),
                 ("direction.meetings_inbound", "Meetings booked on me"),
                 ("direction.rsvp_missing", "Meetings I never accepted"),
                 ("direction.bursts_opened_by_me", "Conversations I opened"),
                 ("direction.bursts_opened_by_them", "Conversations opened on me"),
                 ("direction.msgs_out", "Messages sent"),
                 ("direction.msgs_in", "Messages received"),
                 ("latency.mine_median_s", "My median reply (s)"),
                 ("latency.theirs_median_s", "Their median reply (s)"),
                 ("latency.reciprocity", "Reciprocity (theirs ÷ mine)")]:
    L.append(f"| {label} | {g(n)} |\n")

L.append("\n## Focus\n\n| Metric | Value |\n|---|---|\n")
for n, label in [("focus.blocks", "Focus blocks"), ("focus.longest_min", "Longest block (min)"),
                 ("focus.median_min", "Median block (min)"), ("focus.total_min", "Total focused (min)"),
                 ("focus.deep_ratio", "Share in blocks ≥45 min"),
                 ("focus.single_prompt_blocks", "of which single-prompt touches"),
                 ("focus.fragmentation", "Blocks per focused hour"),
                 ("focus.project_switches", "Project switches")]:
    L.append(f"| {label} | {g(n)} |\n")

L.append("\n## Output and rework\n\n| Metric | Value |\n|---|---|\n")
for n, label in [("output.commits", "Commits"), ("output.repos_touched", "Repos touched"),
                 ("output.client_facing_meetings", "Client-facing meetings"),
                 ("communication.knowledge_shares", "Knowledge shares in channels"),
                 ("rework.self_fix_commits", "Commits fixing my own same-day commit"),
                 ("rework.agent_interrupts", "Agent runs I interrupted"),
                 ("rework.tool_errors", "Tool errors"), ("rework.timeouts", "Command timeouts"),
                 ("rework.blocked_commands", "Commands blocked by guard"),
                 ("rework.context_restarts", "Context restarts"),
                 ("rework.model_switches", "Model switches")]:
    L.append(f"| {label} | {g(n)} |\n")

L.append("\n## Load\n\n| Metric | Value |\n|---|---|\n")
for n, label in [("load.active_start", "First activity"), ("load.active_end", "Last activity"),
                 ("load.span_hours", "Span (h)"), ("load.night_min", "Minutes 00:00–06:00"),
                 ("load.longest_session_min", "Longest single session (min)"),
                 ("load.meetings_out_of_hours", "Meetings before 09:00 or after 20:00")]:
    L.append(f"| {label} | {g(n)} |\n")

if commits:
    L.append("\n## Commits\n\n")
    for c in commits:
        L.append(f"- `{c['t']:%H:%M}` **{c['repo']}** `{c['sha']}` {c['subj']}\n")

L.append("\n---\n\nInterruption and load figures are context, not performance. "
         "Metrics marked `confirmed: false` in the JSONL are heuristics and are excluded from rollups "
         "until reviewed.\n")

report = os.path.join(BRIEFS, f"metrics/{day}.md")
with open(report, "w", encoding="utf-8") as fh:
    fh.write("".join(L))

print(f"{day}: {len(records)} events, {len(metrics)} metrics")
print(f"  {jsonl}")
print(f"  {report}")
for k in ("interrupt.instant_meetings", "interrupt.focus_minutes_lost", "direction.sync_requests_received",
          "latency.mine_median_s", "latency.theirs_median_s", "output.commits", "focus.blocks"):
    print(f"  {k} = {g(k)}")
