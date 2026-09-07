"""F05 T5 -- the keel seam, against the REAL `fleet gate lld-ready` binary (lane contract §5 T5).

Not a mock: builds against `cargo build --manifest-path fleet/keel/Cargo.toml`'s real output. Skips
(never fails, never silently counts as a pass) if the binary is genuinely absent -- §7's "a skip is
recorded as a skip, never counted as a pass."

Written by this lane's builder from the lane contract's own §4.2/§4.2a/§5 T5 tables (no test file
existed for F05 before this build; §5's header: "write these FIRST; they are the spec"). Frozen
once green -- do not edit further without re-reading the contract.
"""

from __future__ import annotations

import ast
import json
from pathlib import Path

import pytest

from orb_relay.build import readiness as readiness_module
from orb_relay.build.readiness import (
    ReadinessOutcome,
    SubprocessReadinessGate,
    UnavailableReadinessGate,
)
from orb_relay.proxy.lld_schemas import validate_module_brief

_RELAY_PY_ROOT = Path(__file__).resolve().parents[1]  # orb/backend/relay-py
_WORKTREE_ROOT = _RELAY_PY_ROOT.parents[2]  # .../Light/.worktrees/F05-freeze-protocol
_FIXTURES_DIR = _WORKTREE_ROOT / "fleet" / "contracts" / "fixtures" / "lld"
_FLEET_BIN = _WORKTREE_ROOT / "fleet" / "keel" / "target" / "debug" / "fleet"

# The 14 check ids, transcribed by hand from `fleet/keel/fleet/src/lld_ready.rs`'s own
# `GATE_CHECK_IDS` (T5.7 must not import that constant -- an import would be a dependency on the
# very rubric this seam must hold zero copies of; a hand-transcribed literal here, in the TEST, is
# the right side of that line -- `readiness.py` itself must contain none of them, per T5.7 below).
_GATE_CHECK_IDS = (
    "C1-OPEN", "C2-OWNER", "C3-ACC-PARSE", "C3-ACC-GROUND", "C3-ACC-NONTAUT",
    "R17-DERIV", "R19-ABSOLUTE", "R21-ALTS", "R21-FAIL", "C12-STORE", "C12-DEPS",
    "REG-VERDICT", "IFACE", "SHAPE",
)


def _skip_if_binary_missing() -> None:
    if not _FLEET_BIN.exists():
        pytest.skip("fleet binary not built (cargo build --manifest-path fleet/keel/Cargo.toml)")


def _load_complete_module_brief():
    raw = json.loads((_FIXTURES_DIR / "complete_module.json").read_text(encoding="utf-8"))
    brief = validate_module_brief(raw)
    assert brief is not None, "complete_module.json must itself validate as a ModuleBrief"
    return brief


def test_t5_1_ready_on_the_real_control_fixture() -> None:
    _skip_if_binary_missing()
    gate = SubprocessReadinessGate(_FLEET_BIN)
    verdict = gate.evaluate(_load_complete_module_brief())
    assert verdict.outcome is ReadinessOutcome.READY
    assert verdict.exit_code == 0
    assert "READY" in verdict.stdout
    assert "14/14" in verdict.stdout
    assert verdict.is_pass() is True


def test_t5_2_not_ready_requires_a_shape_valid_gate_invalid_brief() -> None:
    """§4.2a: no checked-in fixture reaches exit 6 -- every shape-invalid fixture is caught by the
    shape layer first, at exit 8. Derive a shape-VALID, gate-INVALID brief by mutating a PARSED
    COPY of the control fixture in memory (never a fixture file on disk): `owner` must resolve in
    `fleet/contracts/owners.v1.json`, which holds exactly `["rachit@devxlabs.ai"]`, so any other
    string fails the gate's owner-resolution check while leaving every OTHER field, and therefore
    every shape constraint, untouched.
    """
    _skip_if_binary_missing()
    brief = _load_complete_module_brief().model_copy(update={"owner": "nobody-at-all"})
    gate = SubprocessReadinessGate(_FLEET_BIN)
    verdict = gate.evaluate(brief)
    assert verdict.outcome is ReadinessOutcome.NOT_READY
    assert verdict.exit_code == 6
    assert "NOT_READY" in verdict.stdout
    assert "C2-OWNER" in verdict.stderr
    assert verdict.is_pass() is False


def test_t5_3_exit_8_shape_invalid_does_not_reach_the_gate() -> None:
    """§4.2a: `one_line_freeze.json` fails PYTHON's own `validate_module_brief` too (it is missing
    `schema_version` entirely), so no real `ModuleBrief` instance can be constructed from it at
    all -- confirmed directly below, which is itself part of what this case pins: exit 8 is
    unreachable from the public, typed `evaluate(ModuleBrief)` entry point in production, exactly
    because upstream (the decomposer) never hands this seam anything but an already-valid brief.
    Exercised here via `_evaluate_payload`, the raw-payload seam `evaluate()` wraps (readiness.py's
    own docstring names why it exists), with the fixture's own bytes, unmodified, on disk.
    """
    _skip_if_binary_missing()
    raw = json.loads((_FIXTURES_DIR / "one_line_freeze.json").read_text(encoding="utf-8"))
    assert validate_module_brief(raw) is None, "fixture must ALSO be pydantic-invalid, or this case is not testing what it claims"

    gate = SubprocessReadinessGate(_FLEET_BIN)
    verdict = gate._evaluate_payload(raw)  # noqa: SLF001 -- deliberate, see docstring above
    assert verdict.outcome is ReadinessOutcome.NOT_READY
    assert verdict.exit_code == 8
    assert "SHAPE INVALID" in verdict.stdout
    assert verdict.is_pass() is False


def test_t5_4_unavailable_gate_never_passes() -> None:
    """A gate that cannot run must refuse, not assume."""
    verdict = UnavailableReadinessGate().evaluate(_load_complete_module_brief())
    assert verdict.outcome is ReadinessOutcome.UNAVAILABLE
    assert verdict.is_pass() is False


def test_t5_5_missing_binary_is_unavailable_not_an_exception() -> None:
    gate = SubprocessReadinessGate("/nonexistent/fleet")
    verdict = gate.evaluate(_load_complete_module_brief())  # must not raise
    assert verdict.outcome is ReadinessOutcome.UNAVAILABLE
    assert verdict.is_pass() is False


def test_t5_6_no_regex_or_split_over_stdout_stderr() -> None:
    """AST scan, not a substring grep over the file's prose (its own docstring discusses `re.` and
    `.split(` as concepts) -- looks for an actual `import re` / `re.<attr>` reference, or an actual
    `.split(` method CALL, anywhere in the module's real code.
    """
    source = (Path(readiness_module.__file__)).read_text(encoding="utf-8")
    tree = ast.parse(source)

    imports_re = any(
        (isinstance(node, ast.Import) and any(alias.name == "re" for alias in node.names))
        or (isinstance(node, ast.ImportFrom) and node.module == "re")
        for node in ast.walk(tree)
    )
    uses_re_attr = any(
        isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name) and node.value.id == "re"
        for node in ast.walk(tree)
    )
    calls_split = any(
        isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and node.func.attr == "split"
        for node in ast.walk(tree)
    )
    assert not imports_re
    assert not uses_re_attr
    assert not calls_split


def test_t5_7_readiness_module_holds_no_copy_of_the_rubric() -> None:
    source = Path(readiness_module.__file__).read_text(encoding="utf-8")
    hits = [check_id for check_id in _GATE_CHECK_IDS if check_id in source]
    assert hits == [], f"readiness.py must hold zero copies of the gate's own check ids; found {hits}"
