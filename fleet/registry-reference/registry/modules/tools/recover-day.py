#!/usr/bin/env python3
"""Recover one IST day of Claude Code activity from ~/.claude transcripts into markdown.

Writes:
  daily-briefs/events/sessions/<day>--<sid>--<slug>.md   one file per session, verbatim
  daily-briefs/events/<day>.md                           a recovered-timeline index section

Timestamps are the transcript's own, so exact. Re-running rewrites both outputs.
Read-only against ~/.claude.

A transcript's user records are not all prompts: /model switches, slash-command stdout,
local-command caveats, interrupt markers and context-continuation summaries all appear
as user turns. Counting those as prompts inflates the day badly, so they are classified
as meta and reported separately.
"""
import json, sys, os, glob, re, datetime
from zoneinfo import ZoneInfo

IST = ZoneInfo("Asia/Kolkata")
ROOT = os.path.expanduser("~/youtube/Principal Engineering/daily-briefs/events")
day = sys.argv[1] if len(sys.argv) > 1 else datetime.datetime.now(IST).strftime("%Y-%m-%d")
NOW = datetime.datetime.now(IST)
BEGIN = f"<!-- BEGIN RECOVERED {day} -->"
END = f"<!-- END RECOVERED {day} -->"


def ist(ts):
    try:
        return datetime.datetime.fromisoformat(str(ts).replace("Z", "+00:00")).astimezone(IST)
    except Exception:
        return None


def text_of(content):
    if isinstance(content, str):
        return content
    out = []
    if isinstance(content, list):
        for it in content:
            if isinstance(it, dict) and it.get("type") == "text":
                out.append(str(it.get("text", "")))
    return "\n".join(out)


def classify(body):
    """Return (kind, label). kind is 'prompt' or a meta flavour."""
    s = body.strip()
    if not s:
        return "meta", "empty turn"
    if s.startswith("[Request interrupted"):
        return "interrupt", "interrupted by user"
    m = re.match(r"<command-name>(.*?)</command-name>", s)
    if m:
        return "command", m.group(1).strip()
    if s.startswith("<local-command-stdout>"):
        inner = re.sub(r"</?local-command-stdout>", "", s).strip()
        return "command-out", " ".join(inner.split())[:120]
    if s.startswith("<local-command-caveat>"):
        return "meta", "local-command caveat"
    if s.startswith("<command-message>"):
        return "meta", "command message"
    if s.startswith("This session is being continued from a previous conversation"):
        return "continuation", "context ran out; session resumed"
    if s.startswith("A session-scoped Stop hook is now active"):
        return "meta", "session-scoped Stop hook armed"
    if s.startswith("[Your previous response had no visible output"):
        return "meta", "empty-response retry"
    if s.startswith("Caveat:"):
        return "meta", "caveat"
    return "prompt", ""


def errors_of(content):
    found = []
    if isinstance(content, list):
        for it in content:
            if isinstance(it, dict) and it.get("type") == "tool_result" and it.get("is_error"):
                t = it.get("content")
                if isinstance(t, list):
                    t = " ".join(str(x.get("text", "")) for x in t if isinstance(x, dict))
                found.append(" ".join(str(t or "").split()))
    return found


def is_tool_result(content):
    return isinstance(content, list) and any(
        isinstance(it, dict) and it.get("type") == "tool_result" for it in content
    )


sessions = {}
for path in glob.glob(os.path.expanduser("~/.claude/projects/*/*.jsonl")):
    if datetime.datetime.fromtimestamp(os.path.getmtime(path), IST).strftime("%Y-%m-%d") < day:
        continue
    events, first, last, replies, awaiting = [], None, None, 0, False
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
                t = ist(rec.get("timestamp"))
                if not t or t.strftime("%Y-%m-%d") != day:
                    continue
                # Session window spans ALL activity, not just prompts.
                first = min(first or t, t)
                last = max(last or t, t)
                content = (rec.get("message") or {}).get("content")
                if rec.get("type") == "user":
                    if is_tool_result(content):
                        for e in errors_of(content):
                            events.append({"t": t, "kind": "error", "text": e})
                        continue
                    body = text_of(content)
                    kind, label = classify(body)
                    events.append({"t": t, "kind": kind, "text": body, "label": label})
                    awaiting = kind == "prompt"
                elif rec.get("type") == "assistant":
                    replies += 1
                    body = text_of(content).strip()
                    if body and awaiting:
                        for ev in reversed(events):
                            if ev["kind"] == "prompt":
                                ev["reply"] = " ".join(body.split())[:220]
                                break
                        awaiting = False
    except Exception:
        continue
    if any(e["kind"] == "prompt" for e in events):
        sid = os.path.basename(path)[:8]
        proj = os.path.basename(os.path.dirname(path)).replace("-Users-rachitsrivastava-", "").strip("-")
        sessions[sid] = {"events": sorted(events, key=lambda e: e["t"]), "proj": proj,
                         "path": path, "first": first, "last": last, "replies": replies}

os.makedirs(os.path.join(ROOT, "sessions"), exist_ok=True)
rows = []

for sid, s in sorted(sessions.items(), key=lambda kv: kv[1]["first"]):
    ev = s["events"]
    prompts = [e for e in ev if e["kind"] == "prompt"]
    errs = [e for e in ev if e["kind"] == "error"]
    interrupts = [e for e in ev if e["kind"] == "interrupt"]
    conts = [e for e in ev if e["kind"] == "continuation"]
    models = [e for e in ev if e["kind"] == "command-out" and "Set model" in (e.get("label") or "")]
    mins = round((s["last"] - s["first"]).total_seconds() / 60)
    span = f"{mins} min" if mins < 90 else f"{mins // 60}h {mins % 60}m"
    slug = s["proj"].split("-")[-1][:24] or "project"
    fname = f"{day}--{sid}--{slug}.md"

    def plural(n, w):
        if n == 1:
            return f"{n} {w}"
        if w == "reply":
            return f"{n} replies"
        return f"{n} " + w + ("es" if w.endswith(("ch", "sh", "s", "x", "z")) else "s")

    L = [f"# Session `{sid}` — {s['first']:%A, %B %-d %Y}\n\n",
         f"**{s['proj']}** · {s['first']:%H:%M}–{s['last']:%H:%M} IST · {span}\n\n",
         f"{plural(len(prompts), 'prompt')} · {plural(s['replies'], 'reply')} · "
         f"{plural(len(errs), 'tool error')} · {plural(len(interrupts), 'interrupt')}"
         + (f" · {plural(len(models), 'model switch')}" if models else "")
         + (f" · {plural(len(conts), 'context restart')}" if conts else "") + "\n\n",
         f"> Recovered from `{s['path'].replace(os.path.expanduser('~'), '~')}` on "
         f"{NOW:%-d %b %H:%M} IST. Timestamps are the transcript's own, so exact. Prompts verbatim; "
         f"assistant replies truncated to their opening line; reasoning omitted. Slash commands, "
         f"model switches and interrupt markers are shown inline but not counted as prompts.\n\n---\n"]
    n = 0
    for e in ev:
        if e["kind"] == "prompt":
            n += 1
            L.append(f"\n## {e['t']:%H:%M:%S} · prompt {n}\n\n")
            L.append("\n".join("> " + ln for ln in (e["text"] or "(empty)").split("\n")) + "\n")
            if e.get("reply"):
                L.append(f"\n*reply opened:* {e['reply']}\n")
        elif e["kind"] == "error":
            L.append(f"\n**{e['t']:%H:%M:%S} · tool error** — `{e['text'][:400]}`\n")
        elif e["kind"] == "interrupt":
            L.append(f"\n**{e['t']:%H:%M:%S} · interrupted by user**\n")
        elif e["kind"] == "continuation":
            L.append(f"\n**{e['t']:%H:%M:%S} · context ran out — session resumed from a summary**\n")
        elif e["kind"] == "command":
            L.append(f"\n`{e['t']:%H:%M:%S}` · command `{e['label']}`\n")
        elif e["kind"] == "command-out":
            L.append(f"\n`{e['t']:%H:%M:%S}` · {e['label']}\n")
    with open(os.path.join(ROOT, "sessions", fname), "w", encoding="utf-8") as fh:
        fh.write("".join(L))

    rows.append({"sid": sid, "first": s["first"], "last": s["last"], "prompts": prompts,
                 "errs": errs, "interrupts": interrupts, "conts": conts, "models": models,
                 "fname": fname, "span": span})

tp = sum(len(r["prompts"]) for r in rows)
te = sum(len(r["errs"]) for r in rows)
ti = sum(len(r["interrupts"]) for r in rows)
tc = sum(len(r["conts"]) for r in rows)

I = [BEGIN, "\n\n## Recovered from Claude Code transcripts\n\n",
     f"**{len(rows)} sessions · {tp} real prompts · {te} tool errors · {ti} interruptions"
     + (f" · {tc} context restart" + ("" if tc == 1 else "s") if tc else "") + "**  \n",
     f"First activity {min(r['first'] for r in rows):%H:%M}, last {max(r['last'] for r in rows):%H:%M} IST. ",
     f"Recovered {NOW:%-d %b %H:%M} IST because the logging hook went live mid-day; ",
     "everything from now on is captured live. Full verbatim prompts in `sessions/`.\n\n",
     "| Window (IST) | Span | Session | Prompts | Errors | Interrupts | Detail |\n",
     "|---|---|---|---|---|---|---|\n"]
for r in rows:
    I.append(f"| {r['first']:%H:%M}–{r['last']:%H:%M} | {r['span']} | `{r['sid']}` | {len(r['prompts'])} | "
             f"{len(r['errs'])} | {len(r['interrupts'])} | [`{r['fname']}`](sessions/{r['fname']}) |\n")
I.append("\n### Prompt-by-prompt\n")
for r in rows:
    I.append(f"\n**`{r['sid']}`** · {r['first']:%H:%M}–{r['last']:%H:%M}"
             + (f" · {len(r['conts'])} context restart(s)" if r["conts"] else "") + "\n\n")
    for p in r["prompts"]:
        line = (p["text"] or "").strip().split("\n")[0][:110]
        I.append(f"- `{p['t']:%H:%M:%S}` {line}\n")
    if r["errs"]:
        I.append(f"\n  *errors:* " + "; ".join(f"`{e['t']:%H:%M}` {e['text'][:90]}" for e in r["errs"][:6]) + "\n")
I.append("\n" + END + "\n")

daylog = os.path.join(ROOT, f"{day}.md")
existing = open(daylog, encoding="utf-8").read() if os.path.exists(daylog) else ""
block = "".join(I)
if BEGIN in existing and END in existing:
    pre, rest = existing.split(BEGIN, 1)
    _, post = rest.split(END, 1)
    new = pre + block + post
    action = "replaced"
else:
    # Drop the older unmarked recovery section if a previous version wrote one.
    existing = existing.split("<!-- RECOVERED-FROM-TRANSCRIPTS")[0].rstrip() + "\n"
    new = existing + "\n" + block
    action = "appended"
with open(daylog, "w", encoding="utf-8") as fh:
    fh.write(new)

print(f"index {action}: {len(rows)} sessions, {tp} real prompts, {te} errors, {ti} interrupts, {tc} restarts")
for r in rows:
    print(f"  {r['first']:%H:%M}-{r['last']:%H:%M} {r['sid']} {len(r['prompts'])}p {len(r['errs'])}e -> {r['fname']}")
