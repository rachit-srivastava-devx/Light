"""Human-readable Markdown report generator."""

from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path

from .checks import Evaluation, Finding
from .judges import JudgeResult


def _finding_markdown(finding: Finding) -> str:
    evidence = "; ".join(finding.evidence) or "no event evidence"
    suggestion = f" Suggestion: {finding.suggestion}" if finding.suggestion else ""
    return f"- **{finding.severity.upper()} `{finding.code}`** — {finding.message} Evidence: {evidence}.{suggestion}"


def render_report(evaluation: Evaluation, judges: list[JudgeResult] | None = None) -> str:
    counts = evaluation.counts()
    lines = [
        "# Human-Simulation Evidence Report",
        "",
        f"Generated: {datetime.now(timezone.utc).isoformat()}",
        f"Trace: `{evaluation.trace.source_path or 'in-memory'}`",
        f"Verdict: **{'PASS' if evaluation.passed else 'FAIL'}** — {counts['critical']} critical, {counts['error']} errors, {counts['warn']} warnings",
        "",
        "## What felt wrong",
        "",
    ]
    felt_wrong = [finding for finding in evaluation.findings if finding.severity in {"critical", "error", "warn"}]
    if felt_wrong:
        lines.extend(_finding_markdown(finding) for finding in felt_wrong)
    else:
        lines.append("- No deterministic friction was found in the captured evidence.")
    lines.extend(["", "## Evidence", ""])
    if evaluation.trace.schema_errors:
        lines.extend(f"- Schema: {error}" for error in evaluation.trace.schema_errors)
    lines.append(f"- Valid events captured: {len(evaluation.trace.events)}")
    lines.append(f"- Sources: {', '.join(sorted({event.source for event in evaluation.trace.events})) or 'none'}")
    lines.extend(["", "## Deterministic checks", ""])
    for check in evaluation.checks:
        status = "PASS" if check.passed else "FAIL"
        lines.append(f"- **{status}** `{check.name}` — {len(check.findings)} finding(s)")
    lines.extend(["", "## Severity and suggestions", ""])
    if evaluation.findings:
        lines.extend(_finding_markdown(finding) for finding in evaluation.findings)
    else:
        lines.append("- No suggestions; keep the trace contract enabled for future runs.")
    lines.extend(["", "## Optional judge review", ""])
    if not judges:
        lines.append("- Not run. Use `--judges` to invoke local Codex/Claude over captured evidence only.")
    else:
        for judge in judges:
            score = f" score={judge.score:g}/10" if judge.score is not None else ""
            lines.append(f"- **{judge.judge}** — `{judge.status}`{score}: {judge.summary or 'no summary'}")
            for finding in judge.findings:
                lines.append(f"  - {finding}")
            if judge.raw_output and judge.status != "ok":
                compact = " ".join(judge.raw_output.split())[-1200:]
                lines.append(f"  - Judge output: `{compact}`")
    lines.extend(["", "## Trace contract notes", "", "- This report is evidence-only; it does not launch, control, or modify the app.", "- Missing judge CLIs are reported as `unavailable` and do not invalidate deterministic checks.", ""])
    return "\n".join(lines)


def write_report(path: str | Path, evaluation: Evaluation, judges: list[JudgeResult] | None = None) -> None:
    Path(path).write_text(render_report(evaluation, judges), encoding="utf-8")
