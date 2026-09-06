# backend/relay-py/tests/test_lld_schema_conformance.py
import json, pathlib, pytest
from orb_relay.proxy.lld_schemas import validate_module_brief

FIX = pathlib.Path(__file__).parents[3] / "apps/mobile/src/build/fixtures"

@pytest.mark.parametrize("name,expected_ok", [("complete_module", True), ("one_line_freeze", False)])
def test_u1_t6_py_agrees_with_ts_on_every_fixture(name, expected_ok):
    """The cross-language mirror that shared/atomizer-schema.ts's header warns about.
    A drift here means the client would accept a brief the relay rejects."""
    brief = json.loads((FIX / f"{name}.json").read_text())
    assert (validate_module_brief(brief) is not None) is expected_ok
