#!/usr/bin/env python3
"""Survey Claude Code transcripts for one IST day. Compact output, read-only."""
import json, sys, os, glob, datetime, collections
from zoneinfo import ZoneInfo

IST = ZoneInfo("Asia/Kolkata")
day = sys.argv[1] if len(sys.argv) > 1 else datetime.datetime.now(IST).strftime("%Y-%m-%d")

def ist(ts):
    try:
        return datetime.datetime.fromisoformat(ts.replace("Z", "+00:00")).astimezone(IST)
    except Exception:
        return None

sessions = collections.defaultdict(lambda: {"prompts": 0, "errors": 0, "first": None, "last": None, "proj": "", "file": ""})
for path in glob.glob(os.path.expanduser("~/.claude/projects/*/*.jsonl")):
    if datetime.datetime.fromtimestamp(os.path.getmtime(path), IST).strftime("%Y-%m-%d") < day:
        continue
    proj = os.path.basename(os.path.dirname(path))
    sid = os.path.basename(path)[:8]
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            for raw in fh:
                try:
                    rec = json.loads(raw)
                except Exception:
                    continue
                t = ist(rec.get("timestamp") or "")
                if not t or t.strftime("%Y-%m-%d") != day:
                    continue
                s = sessions[(sid, proj)]
                s["proj"], s["file"] = proj, path
                s["first"] = min(s["first"] or t, t)
                s["last"] = max(s["last"] or t, t)
                if rec.get("type") == "user":
                    c = (rec.get("message") or {}).get("content")
                    if isinstance(c, list):
                        for it in c:
                            if isinstance(it, dict) and it.get("type") == "tool_result" and it.get("is_error"):
                                s["errors"] += 1
                        if not any(isinstance(it, dict) and it.get("type") == "tool_result" for it in c):
                            s["prompts"] += 1
                    else:
                        s["prompts"] += 1
    except Exception:
        continue

rows = sorted((v for v in sessions.values() if v["prompts"] or v["errors"]), key=lambda r: r["first"])
print(f"DAY {day} · {len(rows)} sessions · {sum(r['prompts'] for r in rows)} prompts · {sum(r['errors'] for r in rows)} errors")
for r in rows:
    print(f"{r['first']:%H:%M}-{r['last']:%H:%M} | {r['prompts']:>3}p {r['errors']:>2}e | {r['proj'][-42:]} | {os.path.basename(r['file'])[:8]}")
