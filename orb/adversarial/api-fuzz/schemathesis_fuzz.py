"""H2-2: API fuzzing of the relay HTTP surface with schemathesis (Apache-2.0), Track H2 (adversarial).

Black box: imports orb_relay.app read-only, never edits backend/relay-py. Zero network egress and
zero provider cost -- the `get_gateway` FastAPI dependency is overridden with a fake before the
schema is even derived, and the schema itself is DERIVED from FastAPI's own generated OpenAPI
(`schemathesis.openapi.from_asgi("/openapi.json", app)`), not hand-written, per the H2 brief
("Use schemathesis if an OpenAPI/JSON schema exists or can be derived").

Run:
    cd "<repo>" && adversarial/.venv/bin/python adversarial/api-fuzz/schemathesis_fuzz.py

Checks run per generated case (schemathesis 4.25's built-in registry, loaded via
`schemathesis.checks.load_all_checks()`):
  - not_a_server_error          -- a 5xx is always a finding
  - response_schema_conformance -- a 200 whose body doesn't match the declared OpenAPI schema is
                                    exactly the "silently-wrong-200" class the H2 brief calls out
  - negative_data_rejection     -- invalid data schemathesis constructs must NOT be silently accepted
  - positive_data_acceptance    -- valid data must NOT be silently rejected
plus two hand-written semantic checks (money never negative, `mode` always one of the closed union).
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

sys.path.insert(0, "backend/relay-py/src")

import hypothesis  # noqa: E402
import schemathesis  # noqa: E402
from hypothesis import HealthCheck, settings  # noqa: E402
from schemathesis import Case, Response  # noqa: E402
from schemathesis.checks import CheckContext  # noqa: E402

import orb_relay.app as app_module  # noqa: E402
from orb_relay.app import app, get_gateway  # noqa: E402
from orb_relay.cost.meter import UsageDelta  # noqa: E402
from orb_relay.proxy.gateway_client import GatewayCompletion  # noqa: E402
from orb_relay.store.context_store import ContextStore  # noqa: E402
from orb_relay.store.conversation_store import ConversationStore  # noqa: E402

# --- isolate storage: this run must never touch the real dev sqlite files ------------------------
_tmp_dir = Path(tempfile.mkdtemp(prefix="orb-h2-schemathesis-"))
app_module._context_store = ContextStore(_tmp_dir / "context.db")
app_module._conversation_store = ConversationStore(_tmp_dir / "conversation.db")


# --- fake gateway: deterministic, varied, ZERO egress, ZERO cost ---------------------------------
class VariedFakeGateway:
    def __init__(self) -> None:
        self.calls = 0
        self._replies = [
            "Sure, here is a short warm reply.",
            "[warm] Got it. [emphasis]One step at a time.",
            "I hear that. Let's take one small step.",
        ]

    async def complete(self, **_: object) -> GatewayCompletion:
        self.calls += 1
        reply = self._replies[self.calls % len(self._replies)]
        return GatewayCompletion(reply, UsageDelta(llm_tokens_in=8, llm_tokens_out=12))


_fake_gateway = VariedFakeGateway()


async def _override_gateway():
    return _fake_gateway


app.dependency_overrides[get_gateway] = _override_gateway

schema = schemathesis.openapi.from_asgi("/openapi.json", app)
schemathesis.checks.load_all_checks()
BUILTIN_CHECKS = schemathesis.checks.CHECKS.get_by_names(
    ["not_a_server_error", "response_schema_conformance", "negative_data_rejection", "positive_data_acceptance"]
)


@schemathesis.check
def no_silent_negative_cost(ctx: CheckContext, response: Response, case: Case) -> None:
    if response.status_code != 200:
        return
    try:
        body = response.json()
    except ValueError:
        return
    if not isinstance(body, dict):
        return
    for field in ("spent_paise", "remaining_paise"):
        if field in body and isinstance(body[field], (int, float)) and body[field] < 0:
            raise AssertionError(f"{case.method} {case.path} returned {field}={body[field]} (negative) with HTTP 200")


@schemathesis.check
def mode_is_always_one_of_the_typed_values(ctx: CheckContext, response: Response, case: Case) -> None:
    if response.status_code != 200 or case.path != "/v1/respond":
        return
    try:
        body = response.json()
    except ValueError:
        return
    if isinstance(body, dict) and "mode" in body and body["mode"] not in ("focus", "converse", "teach"):
        raise AssertionError(f"/v1/respond returned illegal mode value: {body['mode']!r}")


ALL_CHECKS = [*BUILTIN_CHECKS, no_silent_negative_cost, mode_is_always_one_of_the_typed_values]

EXAMPLES_PER_OPERATION = 200


def main() -> int:
    operations = []
    for wrapped in schema.get_all_operations():
        try:
            operations.append(wrapped.ok())
        except Exception as exc:  # noqa: BLE001
            print(f"SKIP operation (could not build): {exc}")

    total_cases = 0
    status_counts: dict[int, int] = {}
    findings: list[str] = []
    ok_operations: list[str] = []

    for op in operations:
        label = f"{op.method.upper()} {op.path}"
        print(f"\n--- fuzzing {label} ---")
        strategy = op.as_strategy()
        op_cases = 0
        op_findings: list[str] = []

        @settings(max_examples=EXAMPLES_PER_OPERATION, deadline=None, suppress_health_check=list(HealthCheck))
        @hypothesis.given(case=strategy)
        def run_case(case: Case) -> None:
            nonlocal op_cases
            op_cases += 1
            response = case.call()
            status_counts[response.status_code] = status_counts.get(response.status_code, 0) + 1
            case.validate_response(response, checks=ALL_CHECKS)

        try:
            run_case()
        except (KeyboardInterrupt, SystemExit):
            raise
        except BaseException as exc:  # noqa: BLE001 -- schemathesis raises a BaseExceptionGroup on
            # multi-check failure, which does NOT subclass Exception; must be caught broadly here
            # so one operation's finding doesn't abort the whole run before later operations are
            # even attempted (discovered by this exact run -- see the H2 report).
            msg = str(exc)
            print(f"  FINDING after {op_cases}+ cases:\n{msg[:3000]}")
            op_findings.append(msg[:2000])
        else:
            print(f"  clean: {op_cases} cases, 0 check failures")
            ok_operations.append(f"{label} ({op_cases} cases)")

        total_cases += op_cases
        findings.extend(f"{label}: {f}" for f in op_findings)

    print("\n" + "=" * 78)
    print(f"TOTAL CASES SENT (all operations): {total_cases}")
    print(f"HTTP status code distribution: {dict(sorted(status_counts.items()))}")
    print(f"Fake-gateway invocations (0 real provider calls, ~0 INR spent): {_fake_gateway.calls}")
    print(f"Operations with ZERO check failures: {len(ok_operations)}")
    for label in ok_operations:
        print(f"  clean: {label}")
    print(f"Operations with >=1 finding: {len(findings)}")
    for f in findings:
        print(f"  FINDING: {f[:500]}")
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
