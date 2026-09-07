"""F09 -- the orb-side half of the freeze -> SOW handoff (lane contract
docs/lane-contracts/F09-orb-fleet-handoff.md §5.3).

Two real subprocess calls to the compiled `fleet` binary -- `fleet freeze stamp <brief> --out
<lld>` then `fleet sow --lld <lld>` -- never a third Python implementation of either step, and
never a shortcut that skips the first call and hands the second one a hand-built document. This
module does not construct an `lld_schemas.Freeze`, `LldV1`, or `SowSeed`, and never spells out the
gate's own stamp constant (`lld_ready::STAMPED_BY` in Rust, mirrored as `Freeze.stamped_by`'s
`Literal` in Python) as a string of its own: the orb never authors, holds, or transmits a stamp
(blueprint `03` §3.4/§2.3; lane contract §1/§4.1). `tests/test_f09_handoff.py`'s AST check
(F09-T19) and the cross-tree census (F09-T12) both hold this file to that line.

This module duplicates `readiness.py`'s ~15-line tempfile+subprocess shape rather than importing
or extending it, on purpose: `ReadinessGate.evaluate` is a PURE CONSULTATION of the gate (it must
never mutate keel's ledger), while a stamp+intake round trip does mutate it (a ledger entry, a SOW
record) -- folding the two into one seam would make every readiness consultation a potential write.
`readiness.py`'s own `ReadinessGate` Protocol is not touched or widened here: `UnavailableReadinessGate`
and every test fake that implements it would break if it grew a method this lane does not need.

Only the documented success code is ever a pass -- stamp exit 0, `sow --lld` exit 9 -- mirroring
`readiness.py`'s own rule 1 exactly. `stdout`/`stderr` are captured verbatim and never parsed for
AUTHORITY: `freeze_id`/`sow_id`/`version` are read out of them only as IDENTIFIERS to echo and to
name files with (lane contract §5.3 rule 2); the decision that the handoff succeeded rests on the
two exit codes alone. `UNAVAILABLE` refuses -- never "assume stamped", never skipped, never
silently downgraded (rule 3; `UnavailableHandoff` below mirrors `UnavailableReadinessGate` exactly).
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

_EXIT_STAMP_OK = 0
_EXIT_SOW_READY = 9

DEFAULT_TIMEOUT_SECONDS = 10.0


class HandoffOutcome(str, Enum):
    SOW_READY = "sow_ready"  # stamp exit 0 AND sow --lld exit 9
    NOT_STAMPED = "not_stamped"  # stamp refused (any non-zero)
    NOT_ACCEPTED = "not_accepted"  # stamp ok, sow --lld refused (any exit != 9)
    UNAVAILABLE = "unavailable"  # FLEET_BIN unset / not executable / FileNotFoundError
    TIMEOUT = "timeout"  # either subprocess exceeded its budget


@dataclass(frozen=True, slots=True)
class HandoffResult:
    outcome: HandoffOutcome
    sow_id: str | None
    freeze_id: str | None
    node_id: str
    freeze_version: int | None
    detail: str

    def is_pass(self) -> bool:
        return self.outcome is HandoffOutcome.SOW_READY


class Handoff(Protocol):
    def hand_off(self, brief: ModuleBrief) -> HandoffResult: ...


def _field(text: str, prefix: str, key: str) -> str | None:
    """Read one `key=value` token off the first line starting with `prefix`. An identifier to
    echo, never a decision input (rule 2 above) -- independently reimplements
    `fleet/keel/fleet/tests/f07_sow_intake.rs`'s own `field()` helper in Python, matching this
    estate's convention of no shared test/helper module across languages.
    """
    for line in text.splitlines():
        if not line.startswith(prefix):
            continue
        for token in line.split():
            found_key, sep, value = token.partition("=")
            if sep and found_key == key:
                return value
    return None


@dataclass(frozen=True, slots=True)
class _Unavailable:
    detail: str


class FleetHandoff:
    """Authority: keel, twice. `hand_off` never raises -- every failure mode (a bad exit code, a
    timeout, a missing or unexecutable binary) is mapped to a `HandoffResult`, never an exception.
    """

    def __init__(self, fleet_bin: str | Path, *, timeout_s: float = DEFAULT_TIMEOUT_SECONDS) -> None:
        self._fleet_bin = str(fleet_bin)
        self._timeout_s = timeout_s

    def hand_off(self, brief: ModuleBrief) -> HandoffResult:
        node_id = brief.node_id
        # Byte-identical call to readiness.py's own `evaluate()` (lane contract §3.8) -- any
        # divergence (adding exclude_none=True, dumping something other than the bare brief) risks
        # the stamp shape-rejecting a brief the gate just accepted.
        payload = brief.model_dump(mode="json")

        with tempfile.TemporaryDirectory() as tmp_dir:
            brief_path = Path(tmp_dir) / "brief.json"
            lld_path = Path(tmp_dir) / "lld.json"
            with brief_path.open("w", encoding="utf-8") as handle:
                json.dump(payload, handle)

            stamp = self._run(["freeze", "stamp", str(brief_path), "--out", str(lld_path)])
            if stamp is None:
                return HandoffResult(HandoffOutcome.TIMEOUT, None, None, node_id, None, "fleet freeze stamp timed out")
            if isinstance(stamp, _Unavailable):
                return HandoffResult(HandoffOutcome.UNAVAILABLE, None, None, node_id, None, stamp.detail)
            if stamp.returncode != _EXIT_STAMP_OK:
                detail = (stamp.stdout + "\n" + stamp.stderr).strip()
                return HandoffResult(HandoffOutcome.NOT_STAMPED, None, None, node_id, None, detail)

            stamped_freeze_id = _field(stamp.stdout, "FREEZE_STAMPED", "freeze_id")
            version_token = _field(stamp.stdout, "FREEZE_STAMPED", "version")
            stamped_version = int(version_token) if version_token is not None and version_token.isdigit() else None

            sow = self._run(["sow", "--lld", str(lld_path)])
            if sow is None:
                return HandoffResult(
                    HandoffOutcome.TIMEOUT, None, stamped_freeze_id, node_id, stamped_version, "fleet sow --lld timed out"
                )
            if isinstance(sow, _Unavailable):
                return HandoffResult(
                    HandoffOutcome.UNAVAILABLE, None, stamped_freeze_id, node_id, stamped_version, sow.detail
                )
            if sow.returncode != _EXIT_SOW_READY:
                detail = (sow.stdout + "\n" + sow.stderr).strip()
                return HandoffResult(
                    HandoffOutcome.NOT_ACCEPTED, None, stamped_freeze_id, node_id, stamped_version, detail
                )

            sow_id = _field(sow.stderr, "SOW_READY_AWAITING_REVIEW", "id")
            return HandoffResult(
                HandoffOutcome.SOW_READY,
                sow_id,
                stamped_freeze_id,
                node_id,
                stamped_version,
                f"stamped {stamped_freeze_id} v{stamped_version}; sow {sow_id}",
            )

    def _run(self, args: list[str]) -> subprocess.CompletedProcess[str] | _Unavailable | None:
        """`None` means the call timed out; `_Unavailable` means the binary could not even start."""
        try:
            return subprocess.run(
                [self._fleet_bin, *args],
                capture_output=True,
                text=True,
                timeout=self._timeout_s,
                check=False,
            )
        except subprocess.TimeoutExpired:
            return None
        except (FileNotFoundError, PermissionError, OSError) as exc:
            return _Unavailable(detail=str(exc))


class UnavailableHandoff:
    """FLEET_BIN unset or not executable. ALWAYS returns UNAVAILABLE -- never SOW_READY (rule 3;
    mirrors `UnavailableReadinessGate`'s own fail-closed shape).
    """

    def hand_off(self, brief: ModuleBrief) -> HandoffResult:
        return HandoffResult(HandoffOutcome.UNAVAILABLE, None, None, brief.node_id, None, "FLEET_BIN not set")
