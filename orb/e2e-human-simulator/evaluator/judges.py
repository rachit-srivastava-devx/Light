"""Optional local judge adapters.

Judges receive a bounded, redacted evidence payload only. They never receive
an app command, simulator handle, microphone input, or a tool-driving prompt.
Missing CLIs are normal and become an explicit report entry.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import os
from pathlib import Path
import shutil
import subprocess
from typing import Any, Protocol

from .checks import Evaluation


class JudgeAdapter(Protocol):
    name: str

    def available(self) -> bool: ...

    def evaluate(self, evidence: dict[str, Any]) -> "JudgeResult": ...


@dataclass(frozen=True)
class JudgeResult:
    judge: str
    status: str
    summary: str = ""
    score: float | None = None
    findings: tuple[str, ...] = ()
    raw_output: str = ""


def _prompt(evidence: dict[str, Any]) -> str:
    payload = json.dumps(evidence, ensure_ascii=False, sort_keys=True)
    return """You are an evidence reviewer for a voice assistant human-simulation run.
Review ONLY the captured JSON evidence below. Do not run commands, open the app,
call providers, infer unlogged behavior, or propose that you drive the simulator.
Return JSON only: {\"score\": 0-10, \"summary\": string, \"findings\": [string]}.
Judge response relevance, conversational quality, understandability signals,
latency perception, state coherence, repetition, failure handling, and Fish markup.
Be specific about evidence lines/sequences when possible.

CAPTURED_EVIDENCE:
""" + payload


def _parse_result(judge: str, output: str) -> JudgeResult:
    candidate = output.strip()
    if "```" in candidate:
        candidate = candidate.replace("```json", "").replace("```", "").strip()
    try:
        parsed = json.loads(candidate)
    except json.JSONDecodeError:
        start, end = candidate.find("{"), candidate.rfind("}")
        if start < 0 or end <= start:
            return JudgeResult(judge, "invalid_output", summary="Judge did not return JSON.", raw_output=output[-4000:])
        try:
            parsed = json.loads(candidate[start : end + 1])
        except json.JSONDecodeError:
            return JudgeResult(judge, "invalid_output", summary="Judge returned malformed JSON.", raw_output=output[-4000:])
    if not isinstance(parsed, dict):
        return JudgeResult(judge, "invalid_output", summary="Judge JSON was not an object.", raw_output=output[-4000:])
    score = parsed.get("score")
    score = float(score) if isinstance(score, (int, float)) else None
    findings = parsed.get("findings", [])
    if not isinstance(findings, list):
        findings = [str(findings)]
    return JudgeResult(judge, "ok", str(parsed.get("summary", "")).strip(), score, tuple(str(item) for item in findings), output[-4000:])


class CommandJudgeAdapter:
    def __init__(self, name: str, command: str, args: tuple[str, ...], timeout_s: int = 60, cwd: str | Path | None = None):
        self.name = name
        self.command = command
        self.args = args
        self.timeout_s = timeout_s
        # Keep the judge's working directory inside this independent folder.
        # The judge is never given the app checkout as its working root.
        self.cwd = Path(cwd) if cwd is not None else Path(__file__).resolve().parent

    def available(self) -> bool:
        # shutil.which supports both PATH names and absolute executable paths,
        # but the explicit file check makes the absolute-path contract clear.
        if Path(self.command).is_absolute():
            return Path(self.command).is_file() and os.access(self.command, os.X_OK)
        return shutil.which(self.command) is not None

    def evaluate(self, evidence: dict[str, Any]) -> JudgeResult:
        if not self.available():
            return JudgeResult(self.name, "unavailable", summary=f"{self.command!r} CLI is not installed or not on PATH.")
        prompt = _prompt(evidence)
        try:
            completed = subprocess.run(
                [self.command, *self.args],
                input=prompt,
                text=True,
                capture_output=True,
                timeout=self.timeout_s,
                check=False,
                shell=False,
                cwd=self.cwd,
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            return JudgeResult(self.name, "error", summary=f"Could not run {self.command}: {exc}")
        output = completed.stdout.strip() or completed.stderr.strip()
        if completed.returncode != 0:
            return JudgeResult(self.name, "error", summary=f"{self.command} exited with code {completed.returncode}.", raw_output=output[-4000:])
        return _parse_result(self.name, output)


class CodexJudgeAdapter(CommandJudgeAdapter):
    def __init__(self, command: str | None = None, args: tuple[str, ...] | None = None):
        super().__init__("codex", command or os.environ.get("E2E_CODEX_BIN", "codex"), args or ("exec", "--sandbox", "read-only", "--ephemeral", "--skip-git-repo-check", "-"))


class ClaudeJudgeAdapter(CommandJudgeAdapter):
    def __init__(self, command: str | None = None, args: tuple[str, ...] | None = None):
        super().__init__("claude", command or os.environ.get("E2E_CLAUDE_BIN", "claude"), args or ("-p",))


def run_judges(evaluation: Evaluation, adapters: list[JudgeAdapter] | None = None) -> list[JudgeResult]:
    adapters = adapters or [CodexJudgeAdapter(), ClaudeJudgeAdapter()]
    evidence = evaluation.trace.evidence()
    evidence["deterministic_findings"] = [finding.__dict__ for finding in evaluation.findings]
    return [adapter.evaluate(evidence) for adapter in adapters]
