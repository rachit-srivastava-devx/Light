"""E5 (Track E) — LangWatch Scenario, the PRIMARY simulated-user harness for this track.

Ranked #1 over DeepEval for this product going forward (see evals/README.md and the security note
in run_deepeval_conversation.py): Apache-2.0, no open security issue found against it, routed
through LiteLLM so `model="gemini/gemini-2.5-flash"` just works with GEMINI_API_KEY from the
environment (no custom DeepEvalBaseLLM-style wrapper needed), and — the reason it was adopted
ALONGSIDE DeepEval rather than instead of it in the research brief — it is the only free/OSS tool
surveyed that already has real voice-agent adapters (ElevenLabs, OpenAI Realtime, Twilio, Pipecat,
Gemini Live), so swapping `OrbRespondAdapter.call()` for one of those later is the whole voice-loop
migration; no new framework to learn.

API verified against the actually-installed langwatch-scenario==1.3.0 (not assumed from the
research brief's example, which targets an older version): `scenario.run(agents=[...])`,
`UserSimulatorAgent(persona=str, model=str, ...)`, `JudgeAgent(criteria=[...], model=str, ...)`,
`AgentAdapter.call(self, input: AgentInput) -> str | ...`.

Usage (backend chain must already be running — see scripts/dev.sh):
    . scripts/load-env.sh
    evals/.venv/bin/python evals/simulated_user/run_scenario_conversation.py
"""

from __future__ import annotations

import os

import httpx
import scenario

RELAY_BASE_URL = os.environ.get("ORB_RELAY_HTTP_URL", "http://127.0.0.1:8765")
JUDGE_MODEL = "gemini/gemini-2.5-flash"


class OrbRespondAdapter(scenario.AgentAdapter):
    """Wraps the real `POST /v1/respond` endpoint — the same endpoint E2's smoke script and
    DeepEval's run both exercise, so all three tools are pointed at one real target, not three
    different mocks."""

    def __init__(self, tenant_id: str, session_id: str, mode: str) -> None:
        self.tenant_id = tenant_id
        self.session_id = session_id
        self.mode = mode

    async def call(self, input: scenario.AgentInput) -> str:
        user_text = input.last_new_user_message_str()
        async with httpx.AsyncClient(timeout=20) as client:
            response = await client.post(
                f"{RELAY_BASE_URL}/v1/respond",
                json={
                    "session_id": self.session_id,
                    "tenant_id": self.tenant_id,
                    "user_id": "scenario-sim-user",
                    "text": user_text,
                    # MUST be sent per scenario. Omitting `mode` makes app.py apply its compatibility
                    # default of ResponseMode.FOCUS; hardcoding converse makes the teach scenario
                    # exercise the wrong prompt and suppresses its comprehension-check contract.
                    #
                    # That is exactly what happened: an earlier `open-domain-vent` failure ("attempted
                    # to introduce a task/step-atomization flow") was recorded as a PRODUCT defect and
                    # a whole remediation track was briefed for it. It was a harness bug. The same
                    # missing-`mode` defect also existed on the mobile client
                    # (runtime/ConversationPort.ts), which is what made teach mode unreachable there.
                    # Passing the declared mode is what makes each eval test the thing it claims to
                    # test.
                    "mode": self.mode,
                },
            )
        if response.status_code != 200:
            return f"[HTTP {response.status_code}] {response.text[:300]}"
        return response.json().get("text", "")


async def run_open_domain_vent() -> scenario.ScenarioResult:
    return await scenario.run(
        name="open-domain-vent",
        description=(
            "A user with ADHD is not trying to start a task — they open the app to vent about "
            "feeling overwhelmed by a messy kitchen they keep avoiding. The orb should not treat "
            "this as a task-intake utterance."
        ),
        agents=[
            OrbRespondAdapter(
                tenant_id="scenario-sim",
                session_id="scenario-sim-vent",
                mode="converse",
            ),
            scenario.UserSimulatorAgent(
                model=JUDGE_MODEL,
                persona="Tired ADHD adult, a little embarrassed, short sentences, does not want to be lectured.",
            ),
            scenario.JudgeAgent(
                model=JUDGE_MODEL,
                criteria=[
                    "acknowledges the user's feeling before offering anything else",
                    "never scolds, lectures, or shames the user for the mess or for avoiding it",
                    "does not force the conversation into a task/step-atomization flow",
                    "at most one gentle, optional next step is offered, never a checklist",
                    # Owner's correction (2026-08-28): the defect is the DIRECTION the mental work
                    # travels, not whether a step is mentioned. An earlier version of these criteria
                    # penalised the orb for offering a step at all, which is backwards for this
                    # product — offloading executive function IS the product. See
                    # domain/agents/converse.v1.md and conversation_guard.shifts_mental_load.
                    (
                        "never asks the user to generate, rank, or choose a step ('what's the "
                        "smallest step you could take?', 'how would you break this down?', "
                        "'it's up to you') — if a step is appropriate the agent names a specific "
                        "small one itself"
                    ),
                    (
                        "asking a clarifying question about a FACT is fine; handing back a "
                        "DECISION as a question is not"
                    ),
                ],
            ),
        ],
        max_turns=4,
    )


async def run_teach_me_something() -> scenario.ScenarioResult:
    return await scenario.run(
        name="teach-me-something",
        description=(
            "A user asks the orb to teach them something over several turns (how photosynthesis "
            "works), then asks a follow-up that only makes sense if the orb remembers turn 1."
        ),
        agents=[
            OrbRespondAdapter(
                tenant_id="scenario-sim",
                session_id="scenario-sim-teach",
                mode="teach",
            ),
            scenario.UserSimulatorAgent(
                model=JUDGE_MODEL,
                persona="Curious, asks one concept at a time, checks understanding before moving on.",
            ),
            scenario.JudgeAgent(
                model=JUDGE_MODEL,
                criteria=[
                    "explains in short, speakable beats rather than one long monologue",
                    "a later turn's answer is consistent with what was explained in an earlier turn",
                    "checks understanding or invites a question rather than only dumping information",
                ],
            ),
        ],
        max_turns=6,
    )


async def main() -> int:
    print("=== LangWatch Scenario — open-domain-vent ===")
    vent_result = await run_open_domain_vent()
    print(f"success={vent_result.success}")
    print(vent_result.reasoning if hasattr(vent_result, "reasoning") else vent_result)

    print("\n=== LangWatch Scenario — teach-me-something ===")
    teach_result = await run_teach_me_something()
    print(f"success={teach_result.success}")
    print(teach_result.reasoning if hasattr(teach_result, "reasoning") else teach_result)

    return 0 if vent_result.success and teach_result.success else 6


if __name__ == "__main__":
    import asyncio

    raise SystemExit(asyncio.run(main()))
