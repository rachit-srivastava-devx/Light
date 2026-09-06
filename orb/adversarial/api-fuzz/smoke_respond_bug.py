"""Quick, targeted smoke check (NOT the full fuzz harness) for a suspected defect spotted while
reading backend/relay-py/src/orb_relay/app.py: `respond_to_user`'s `return ConversationResponse(...)`
statement (around line 525) does not pass `mode`, `beats`, `degraded`, `degrade_reason`, or
`token_ceiling` -- all of which are REQUIRED (no default) fields on the `ConversationResponse`
Pydantic model (proxy/schemas.py added them for contract C1-C5/B4, but app.py's handler was not
updated to populate them). If that reading is right, EVERY call to POST /v1/respond should 500 with
a pydantic ResponseValidationError, not just a fuzzer-found edge case -- i.e. the endpoint may be
completely broken for its single documented happy path. Zero network egress: the gateway dependency
is overridden with a fake.

Run: PYTHONPATH=backend/relay-py/src adversarial/.venv/bin/python adversarial/api-fuzz/smoke_respond_bug.py
"""

from __future__ import annotations

import sys

from fastapi.testclient import TestClient

sys.path.insert(0, "backend/relay-py/src")

from orb_relay.app import app, get_gateway  # noqa: E402
from orb_relay.cost.meter import UsageDelta  # noqa: E402
from orb_relay.proxy.gateway_client import GatewayCompletion  # noqa: E402


class FakeGateway:
    async def complete(self, **_: object) -> GatewayCompletion:
        return GatewayCompletion("Hi there, happy to help.", UsageDelta(llm_tokens_in=5, llm_tokens_out=6))


async def _override():
    return FakeGateway()


app.dependency_overrides[get_gateway] = _override

client = TestClient(app, raise_server_exceptions=False)  # False: we want the real 500, not a raised exception
response = client.post(
    "/v1/respond",
    json={"session_id": "smoke-1", "tenant_id": "smoke-tenant", "user_id": "smoke-user", "text": "hello there"},
)

print("HTTP status:", response.status_code)
print("Body       :", response.text[:2000])

if response.status_code == 200:
    print("\nRESULT: /v1/respond's documented happy path WORKS (200). The suspected bug is not currently present.")
elif response.status_code == 500:
    print(
        "\nRESULT: CONFIRMED -- /v1/respond 500s on its OWN documented happy path (a plain text message, "
        "no special mode). Every real caller (the mobile app's ConversationPort.ts) would see this as a "
        "'couldn't reach the service' fallback on literally every conversational turn."
    )
else:
    print(f"\nRESULT: unexpected status {response.status_code} -- investigate further (see body above).")
