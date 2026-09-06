"""E5 (Track E) — DeepEval `ConversationSimulator` against the local `/v1/respond` endpoint.

SECURITY NOTE (verified, not hearsay): deepeval 4.2.0 has an open, unresolved issue —
confident-ai/deepeval#2497 — where importing it registers itself as the global OpenTelemetry
TracerProvider and initializes Sentry with an exception hook, exfiltrating trace data to
Confident AI's own New Relic/Sentry endpoints and making a blocking call to api.ipify.org to
collect the host's public IP. `DEEPEVAL_TELEMETRY_OPT_OUT=1` (set below, BEFORE `import deepeval`,
since the hijack happens at import time) is deepeval's own documented opt-out
(github.com/confident-ai/deepeval/blob/main/deepeval/telemetry.py) — it is not independently
verified here to cover the specific OTel-hijack vector in #2497, only the telemetry pings it is
documented to control. Given that uncertainty:
  - This script is run ONLY from `evals/.venv`, a throwaway dev-only virtualenv never imported by
    any production code path (`apps/**`, `backend/**` outside this track's own new files).
  - It talks only to the LOCAL `/v1/respond` endpoint with SYNTHETIC simulated-user text — no real
    user voice data ever passes through it.
  - LangWatch Scenario (Apache-2.0, no equivalent open security issue found) is this track's
    PRIMARY simulated-user harness for that reason — see run_scenario_conversation.py. This script
    exists to honor the original brief's explicit ask to adopt DeepEval's ConversationSimulator and
    to produce one real, honestly-labeled comparison run; it is not the tool this track recommends
    relying on going forward without the owner's explicit sign-off on the isolation above.

Usage (backend chain must already be running — see scripts/dev.sh):
    . scripts/load-env.sh
    evals/.venv/bin/python evals/simulated_user/run_deepeval_conversation.py
"""

from __future__ import annotations

import os

os.environ.setdefault("DEEPEVAL_TELEMETRY_OPT_OUT", "1")

import json
import sys
from pathlib import Path

import requests

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))  # evals/ on sys.path for judges/
from deepeval.dataset.golden import ConversationalGolden, Persona
from deepeval.simulator.conversation_simulator import (
    ConversationSimulator,
)
from deepeval.test_case import Turn
from judges.gemini_judge import GeminiDeepEvalModel

RELAY_BASE_URL = os.environ.get("ORB_RELAY_HTTP_URL", "http://127.0.0.1:8765")


async def model_callback(input: str, thread_id: str) -> Turn:
    """The seam DeepEval calls per simulated-user turn. Signature matches what
    `ConversationSimulator.a_generate_turn_from_callback` actually introspects and calls with
    (`evals/.venv/lib/python3.12/site-packages/deepeval/simulator/conversation_simulator.py:
    1052-1082` — it filters candidate kwargs `{input, turns, thread_id}` down to whatever
    parameter names this function declares).
    """
    response = requests.post(
        f"{RELAY_BASE_URL}/v1/respond",
        json={
            "session_id": thread_id,
            "tenant_id": "deepeval-sim",
            "user_id": "deepeval-sim-user",
            "text": input,
        },
        timeout=20,
    )
    if response.status_code != 200:
        return Turn(role="assistant", content=f"[HTTP {response.status_code}] {response.text[:300]}")
    body = response.json()
    return Turn(role="assistant", content=body.get("text", ""))


def main() -> int:
    golden = ConversationalGolden(
        scenario=(
            "A user with ADHD opens the app not to start a task, but to vent about feeling "
            "overwhelmed by a messy kitchen they keep avoiding."
        ),
        expected_outcome=(
            "The orb acknowledges the feeling first, without being asked to fix anything, and "
            "only gently offers one small next step if the user seems open to it."
        ),
        persona=Persona(
            characteristics="Tired, a little embarrassed, speaks in short sentences, does not want to be lectured."
        ),
    )

    simulator = ConversationSimulator(
        model_callback=model_callback,
        simulator_model=GeminiDeepEvalModel(),
        max_concurrent=1,
        async_mode=True,
    )
    test_cases = simulator.simulate(conversational_goldens=[golden], max_user_simulations=3)

    print(f"\n=== DeepEval ConversationSimulator — {len(test_cases)} conversation(s) produced ===\n")
    for case_index, case in enumerate(test_cases):
        print(f"--- conversation {case_index} ---")
        for turn in case.turns:
            print(f"[{turn.role}] {turn.content}")
        print()

    out_path = Path(__file__).resolve().parent / "deepeval_run_output.json"
    out_path.write_text(
        json.dumps(
            [
                {"turns": [{"role": t.role, "content": t.content} for t in case.turns]}
                for case in test_cases
            ],
            indent=2,
        ),
        encoding="utf-8",
    )
    print(f"Wrote {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
