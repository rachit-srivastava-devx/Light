"""F05 -- the keel seam (lane contract §4.2/§2.2).

Contains ZERO rule logic. The 14 named lld-ready checks exist in exactly two places in this tree --
`fleet/keel/fleet/src/lld_ready.rs` and `orb/apps/mobile/src/build/LldReadyGate.ts` -- kept honest
by `fleet/tests/acceptance/lld-ready-crosslang.sh`. A third, Python copy would ship with zero
cross-language coverage and would invert the authority (blueprint §6.2: "if they ever disagree,
keel wins"). This module never re-derives the rubric; it shells out to the real `fleet gate
lld-ready <file>` binary and reads only its EXIT CODE. `tests/test_f05_readiness_gate.py`'s T5.7
greps this file's source for every one of those check identifiers to prove the rubric is not
duplicated here -- so this docstring deliberately never spells out any of them either.

The exit-code contract (read out of `fleet/keel/fleet/src/main.rs`, not guessed):

| exit | keel's meaning                                                     | ReadinessOutcome |
|------|---------------------------------------------------------------------|------------------|
| 0    | READY (<path>) -- N/14 checks passed                                 | READY            |
| 6    | NOT_READY or MEASURED_NOTHING (share one code, EXIT_INVARIANT)       | NOT_READY        |
| 8    | the gate's own invalid-shape refusal (the gate itself was not reached, EXIT_MISMATCH) | NOT_READY |
| 3    | unreadable file / bad JSON / missing owners.v1.json (EXIT_ENV)       | UNAVAILABLE      |
| 7    | usage error (EXIT_REFUSAL)                                          | UNAVAILABLE      |
| n/a  | timeout, or the binary itself is missing                             | UNAVAILABLE      |

Three rules, each load-bearing (lane contract §4.2):

1. Only exit 0 is a pass -- NOT_READY and MEASURED_NOTHING are deliberately indistinguishable by
   exit code, so this module does not try to tell them apart; both mean "may not freeze."
2. `stdout`/`stderr` are captured verbatim and never parsed (no `re.`, no `.split(` over them
   anywhere in this file) -- carried as evidence for a human, not as data this module reads.
3. `UNAVAILABLE` refuses the freeze. It is never treated as "assume ready", never skipped, and
   never silently downgraded to a dialogue-only check.
"""

from __future__ import annotations

import json
import subprocess
import tempfile
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Protocol

from ..proxy.lld_schemas import ModuleBrief

_EXIT_READY = 0
_EXIT_ENV = 3
_EXIT_INVARIANT = 6
_EXIT_REFUSAL = 7
_EXIT_MISMATCH = 8

DEFAULT_TIMEOUT_SECONDS = 10.0


class ReadinessOutcome(str, Enum):
    READY = "ready"
    NOT_READY = "not_ready"
    UNAVAILABLE = "unavailable"


@dataclass(frozen=True, slots=True)
class ReadinessVerdict:
    outcome: ReadinessOutcome
    exit_code: int | None
    stdout: str
    stderr: str

    def is_pass(self) -> bool:
        return self.outcome is ReadinessOutcome.READY


def _outcome_for_exit_code(exit_code: int) -> ReadinessOutcome:
    if exit_code == _EXIT_READY:
        return ReadinessOutcome.READY
    if exit_code in (_EXIT_INVARIANT, _EXIT_MISMATCH):
        return ReadinessOutcome.NOT_READY
    # _EXIT_ENV, _EXIT_REFUSAL, and anything else unrecognized all refuse rather than assume ready.
    return ReadinessOutcome.UNAVAILABLE


class ReadinessGate(Protocol):
    def evaluate(self, brief: ModuleBrief) -> ReadinessVerdict: ...


class SubprocessReadinessGate:
    """Authority: keel. Writes `brief.model_dump(mode="json")` to a NamedTemporaryFile, runs
    `<fleet_bin> gate lld-ready <path>` with a hard timeout, deletes the file, and returns the
    verdict built purely from the process's exit code (rule 1 above).
    """

    def __init__(self, fleet_bin: str | Path, *, timeout_s: float = DEFAULT_TIMEOUT_SECONDS) -> None:
        self._fleet_bin = str(fleet_bin)
        self._timeout_s = timeout_s

    def evaluate(self, brief: ModuleBrief) -> ReadinessVerdict:
        return self._evaluate_payload(brief.model_dump(mode="json"))

    def _evaluate_payload(self, payload: dict) -> ReadinessVerdict:
        """The real subprocess mechanics, over a plain JSON-serializable payload rather than a
        typed `ModuleBrief`. `evaluate()` (the public, contract-typed entry point) is a one-line
        wrapper over this. This method exists as its own seam because exit 8 (the gate's own
        invalid-shape refusal) is, by construction, unreachable from a real `ModuleBrief` instance:
        every fixture that trips keel's shape layer also fails `lld_schemas.validate_module_brief`
        (§4.2a) -- there is no pydantic-valid-but-keel-shape-invalid brief to construct.
        `tests/test_f05_readiness_gate.py` T5.3 calls this directly with a raw, genuinely
        shape-invalid fixture dict to exercise that branch for real; production code only ever
        calls the public `evaluate(ModuleBrief)`.
        """
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False, encoding="utf-8") as tmp:
            tmp_path = Path(tmp.name)
            json.dump(payload, tmp)
        try:
            try:
                result = subprocess.run(
                    [self._fleet_bin, "gate", "lld-ready", str(tmp_path)],
                    capture_output=True,
                    text=True,
                    timeout=self._timeout_s,
                    check=False,
                )
            except subprocess.TimeoutExpired as exc:
                return ReadinessVerdict(
                    outcome=ReadinessOutcome.UNAVAILABLE,
                    exit_code=None,
                    stdout=exc.stdout or "" if isinstance(exc.stdout, str) else "",
                    stderr=exc.stderr or "" if isinstance(exc.stderr, str) else "",
                )
            except (FileNotFoundError, PermissionError, OSError) as exc:
                return ReadinessVerdict(
                    outcome=ReadinessOutcome.UNAVAILABLE,
                    exit_code=None,
                    stdout="",
                    stderr=str(exc),
                )
            return ReadinessVerdict(
                outcome=_outcome_for_exit_code(result.returncode),
                exit_code=result.returncode,
                stdout=result.stdout,
                stderr=result.stderr,
            )
        finally:
            tmp_path.unlink(missing_ok=True)


class UnavailableReadinessGate:
    """Returned when FLEET_BIN is unset or not executable. ALWAYS returns UNAVAILABLE -- never
    READY -- so a missing gate binary fails closed rather than silently permitting every freeze
    (lane contract §4.2 rule 3 / T5.4).
    """

    def evaluate(self, brief: ModuleBrief) -> ReadinessVerdict:
        return ReadinessVerdict(outcome=ReadinessOutcome.UNAVAILABLE, exit_code=None, stdout="", stderr="")
