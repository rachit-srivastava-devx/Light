# backend/relay-py/tests/test_lld_schema_conformance.py
import json
import os
import pathlib

import pytest

from orb_relay.proxy.lld_schemas import (
    AcceptanceLine,
    DataDecl,
    DepthScore,
    FailureStory,
    Freeze,
    Guarantee,
    InterfaceDecl,
    KilledAlt,
    LldV1,
    ModuleBrief,
    canonical_json,
    content_hash,
    cross_lang_report,
    validate_lld_v1_detailed,
    validate_module_brief,
    validate_module_brief_detailed,
)

# F02: fixtures moved fleet-ward (schema ownership moved with them -- fleet is the lld-ready
# gate's owner, F02 lane contract §3.1). __file__ = orb/backend/relay-py/tests/<this file>;
# parents[3] = orb/, parents[4] = the repo root (Light/, sibling of fleet/).
FIX = pathlib.Path(__file__).parents[4] / "fleet/contracts/fixtures/lld"


@pytest.mark.parametrize("name,expected_ok", [("complete_module", True), ("one_line_freeze", False)])
def test_u1_t6_py_agrees_with_ts_on_every_fixture(name, expected_ok):
    """The cross-language mirror that shared/atomizer-schema.ts's header warns about.
    A drift here means the client would accept a brief the relay rejects."""
    brief = json.loads((FIX / f"{name}.json").read_text())
    assert (validate_module_brief(brief) is not None) is expected_ok


def test_f02_t6_rejects_a_leaf_grain_brief_with_only_one_killed_alternative():
    raw = json.loads((FIX / "one_alternative.json").read_text())
    r = validate_module_brief_detailed(raw)
    assert r.ok is False
    assert "alternatives" in {e.path for e in r.errors}


def test_f02_t7_rejects_a_freeze_stamped_by_the_proposer():
    raw = json.loads((FIX / "forged_freeze.json").read_text())
    r = validate_lld_v1_detailed(raw)
    assert r.ok is False
    assert "freeze.stamped_by" in {e.path for e in r.errors}


def test_f02_t9_canonical_json_refuses_a_numeric_leaf():
    with pytest.raises(TypeError, match="numbers are not canonicalizable"):
        canonical_json({"ratio": 1.0})
    numeric_trap = json.loads((FIX / "numeric_trap.json").read_text())
    with pytest.raises(TypeError, match="numbers are not canonicalizable"):
        content_hash(numeric_trap)
    # and the guard is not vacuous: the good fixture still hashes.
    complete_module = json.loads((FIX / "complete_module.json").read_text())
    assert content_hash(complete_module).startswith("sha256:")
    assert len(content_hash(complete_module)) == len("sha256:") + 64


def test_f02_t10_content_hash_carries_its_algorithm():
    complete_module = json.loads((FIX / "complete_module.json").read_text())
    h = content_hash(complete_module)
    assert h.startswith("sha256:")
    assert len(h) == len("sha256:") + 64
    assert all(c in "0123456789abcdef" for c in h[len("sha256:"):])


def test_f02_t11_a_freeze_self_verifies_against_its_own_brief():
    w = json.loads((FIX / "complete_freeze.json").read_text())
    assert w["freeze"]["content_hash"] == content_hash(w["module_brief"])


def test_f02_t12_emits_the_cross_language_report(tmp_path):
    # ensure_ascii=False matches JSON.stringify's default (never \u-escapes printable non-ASCII,
    # e.g. this contract's own "§" section signs) -- ensure_ascii's True default would make this
    # mirror's report byte-diverge from the TS/Rust mirrors' reports on the identical content. No
    # trailing newline is appended: the frozen TS test (lld-v1.test.ts F02-T12) writes
    # `JSON.stringify(x, null, 2)` verbatim with none, so this mirror matches that actual behaviour
    # rather than this section's own prose ("a trailing newline"), which the frozen test's code
    # does not follow -- see the F02 builder's final report for this disclosed discrepancy.
    out = os.environ.get("F02_REPORT_OUT") or str(tmp_path / "py.json")
    report = cross_lang_report(FIX)
    pathlib.Path(out).write_text(json.dumps(report, indent=2, ensure_ascii=False))


def test_f02_t13_pydantic_forbids_extra_on_every_nested_model():
    """extra='forbid' on the outer model does not imply it on nested ones.
    additionalProperties:false is on EVERY $defs entry in the schema; assert the mirror matches."""
    for model in (ModuleBrief, Freeze, LldV1, Guarantee, KilledAlt, FailureStory,
                  InterfaceDecl, DataDecl, AcceptanceLine, DepthScore):
        assert model.model_config.get("extra") == "forbid", model.__name__
