#!/usr/bin/env python3
"""Evidence-backed model routing for the keyless Claude/Codex boundary.

This module aggregates only recorded outcomes. It never calls a model and it
never writes the append-only receipt ledger.
"""
import argparse
import csv
import json
import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
ROOT = REPO_ROOT
REPUTATION = REPO_ROOT / "registry" / "features" / "reputation" / "reputation.sh"
STATIC = {"lead": "opus", "build": "sonnet", "verify": "sonnet", "review": "opus", "mechanical": "haiku", "research": "haiku"}
KINDS = ("build", "verify", "review", "mechanical")
PERFORMANCE_FIELDS = (
    "model", "task_kind", "attempts", "accepted", "revised", "rejected",
    "gate_failed", "measured_tokens", "mean_tokens", "mean_wall_sec",
    "measured_cost_micro_usd", "cost_known_attempts", "cost_per_accepted_change_micro_usd",
    "evidence",
)


def model_name(raw):
    value = (raw or "").strip().lower()
    if not value:
        return ""
    if value.startswith(("codex", "gpt-")):
        return "codex"
    if value.startswith(("claude-opus", "opus")):
        return "opus"
    if value.startswith(("claude-sonnet", "sonnet")):
        return "sonnet"
    if value.startswith(("claude-haiku", "haiku")):
        return "haiku"
    if value.startswith("claude"):
        return "claude"
    return value if value in {"codex", "opus", "sonnet", "haiku", "claude"} else ""


def integer(value, default=0):
    try:
        return int(str(value))
    except (TypeError, ValueError):
        return default


def float_value(value, default=0.0):
    try:
        return float(str(value))
    except (TypeError, ValueError):
        return default


def read_receipts(path):
    rows = []
    parse_errors = 0
    if not path.is_file():
        return rows, 0, "missing"
    with path.open(encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
                if not isinstance(row, dict):
                    raise ValueError("receipt is not an object")
                row["_line"] = line_number
                rows.append(row)
            except (json.JSONDecodeError, ValueError):
                parse_errors += 1
    return rows, parse_errors, "ok"


def parse_meta(path):
    result = {}
    with path.open(encoding="utf-8") as handle:
        for line in handle:
            if "=" in line:
                key, value = line.rstrip("\n").split("=", 1)
                result[key] = value
    return result


def role_kind(value):
    value = (value or "").lower()
    if value in {"verification", "verify"}:
        return "verify"
    if value == "review":
        return "review"
    if value in {"mechanical", "docs", "research"}:
        return "mechanical"
    if value in {"lead", "architect"}:
        return "review"
    if value in {"build", "builder", "implementation", "debugging"}:
        return "build"
    return ""


def receipt_kind(row):
    component = str(row.get("component", "")).lower()
    event = str(row.get("event", "")).lower()
    if event in {"pass", "fail"}:
        return "verify"
    if component in {"builder", "build", "implementation", "debugging"}:
        return "build"
    if component in {"verification", "verify"}:
        return "verify"
    if component in {"review", "architect", "lead"}:
        return "review"
    if component in {"mechanical", "docs", "research"}:
        return "mechanical"
    return ""


def real_model(raw):
    return model_name(raw)


def outcome(state, exit_code, event=""):
    if state == "needs-iteration":
        return "revised"
    if event.lower() == "fail" or state == "failed" or exit_code != 0:
        return "rejected"
    return "accepted"


def make_sample(model, kind, result, task_id, tokens=0, wall=0.0, cost=0, gate_failed=0, source=""):
    if not model or kind not in KINDS:
        return None
    return {
        "model": model, "task_kind": kind, "outcome": result,
        "task_id": task_id, "tokens": max(0, tokens), "wall": max(0.0, wall),
        "cost": max(0, cost), "cost_known": cost > 0, "gate_failed": gate_failed,
        "source": source,
    }


def build_samples(ledger_rows, work_dir):
    dispatch_by_task = {}
    failures_by_task = set()
    for row in ledger_rows:
        task_id = str(row.get("task_id", ""))
        if row.get("event") == "dispatch":
            dispatch_by_task.setdefault(task_id, []).append(row)
        if row.get("event") == "FAIL":
            failures_by_task.add(task_id)

    samples = []
    seen_tasks = set()
    meta_files = sorted(work_dir.glob("*/meta.txt")) if work_dir.is_dir() else []
    for meta_path in meta_files:
        meta = parse_meta(meta_path)
        task_id = meta.get("id", meta_path.parent.name)
        dispatch = dispatch_by_task.get(task_id, [])
        dispatch_row = dispatch[-1] if dispatch else {}
        kind = role_kind(meta.get("role")) or receipt_kind(dispatch_row)
        raw_model = meta.get("model", "") or meta.get("harness", "") or dispatch_row.get("generator_model", "")
        model = real_model(raw_model)
        sample = make_sample(
            model, kind, outcome(meta.get("state", ""), integer(meta.get("exit", 0))), task_id,
            integer(meta.get("tokens", dispatch_row.get("tokens_out", 0))),
            float_value(meta.get("elapsed_sec", 0)), integer(meta.get("cost_micro_usd", dispatch_row.get("cost_micro_usd", 0))),
            int(task_id in failures_by_task), "work/meta.txt",
        )
        if sample:
            samples.append(sample)
            seen_tasks.add(task_id)

    for row in ledger_rows:
        if row.get("event") != "dispatch":
            continue
        task_id = str(row.get("task_id", ""))
        if task_id in seen_tasks:
            continue
        kind = receipt_kind(row)
        sample = make_sample(
            real_model(row.get("generator_model", "")), kind,
            outcome("", integer(row.get("exit_code", 0))), task_id,
            integer(row.get("tokens_out", 0)), 0.0, integer(row.get("cost_micro_usd", 0)),
            int(task_id in failures_by_task), "ledger/RECEIPTS.jsonl:dispatch",
        )
        if sample:
            samples.append(sample)
            seen_tasks.add(task_id)

    # A PASS/FAIL gate is evidence about the verifier, while its FAIL bit is
    # also preserved as a first-class field. No cross-task attribution is made.
    for row in ledger_rows:
        if row.get("event") not in {"PASS", "FAIL"}:
            continue
        sample = make_sample(
            real_model(row.get("verifier_model", "")), "verify",
            outcome("", integer(row.get("exit_code", 0)), str(row.get("event", ""))),
            "gate:" + str(row.get("task_id", "")) + ":" + str(row.get("_line", "")),
            integer(row.get("tokens_out", 0)), 0.0, integer(row.get("cost_micro_usd", 0)),
            int(row.get("event") == "FAIL"), "ledger/RECEIPTS.jsonl:" + str(row.get("_line", "")),
        )
        if sample:
            samples.append(sample)
    return samples, len(meta_files)


def ccusage_snapshot(path=None, skip=False):
    if path:
        try:
            return json.loads(Path(path).read_text(encoding="utf-8")), "fixture"
        except (OSError, json.JSONDecodeError):
            return {}, "unparseable"
    if skip:
        return {}, "skipped"
    try:
        completed = subprocess.run(
            ["ccusage", "daily", "--json", "--offline"], capture_output=True, text=True, timeout=30, check=False
        )
        if completed.returncode != 0:
            return {}, "failed"
        return json.loads(completed.stdout), "live"
    except (OSError, subprocess.TimeoutExpired, json.JSONDecodeError):
        return {}, "unavailable"


def ccusage_cost(snapshot):
    total = integer(round(float_value(snapshot.get("totals", {}).get("totalCost", 0)) * 1000000))
    by_model = {}
    for day in snapshot.get("daily", []):
        for item in day.get("modelBreakdowns", []):
            name = model_name(item.get("modelName", ""))
            if name:
                by_model[name] = by_model.get(name, 0) + integer(round(float_value(item.get("cost", 0)) * 1000000))
    return total, by_model


def aggregate(samples):
    grouped = {}
    for sample in samples:
        key = (sample["model"], sample["task_kind"])
        grouped.setdefault(key, []).append(sample)
    records = []
    for (model, kind), rows in sorted(grouped.items()):
        accepted = sum(row["outcome"] == "accepted" for row in rows)
        revised = sum(row["outcome"] == "revised" for row in rows)
        rejected = sum(row["outcome"] == "rejected" for row in rows)
        measured_cost = sum(row["cost"] for row in rows if row["cost_known"])
        known = sum(row["cost_known"] for row in rows)
        cost_per = "unknown"
        if accepted and known == len(rows):
            cost_per = str(measured_cost // accepted)
        evidence = ";".join(row["source"] + ":" + row["task_id"] for row in rows[:5])
        records.append({
            "model": model, "task_kind": kind, "attempts": len(rows), "accepted": accepted,
            "revised": revised, "rejected": rejected, "gate_failed": sum(row["gate_failed"] for row in rows),
            "measured_tokens": sum(row["tokens"] for row in rows),
            "mean_tokens": sum(row["tokens"] for row in rows) // len(rows),
            "mean_wall_sec": round(sum(row["wall"] for row in rows) / len(rows), 2),
            "measured_cost_micro_usd": measured_cost, "cost_known_attempts": known,
            "cost_per_accepted_change_micro_usd": cost_per, "evidence": evidence,
        })
    return records


def write_record(state, records, metadata):
    state.mkdir(parents=True, exist_ok=True)
    record_path = state / "performance.tsv"
    with record_path.open("w", encoding="utf-8", newline="") as handle:
        handle.write("# source_ledger_rows={ledger_rows} work_meta_files={work_meta_files} parse_errors={parse_errors} ccusage={ccusage}\n".format(**metadata))
        writer = csv.DictWriter(handle, fieldnames=PERFORMANCE_FIELDS, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(records)
    (state / "source.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return record_path


def load_record(path):
    records = []
    if not path.is_file():
        return records
    with path.open(encoding="utf-8", newline="") as handle:
        for row in csv.DictReader((line for line in handle if not line.startswith("#")), delimiter="\t"):
            for field in ("attempts", "accepted", "revised", "rejected", "gate_failed", "measured_tokens", "mean_tokens", "cost_known_attempts"):
                row[field] = integer(row.get(field))
            row["mean_wall_sec"] = float_value(row.get("mean_wall_sec"))
            row["measured_cost_micro_usd"] = integer(row.get("measured_cost_micro_usd"))
            records.append(row)
    return records


def make_record(args):
    ledger = Path(args.ledger or os.environ.get("FLEET_LEDGER", ROOT / "ledger/RECEIPTS.jsonl"))
    work = Path(args.work_dir or os.environ.get("FLEET_WORK", ROOT / "var" / "runs"))
    state = Path(args.state or os.environ.get("FLEET_ROUTER_STATE", ROOT / "state/router"))
    rows, parse_errors, ledger_state = read_receipts(ledger)
    if parse_errors:
        print("router: receipt ledger has unparseable rows", file=sys.stderr)
        return 4
    samples, meta_count = build_samples(rows, work)
    snapshot, cc_state = ccusage_snapshot(args.ccusage_json, args.no_ccusage)
    cc_total, cc_by_model = ccusage_cost(snapshot)
    records = aggregate(samples)
    metadata = {
        "ledger": str(ledger), "ledger_rows": len(rows), "ledger_state": ledger_state,
        "work_dir": str(work), "work_meta_files": meta_count, "parse_errors": parse_errors,
        "ccusage": cc_state, "ccusage_period_cost_micro_usd": cc_total,
        "ccusage_by_model_micro_usd": cc_by_model,
    }
    record_path = write_record(state, records, metadata)
    payload = {"ok": True, "record": str(record_path), "metadata": metadata, "performance": records}
    if not getattr(args, "quiet", False):
        emit(payload, args.json)
    return 0


def static_model(kind):
    if kind == "verify":
        return os.environ.get("FLEET_MODEL_VERIFY", STATIC["verify"])
    if kind == "review":
        return os.environ.get("FLEET_MODEL_LEAD", STATIC["review"])
    if kind == "mechanical":
        return os.environ.get("FLEET_MODEL_MECH", STATIC["mechanical"])
    return os.environ.get("FLEET_MODEL_BUILD", STATIC["build"])


def static_role_model(role):
    override = os.environ.get("FLEET_MODEL_" + role.upper())
    if override:
        return override
    if os.environ.get("FLEET_PLAN", "max") == "pro" and role in {"lead", "build", "verify"}:
        return "sonnet"
    return STATIC.get(role, "haiku")


def reputation_report(args):
    command = [str(REPUTATION), "report", "--json"]
    if getattr(args, "ledger", None):
        command.extend(["--ledger", args.ledger])
    if getattr(args, "work_dir", None):
        command.extend(["--runs", args.work_dir])
    if getattr(args, "ccusage_json", None):
        command.extend(["--ccusage-json", args.ccusage_json])
    if getattr(args, "no_ccusage", False):
        command.append("--no-ccusage")
    try:
        completed = subprocess.run(command, cwd=REPO_ROOT, capture_output=True, text=True, timeout=60, check=False)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if completed.returncode != 0:
        return None
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError:
        return None


def reputation_roles(kind):
    return {
        "build": {"build", "builder", "implementation", "debugging"},
        "verify": {"verify", "verification"},
        "review": {"review", "lead", "architect"},
        "mechanical": {"mechanical", "docs"},
    }.get(kind, {kind})


def choose_reputation(report, roles):
    if not report:
        return None
    candidates = [row for row in report.get("reputation", [])
                  if row.get("role") in roles and row.get("verdict") == "rankable"]
    if not candidates:
        return None
    return sorted(candidates, key=lambda row: (
        integer(row.get("cost_per_accepted_change_micro_usd"), 10**30),
        -integer(row.get("accepted")),
        integer(row.get("rejected"), 10**30),
        row.get("model", ""),
        row.get("role", ""),
    ))[0]


def reputation_reason(selected):
    return (
        "reputation citation: %s/%s n=%s accepted=%s revised=%s rejected=%s "
        "later_gate_failed=%s cost_per_accepted_change=%s USD"
        % (
            selected["model"], selected["role"], selected["sample_size"],
            selected["accepted"], selected["revised"], selected["rejected"],
            selected["later_gate_failed"], selected["cost_per_accepted_change_usd"],
        )
    )


def route_role(role):
    args = argparse.Namespace(
        ledger=None, work_dir=None, ccusage_json=None, no_ccusage=False,
    )
    report = reputation_report(args)
    selected = choose_reputation(report, reputation_roles(role))
    model = selected["model"] if selected else static_role_model(role)
    if selected:
        print(model)
        print(reputation_reason(selected), file=sys.stderr)
    else:
        print(model)
        print("model-for: falling back to the static table (insufficient data) role=" + role, file=sys.stderr)
    return 0


def route(args):
    state = Path(args.state or os.environ.get("FLEET_ROUTER_STATE", ROOT / "state/router"))
    record_path = state / "performance.tsv"
    if args.refresh or not record_path.is_file():
        record_args = argparse.Namespace(
            ledger=args.ledger, work_dir=args.work_dir, state=str(state), ccusage_json=args.ccusage_json,
            no_ccusage=args.no_ccusage, json=False, quiet=True,
        )
        result = make_record(record_args)
        if result != 0:
            return result
    records = [row for row in load_record(record_path) if row.get("task_kind") == args.kind]
    if args.specified == "no":
        payload = {"ok": False, "exit": 7, "reason": "routing refused: task is not fully specified", "kind": args.kind}
        emit(payload, args.json)
        return 7

    if not args.no_ccusage:
        reputation = reputation_report(args)
        reputation_selected = choose_reputation(reputation, reputation_roles(args.kind))
        if reputation_selected:
            model = reputation_selected["model"]
            reason = reputation_reason(reputation_selected)
            payload = {
                "ok": True, "kind": args.kind, "size": args.size, "fully_specified": args.specified,
                "needs_web": args.web, "deterministic_required": args.deterministic, "model": model,
                "source": "reputation evidence", "reason": reason, "evidence": reputation_selected,
                "feature_conditioning": "reputation is aggregated by recorded model-role cells; no feature-specific labels are invented",
            }
            emit(payload, args.json)
            return 0
        model = static_model(args.kind)
        reason = "falling back to the static table (insufficient data) for " + args.kind
        payload = {
            "ok": True, "kind": args.kind, "size": args.size, "fully_specified": args.specified,
            "needs_web": args.web, "deterministic_required": args.deterministic, "model": model,
            "source": "static fallback (insufficient data)", "reason": reason,
            "evidence": {"fallback_model": model, "static_table": True},
            "feature_conditioning": "reputation cells below the stated sample floor are not ranked",
        }
        emit(payload, args.json)
        return 0

    candidates = [row for row in records if row.get("accepted", 0) > 0]
    if args.deterministic == "yes":
        clean = [row for row in candidates if row.get("gate_failed", 0) == 0]
        if clean:
            candidates = clean
    source = "static fallback"
    reason = "insufficient history for " + args.kind
    selected = None
    if candidates:
        with_cost = [row for row in candidates if row.get("cost_per_accepted_change_micro_usd") not in {"", "unknown", None}]
        if with_cost:
            selected = sorted(with_cost, key=lambda row: (integer(row["cost_per_accepted_change_micro_usd"], 10**30), -row["accepted"], row["mean_tokens"], row["model"]))[0]
            source = "recorded cost-per-accepted-change history"
            reason = f"{selected['model']} for {args.kind} — {selected['accepted']} accepted, {selected['rejected']} rejected, mean {selected['mean_tokens']} tokens, cost/accepted {selected['cost_per_accepted_change_micro_usd']} micro-USD"
        else:
            selected = sorted(candidates, key=lambda row: (-row["accepted"], row["rejected"], row["mean_tokens"], row["model"]))[0]
            source = "recorded outcome history; cost attribution unavailable"
            reason = f"{selected['model']} for {args.kind} — {selected['accepted']} accepted, {selected['rejected']} rejected, mean {selected['mean_tokens']} tokens; cost/accepted unknown"
    model = selected["model"] if selected else static_model(args.kind)
    payload = {
        "ok": True, "kind": args.kind, "size": args.size, "fully_specified": args.specified,
        "needs_web": args.web, "deterministic_required": args.deterministic, "model": model,
        "source": source, "reason": reason,
        "evidence": selected or {"fallback_model": model, "static_table": True},
        "feature_conditioning": "history is aggregated by task-kind; size and web are recorded inputs, not retroactively invented labels",
    }
    decision_path = state / "decisions.tsv"
    state.mkdir(parents=True, exist_ok=True)
    with decision_path.open("a", encoding="utf-8") as handle:
        handle.write("\t".join([args.kind, args.size, args.specified, args.web, args.deterministic, model, source, reason]) + "\n")
    emit(payload, args.json)
    return 0


def emit(payload, as_json):
    if as_json:
        print(json.dumps(payload, sort_keys=True))
        return
    if "performance" in payload:
        print(f"record: {payload['record']}")
        print(f"source: ledger_rows={payload['metadata']['ledger_rows']} work_meta_files={payload['metadata']['work_meta_files']} ccusage={payload['metadata']['ccusage']}")
        for row in payload["performance"]:
            print("performance: " + "\t".join(str(row.get(field, "")) for field in PERFORMANCE_FIELDS))
    else:
        print(f"model: {payload.get('model', '')}")
        print(f"source: {payload.get('source', '')}")
        print(f"reason: {payload.get('reason', '')}")


def parser():
    root = argparse.ArgumentParser(prog="model-for.sh")
    sub = root.add_subparsers(dest="command")
    for role in ("lead", "build", "verify", "mechanical", "research"):
        sub.add_parser(role)
    record = sub.add_parser("record")
    route_record_args(record)
    report = sub.add_parser("report")
    route_record_args(report)
    route = sub.add_parser("route")
    route.add_argument("--kind", choices=KINDS, required=True)
    route.add_argument("--size", choices=("small", "medium", "large"), required=True)
    route.add_argument("--specified", choices=("yes", "no"), required=True)
    route.add_argument("--web", choices=("yes", "no"), default="no")
    route.add_argument("--deterministic", choices=("yes", "no"), default="no")
    route.add_argument("--ledger")
    route.add_argument("--work-dir")
    route.add_argument("--state")
    route.add_argument("--ccusage-json")
    route.add_argument("--no-ccusage", action="store_true")
    route.add_argument("--refresh", action="store_true")
    route.add_argument("--json", action="store_true")
    return root


def route_record_args(command):
    command.add_argument("--ledger")
    command.add_argument("--work-dir")
    command.add_argument("--state")
    command.add_argument("--ccusage-json")
    command.add_argument("--no-ccusage", action="store_true")
    command.add_argument("--json", action="store_true")


def main(argv):
    args = parser().parse_args(argv)
    if args.command in {"lead", "build", "verify", "mechanical", "research"}:
        return route_role(args.command)
    if args.command == "record":
        return make_record(args)
    if args.command == "report":
        state = Path(args.state or os.environ.get("FLEET_ROUTER_STATE", ROOT / "state/router"))
        records = load_record(state / "performance.tsv")
        emit({"ok": True, "record": str(state / "performance.tsv"), "performance": records}, args.json)
        return 0
    if args.command == "route":
        return route(args)
    parser().print_help(sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
