#!/usr/bin/env bash
# reputation.sh — deterministic reputation evidence from recorded fleet work.
#
# This feature reads run metadata, the append-only receipt ledger, and the
# keyless ccusage report. It never calls a model and never writes the ledger.
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
FLEET_REPUTATION_ROOT="$ROOT" exec python3 - "$@" <<'PY'
import argparse
import json
import os
import subprocess
import sys
from collections import defaultdict
from decimal import Decimal, InvalidOperation, ROUND_HALF_UP
from pathlib import Path

ROOT = Path(os.environ.get("FLEET_REPUTATION_ROOT", Path.cwd()))
DEFAULT_MIN_SAMPLE = 10


def integer(value, default=0):
    try:
        return int(str(value))
    except (TypeError, ValueError):
        return default


def model_name(raw):
    value = str(raw or "").strip().lower()
    if not value:
        return ""
    if value.startswith(("codex", "gpt-")) or "codex" in value:
        return "codex"
    if value == "opus" or value.startswith("claude-opus"):
        return "opus"
    if value == "sonnet" or value.startswith("claude-sonnet"):
        return "sonnet"
    if value == "haiku" or value.startswith("claude-haiku"):
        return "haiku"
    if value.startswith("claude"):
        return "claude"
    return value


def usage_model(raw):
    value = str(raw or "").strip().lower()
    if value.startswith(("codex", "gpt-")) or "codex" in value:
        return "codex"
    if value == "opus" or value.startswith("claude-opus"):
        return "opus"
    if value == "sonnet" or value.startswith("claude-sonnet"):
        return "sonnet"
    if value == "haiku" or value.startswith("claude-haiku"):
        return "haiku"
    if value.startswith("claude"):
        return "claude"
    return ""


def parse_meta(path):
    result = {}
    with path.open(encoding="utf-8") as handle:
        for line in handle:
            if "=" in line:
                key, value = line.rstrip("\n").split("=", 1)
                result[key] = value
    result["_path"] = str(path)
    result["_task_id"] = result.get("id", path.parent.name)
    return result


def outcome(meta):
    state = str(meta.get("state", "")).strip().lower()
    exit_code = integer(meta.get("exit", 0), 0)
    if state in {"needs-iteration", "revised"}:
        return "revised"
    if state == "ok" and exit_code == 0:
        return "accepted"
    return "rejected"


def read_ledger(path):
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


def gate_failures(rows):
    failures = defaultdict(list)
    for row in rows:
        if str(row.get("event", "")).upper() != "FAIL":
            continue
        if integer(row.get("exit_code", 1), 1) == 0:
            continue
        task_id = str(row.get("task_id", ""))
        if task_id:
            failures[task_id].append(str(row.get("ts", "")))
    return failures


def later_gate_failed(failures, task_id, ended):
    for timestamp in failures.get(task_id, []):
        if timestamp and (not ended or timestamp > ended):
            return True
    return False


def load_runs(path):
    metas = sorted(path.glob("*/meta.txt")) if path.is_dir() else []
    runs = []
    unattributed = 0
    for meta_path in metas:
        meta = parse_meta(meta_path)
        model = model_name(meta.get("model", "") or meta.get("harness", ""))
        role = str(meta.get("role", "")).strip().lower()
        if not model or not role:
            unattributed += 1
            continue
        runs.append({
            "model": model,
            "role": role,
            "task_id": meta["_task_id"],
            "ended": str(meta.get("ended", "")),
            "outcome": outcome(meta),
        })
    return runs, len(metas), unattributed


def ccusage_snapshot(path, skip):
    if path:
        try:
            return json.loads(Path(path).read_text(encoding="utf-8")), "fixture"
        except (OSError, json.JSONDecodeError):
            return {}, "unparseable"
    if skip:
        return {}, "skipped"
    try:
        completed = subprocess.run(
            ["ccusage", "daily", "--json"],
            capture_output=True,
            text=True,
            timeout=45,
            check=False,
        )
        if completed.returncode != 0:
            return {}, "failed"
        return json.loads(completed.stdout), "live"
    except (OSError, subprocess.TimeoutExpired, json.JSONDecodeError):
        return {}, "unavailable"


def ccusage_cost(snapshot):
    by_model = defaultdict(Decimal)
    periods = []
    for day in snapshot.get("daily", []):
        if day.get("period"):
            periods.append(str(day["period"]))
        for item in day.get("modelBreakdowns", []):
            model = usage_model(item.get("modelName", ""))
            if not model:
                continue
            try:
                by_model[model] += Decimal(str(item.get("cost", 0)))
            except (InvalidOperation, TypeError, ValueError):
                continue
    micros = {}
    for model, dollars in by_model.items():
        micros[model] = int((dollars * Decimal(1000000)).quantize(Decimal("1"), rounding=ROUND_HALF_UP))
    return micros, sorted(periods)


def usd(micros):
    if micros == "unknown":
        return micros
    amount = (Decimal(micros) / Decimal(1000000)).quantize(Decimal("0.000001"), rounding=ROUND_HALF_UP)
    return format(amount, "f")


def aggregate(runs, failures, usage_micros, min_sample):
    grouped = defaultdict(list)
    accepted_by_model = defaultdict(int)
    for run in runs:
        run["later_gate_failed"] = later_gate_failed(failures, run["task_id"], run["ended"])
        grouped[(run["model"], run["role"])].append(run)
        if run["outcome"] == "accepted":
            accepted_by_model[run["model"]] += 1

    rows = []
    for (model, role), values in sorted(grouped.items()):
        accepted = sum(value["outcome"] == "accepted" for value in values)
        revised = sum(value["outcome"] == "revised" for value in values)
        rejected = sum(value["outcome"] == "rejected" for value in values)
        gate_failed = sum(value["later_gate_failed"] for value in values)
        sample = len(values)
        model_cost = usage_micros.get(model, "unknown")
        cost = "unknown"
        if model_cost != "unknown" and accepted_by_model[model] > 0:
            cost = model_cost // accepted_by_model[model]
        if sample < min_sample:
            verdict = "insufficient-data"
        elif accepted == 0:
            verdict = "no-accepted-change"
        elif cost == "unknown":
            verdict = "cost-unknown"
        else:
            verdict = "rankable"
        rows.append({
            "model": model,
            "role": role,
            "accepted": accepted,
            "revised": revised,
            "rejected": rejected,
            "later_gate_failed": "yes" if gate_failed else "no",
            "later_gate_failed_count": gate_failed,
            "sample_size": sample,
            "cost_per_accepted_change_micro_usd": cost,
            "cost_per_accepted_change_usd": usd(cost),
            "verdict": verdict,
        })
    return rows


def report(args):
    runs_path = Path(args.runs or os.environ.get("FLEET_WORK", ROOT / "var" / "runs"))
    ledger_path = Path(args.ledger or os.environ.get("FLEET_LEDGER", ROOT / "ledger" / "RECEIPTS.jsonl"))
    min_sample = args.min_sample
    rows, parse_errors, ledger_state = read_ledger(ledger_path)
    if parse_errors:
        print("reputation: receipt ledger has unparseable rows", file=sys.stderr)
        return 4
    runs, run_count, unattributed = load_runs(runs_path)
    failures = gate_failures(rows)
    snapshot, ccusage_state = ccusage_snapshot(args.ccusage_json, args.no_ccusage)
    usage_micros, periods = ccusage_cost(snapshot)
    performance = aggregate(runs, failures, usage_micros, min_sample)
    payload = {
        "ok": True,
        "min_sample_size": min_sample,
        "metadata": {
            "runs_path": str(runs_path),
            "run_records": run_count,
            "scored_records": len(runs),
            "unattributed_records": unattributed,
            "ledger": str(ledger_path),
            "ledger_rows": len(rows),
            "ledger_state": ledger_state,
            "ccusage": ccusage_state,
            "ccusage_periods": periods,
            "ccusage_by_model_micro_usd": usage_micros,
            "cost_allocation": "ccusage model cost divided by accepted runs for that model; role-specific cost is not derivable from per-model transcripts",
        },
        "reputation": performance,
    }
    if args.json:
        print(json.dumps(payload, sort_keys=True))
        return 0
    print("reputation_min_sample_size=" + str(min_sample))
    print("source_runs=%d scored_records=%d unattributed_records=%d ledger_rows=%d ccusage=%s" % (
        run_count, len(runs), unattributed, len(rows), ccusage_state
    ))
    print("model\trole\taccepted\trevised\trejected\tlater_gate_failed\tsample_size\tcost_per_accepted_change_usd\tverdict")
    for row in performance:
        print("\t".join(str(row[field]) for field in (
            "model", "role", "accepted", "revised", "rejected", "later_gate_failed",
            "sample_size", "cost_per_accepted_change_usd", "verdict"
        )))
    return 0


def parser():
    root = argparse.ArgumentParser(prog="reputation.sh")
    sub = root.add_subparsers(dest="command")
    report_parser = sub.add_parser("report")
    report_parser.add_argument("--runs")
    report_parser.add_argument("--ledger")
    report_parser.add_argument("--ccusage-json")
    report_parser.add_argument("--no-ccusage", action="store_true")
    report_parser.add_argument("--min-sample", type=int, default=int(os.environ.get("FLEET_REPUTATION_MIN_SAMPLE", DEFAULT_MIN_SAMPLE)))
    report_parser.add_argument("--json", action="store_true")
    return root


args = parser().parse_args()
if args.command != "report":
    parser().print_help(sys.stderr)
    raise SystemExit(2)
if args.min_sample < 1:
    print("reputation: --min-sample must be positive", file=sys.stderr)
    raise SystemExit(2)
raise SystemExit(report(args))
PY
