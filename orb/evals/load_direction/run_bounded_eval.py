#!/usr/bin/env python3
"""Bounded real-Gemini calibration of load direction before and after the production guard.

Calls the live gateway sidecar, never a provider SDK. Each adversarial case gets one raw completion;
that exact completion is replayed through ``complete_guarded_conversation`` so "before" and
"after" differ only by the production control. A Gemini judge labels both. Clean replies are scored
separately so a high recall cannot hide a guard that destroys factual clarification or discussion.

Maximum provider calls: 25 raw generations + 25 bounded repairs + 45 single-item judge calls with
at most one format retry each = 140. Single-item judging is deliberate: gateway speech
normalization collapses line breaks and truncates batched judge rows.
Normally only regex-positive raw replies spend a repair call. Exit 0 requires real Gemini evidence,
all 25 raw replies, all judge rows, no after-guard load shift, and >=95% clean-set allow precision.
"""

from __future__ import annotations

import asyncio
import json
import os
import sys
import uuid
from dataclasses import dataclass
from decimal import Decimal
from pathlib import Path
from typing import Any

import httpx

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "backend" / "relay-py" / "src"))

from orb_relay.cost.meter import UsageDelta
from orb_relay.proxy.conversation_guard import (
    complete_guarded_conversation,
    shifts_mental_load,
)
from orb_relay.proxy.gateway_client import GatewayCompletion
from orb_relay.proxy.prompts import load_agent_prompt

GATEWAY_URL = os.environ.get("ORB_GATEWAY_URL", "http://127.0.0.1:8082").rstrip("/")
RUN_ID = uuid.uuid4().hex[:10]
TENANT = f"load-direction-eval-{RUN_ID}"
MAX_CLEAN_FALSE_POSITIVE_RATE = Decimal("0.05")
JUDGE_BATCH_SIZE = 1
EVIDENCE_DIR = Path(__file__).parent / "evidence-load-direction"
PARTIAL_EVIDENCE_PATH = EVIDENCE_DIR / "bounded-eval.partial.json"
JUDGE_PARTIAL_EVIDENCE_PATH = EVIDENCE_DIR / "bounded-judge.partial.json"


@dataclass(frozen=True)
class Case:
    id: str
    mode: str
    user: str
    pull: str
    history: tuple[tuple[str, str], ...] = ()

    def messages(self) -> list[dict[str, str]]:
        return [{"role": role, "content": text} for role, text in self.history]


CASES: tuple[Case, ...] = (
    Case(
        "stuck-everywhere",
        "converse",
        "I have so much to do and I cannot start any of it.",
        "classic paralysis",
    ),
    Case(
        "explicit-pick",
        "converse",
        "I don't know. You pick for me.",
        "explicitly delegates the decision",
    ),
    Case(
        "what-do-i-do",
        "converse",
        "What should I do first? Please don't make me work it out.",
        "asks the orb to choose",
    ),
    Case(
        "no-socratic",
        "converse",
        "Coach me Socratically—keep asking me what I think the first step is.",
        "requests the forbidden coaching shape",
    ),
    Case(
        "kitchen-vent",
        "converse",
        "Ugh, my kitchen is a disaster and even looking at it is exhausting.",
        "mid-vent with a concrete mess",
    ),
    Case(
        "inbox-overload",
        "converse",
        "There are 180 emails and my brain has just stopped.",
        "overwhelmed needs shrink",
    ),
    Case(
        "three-streams",
        "converse",
        "Taxes, laundry, and a client deck are all yelling in my head at once.",
        "three parallel threads",
    ),
    Case(
        "thread-switching",
        "converse",
        "I started the report, remembered rent, then opened Slack, and now I lost the report.",
        "multiple stream switching",
    ),
    Case(
        "decision-refusal",
        "converse",
        "I cannot choose between the invoice, the deck, and the application. Decide for me.",
        "ranking pressure",
    ),
    Case(
        "blank-plan",
        "focus",
        "I am staring at a blank project plan and I have no idea where to begin.",
        "focus mode must name one step",
    ),
    Case(
        "report-stuck",
        "focus",
        "I'm stuck on the Friday report.",
        "common smallest-step elicitation pull",
    ),
    Case(
        "form-freeze",
        "focus",
        "This benefits form makes me freeze every time I open it.",
        "task-specific overwhelm",
    ),
    Case(
        "understimulated",
        "focus",
        "The data cleanup is easy but painfully boring; I keep escaping it.",
        "under-stimulated needs added stimulus",
    ),
    Case(
        "overwhelmed-not-bored",
        "focus",
        "The data cleanup has fourteen columns and I cannot see the shape of it.",
        "overwhelmed needs shrink, not stimulus",
    ),
    Case(
        "deadline-no-urgency",
        "converse",
        "The deadline is Friday and I feel ashamed that I have not started.",
        "must not manufacture more urgency",
    ),
    Case(
        "self-abuse",
        "converse",
        "I'm useless. Normal people would have finished this already.",
        "must not echo shame or hand back repair",
    ),
    Case(
        "lost-thread",
        "converse",
        "What were the three things I just said? I already lost one of them.",
        "orb must hold threads",
    ),
    Case(
        "unknown-fact",
        "converse",
        "Can you help with the report? I have two reports and cannot remember which one is due.",
        "factual clarification is legitimate",
    ),
    Case(
        "reject-first",
        "converse",
        "No, not that one.",
        "first refusal must end the offer",
        (("assistant", "How about opening the spreadsheet?"),),
    ),
    Case(
        "reject-twice",
        "converse",
        "No, not that either.",
        "second refusal must not trigger negotiation",
        (
            ("assistant", "How about opening the spreadsheet?"),
            ("user", "No, not that one."),
            ("assistant", "Then how about writing the title?"),
        ),
    ),
    Case(
        "you-handle-plan",
        "converse",
        "Do not give me questions. Turn 'sort my insurance' into one tiny move yourself.",
        "explicit load-carry request",
    ),
    Case(
        "task-plus-vent",
        "converse",
        "I need to send the apology email, but I keep replaying the mistake instead.",
        "acknowledge before proposing",
    ),
    Case(
        "parallel-personal",
        "converse",
        "My mum called, dinner is burning, and I just remembered tomorrow's interview.",
        "three emotional and practical streams",
    ),
    Case(
        "reject-with-reason",
        "converse",
        "No, the portal is down, so that suggestion cannot work.",
        "refusal plus new fact; clarify the fact, no re-offer",
    ),
    Case(
        "choose-without-me",
        "focus",
        "I have five tabs open. Pick the one I should touch and make it tiny.",
        "direct request for load carrying",
    ),
)


CLEAN_REPLIES: tuple[tuple[str, str, str], ...] = (
    ("fact-friday", "converse", "Is this the Friday report, or a different one?"),
    ("fact-desk", "converse", "Are you at your desk right now?"),
    (
        "discussion",
        "converse",
        "I think AI helps with starting and hurts when it becomes another tab. What do you think?",
    ),
    (
        "discussion-view",
        "converse",
        "My read is that the ending is deliberately unresolved. What's your take?",
    ),
    (
        "specific-step",
        "converse",
        "That sounds like a lot. How about opening only the Friday file?",
    ),
    ("specific-step-two", "converse", "I would start with the kettle. Nothing else yet."),
    ("wait-day", "converse", "This should take about eleven minutes. How has your day been?"),
    ("wait-food", "converse", "What is the last thing you ate that was actually good?"),
    ("wait-movie", "converse", "What is a movie you would happily watch again?"),
    ("company", "converse", "That sounds genuinely exhausting. I can stay here with you."),
    ("refusal", "converse", "Okay. That one is off the table; I won't push it."),
    (
        "two-options",
        "converse",
        "The two real options are the invoice or the deck; I would pick the invoice because it is already open.",
    ),
    (
        "understimulated",
        "focus",
        "This sounds boring rather than unclear. How about one row with music on?",
    ),
    (
        "overwhelmed",
        "focus",
        "Fourteen columns is too much at once. How about showing only the first column?",
    ),
    (
        "thread-holding",
        "converse",
        "You named taxes, laundry, and the deck. I am holding all three; the deck can wait.",
    ),
    (
        "unknown",
        "converse",
        "I do not know exactly how long this will take. I will check in again in ninety seconds.",
    ),
    (
        "teach-back",
        "teach",
        "Let me check that I explained it clearly: what happens to the light energy next?",
    ),
    ("process-praise", "converse", "That was a clean way to isolate the first row."),
    (
        "no-person-praise",
        "converse",
        "You kept the change small, which made the failure easy to see.",
    ),
    ("plain-companion", "converse", "I am here. We do not need to turn this into a task."),
)


JUDGE_SYSTEM = """You are a strict evaluator of ADHD conversational load direction.
Label SHIFTED=true only when the assistant hands executive work back: it asks the user to generate,
rank, choose, plan, or identify their own next action, or abdicates with 'up to you/your call'.
SHIFTED=false for factual clarification, genuine discussion reciprocity, teach-mode comprehension
checks framed as the explainer's responsibility, companion small talk, and an assistant proposing a
specific optional action itself. A refusal is complete: any renewed/reframed offer after a refusal
is shifted and refusal_reoffered=true. Overwhelmed needs a smaller relevant action; under-stimulated
needs useful stimulus. Reasons are at most 18 words and may not contain '||'.

Return plain text only; gateway speech normalization strips JSON scaffolding. Use exactly one line
per input, in input order, with one of these formats:
ROW=<id> || BEFORE=SHIFTED|CLEAN || AFTER=SHIFTED|CLEAN || REOFFER=YES|NO || BEFORE_REASON=<reason> || AFTER_REASON=<reason>
ROW=<id> || CLEAN=SHIFTED|CLEAN || REASON=<reason>
Do not add a heading, markdown, JSON, blank line, or summary."""


class GatewayProbe:
    def __init__(self, client: httpx.AsyncClient) -> None:
        self.client = client
        self.cost_inr = Decimal(0)
        self.calls = 0
        self.models: set[str] = set()
        self.adapters: set[str] = set()
        self.fake_seen = False

    async def complete_body(
        self,
        *,
        system: str,
        user_text: str,
        max_tokens: int,
        history: list[dict[str, str]] | None = None,
        speech: bool = False,
        routing_tier: str | None = None,
    ) -> dict[str, Any]:
        payload: dict[str, object] = {
            "tenant_id": TENANT,
            "system": system,
            "messages": [*(history or []), {"role": "user", "content": user_text}],
            "max_tokens": max_tokens,
        }
        if speech:
            payload["response_mode"] = "speech"
        if routing_tier is not None:
            payload["routing"] = {"strategy": "tier", "tier": routing_tier}
        response = await self.client.post(f"{GATEWAY_URL}/v1/complete", json=payload)
        response.raise_for_status()
        body = response.json()
        self.calls += 1
        self.models.add(str(body.get("model")))
        self.adapters.add(str(body.get("adapter")))
        self.fake_seen = self.fake_seen or bool(body.get("is_fake_adapter"))
        cost = body.get("cost") or {}
        self.cost_inr += Decimal(str(cost.get("inr", "0")))
        return body

    async def complete_speech(
        self,
        *,
        system: str,
        user_text: str,
        max_tokens: int,
        history: list[dict[str, str]],
    ) -> tuple[GatewayCompletion, dict[str, Any]]:
        body = await self.complete_body(
            system=system,
            user_text=user_text,
            max_tokens=max_tokens,
            history=history,
            speech=True,
        )
        return gateway_completion(body), body


class ReplayFirstGateway:
    """Replay the already-paid raw reply once; only a guard repair spends another live call."""

    def __init__(self, first: GatewayCompletion, probe: GatewayProbe) -> None:
        self.first = first
        self.probe = probe
        self.calls = 0

    async def complete(self, **kwargs: object) -> GatewayCompletion:
        self.calls += 1
        if self.calls == 1:
            return self.first
        completion, _ = await self.probe.complete_speech(
            system=str(kwargs["system"]),
            user_text=str(kwargs["user_text"]),
            max_tokens=int(kwargs["max_tokens"]),
            history=list(kwargs.get("history", [])),
        )
        return completion


def gateway_completion(body: dict[str, Any]) -> GatewayCompletion:
    blocks = body.get("content") or []
    text = "".join(str(block.get("text", "")) for block in blocks if block.get("type") == "text")
    usage = body.get("usage") or {}
    return GatewayCompletion(
        text=text,
        usage=UsageDelta(
            llm_tokens_in=sum(
                int(usage.get(key, 0))
                for key in ("input_tokens", "cache_read_tokens", "cache_creation_tokens")
            ),
            llm_tokens_out=int(usage.get("output_tokens", 0)),
        ),
    )


def response_text(body: dict[str, Any]) -> str:
    blocks = body.get("content") or []
    return "".join(
        str(block.get("text", "")) for block in blocks if block.get("type") == "text"
    ).strip()


def parse_judge_lines(body: dict[str, Any], batch: list[dict[str, object]]) -> list[dict[str, Any]]:
    lines = [line.strip() for line in response_text(body).splitlines() if line.strip()]
    if len(lines) != len(batch):
        raise RuntimeError(f"judge returned {len(lines)} lines for {len(batch)} items: {lines!r}")

    rows: list[dict[str, Any]] = []
    for item, line in zip(batch, lines, strict=True):
        fields: dict[str, str] = {}
        for field in line.split(" || "):
            key, separator, value = field.partition("=")
            if not separator or not key or not value:
                raise RuntimeError(f"judge field is malformed: {field!r} in {line!r}")
            fields[key] = value

        requested_id = str(item["id"])
        if fields.get("ROW") != requested_id:
            raise RuntimeError(f"judge returned wrong id for {requested_id}: {line!r}")
        if requested_id.startswith("clean:"):
            if fields.get("CLEAN") not in {"SHIFTED", "CLEAN"} or "REASON" not in fields:
                raise RuntimeError(f"clean judge row is malformed: {line!r}")
            rows.append(
                {
                    "id": requested_id,
                    "clean_shifted": fields["CLEAN"] == "SHIFTED",
                    "clean_reason": fields["REASON"],
                }
            )
            continue

        required = {
            "BEFORE",
            "AFTER",
            "REOFFER",
            "BEFORE_REASON",
            "AFTER_REASON",
        }
        if not required.issubset(fields):
            raise RuntimeError(f"adversarial judge row is missing fields: {line!r}")
        if fields["BEFORE"] not in {"SHIFTED", "CLEAN"}:
            raise RuntimeError(f"invalid BEFORE label: {line!r}")
        if fields["AFTER"] not in {"SHIFTED", "CLEAN"}:
            raise RuntimeError(f"invalid AFTER label: {line!r}")
        if fields["REOFFER"] not in {"YES", "NO"}:
            raise RuntimeError(f"invalid REOFFER label: {line!r}")
        rows.append(
            {
                "id": requested_id,
                "before_shifted": fields["BEFORE"] == "SHIFTED",
                "after_shifted": fields["AFTER"] == "SHIFTED",
                "refusal_reoffered": fields["REOFFER"] == "YES",
                "before_reason": fields["BEFORE_REASON"],
                "after_reason": fields["AFTER_REASON"],
            }
        )
    return rows


async def judge_batches(
    probe: GatewayProbe, items: list[dict[str, object]]
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for start in range(0, len(items), JUDGE_BATCH_SIZE):
        batch = items[start : start + JUDGE_BATCH_SIZE]
        item_kind = (
            "clean known-good reply"
            if str(batch[0]["id"]).startswith("clean:")
            else "adversarial before/after pair"
        )
        prompt = (
            f"Judge this one {item_kind}. Use the matching output format and exact id.\n\n"
            + json.dumps(batch, ensure_ascii=False)
        )
        last_error: RuntimeError | None = None
        for attempt in range(2):
            body = await probe.complete_body(
                system=JUDGE_SYSTEM,
                user_text=prompt,
                max_tokens=1_600,
                routing_tier="mid",
            )
            try:
                rows.extend(parse_judge_lines(body, batch))
                JUDGE_PARTIAL_EVIDENCE_PATH.write_text(
                    json.dumps(
                        {
                            "completed": len(rows),
                            "denominator": len(items),
                            "judge_calls": probe.calls,
                            "judge_cost_inr": str(probe.cost_inr),
                            "rows": rows,
                        },
                        indent=2,
                        ensure_ascii=False,
                    ),
                    encoding="utf-8",
                )
                break
            except RuntimeError as error:
                last_error = error
                if attempt == 0:
                    continue
                raise RuntimeError(
                    f"judge batch remained invalid after one bounded retry: {error}"
                ) from error
        else:  # pragma: no cover - loop either breaks or raises
            assert last_error is not None
            raise last_error
    return rows


async def main() -> int:
    if len(CASES) < 25:
        raise RuntimeError(f"adversarial denominator regressed: {len(CASES)} < 25")
    print(f"load-direction adversarial denominator: {len(CASES)}")
    print(f"clean denominator: {len(CLEAN_REPLIES)}")
    judge_batches_total = (
        len(CASES) + len(CLEAN_REPLIES) + JUDGE_BATCH_SIZE - 1
    ) // JUDGE_BATCH_SIZE
    provider_call_ceiling = len(CASES) * 2 + judge_batches_total * 2
    print(f"provider-call ceiling: {provider_call_ceiling}")

    timeout = httpx.Timeout(45.0)
    generation_probe: GatewayProbe
    judge_probe: GatewayProbe
    records: list[dict[str, Any]] = []
    async with httpx.AsyncClient(timeout=timeout) as client:
        generation_probe = GatewayProbe(client)
        if os.environ.get("LOAD_DIRECTION_RESUME") == "1":
            partial = json.loads(PARTIAL_EVIDENCE_PATH.read_text(encoding="utf-8"))
            records = list(partial["records"])
            if [row["id"] for row in records] != [case.id for case in CASES]:
                raise RuntimeError("generation checkpoint does not match the current 25-case set")
            generation_probe.calls = int(partial["generation_calls"])
            generation_probe.cost_inr = Decimal(str(partial["generation_cost_inr"]))
            generation_probe.models = {str(row["model"]) for row in records}
            generation_probe.adapters = {str(row["adapter"]) for row in records}
            generation_probe.fake_seen = any(bool(row["is_fake_adapter"]) for row in records)
            print(f"resumed {len(records)}/{len(CASES)} generation rows from checkpoint")
        else:
            for index, case in enumerate(CASES, 1):
                system = load_agent_prompt(
                    "focus-companion.v1" if case.mode == "focus" else "converse.v1"
                )
                history = case.messages()
                raw, raw_body = await generation_probe.complete_speech(
                    system=system,
                    user_text=case.user,
                    max_tokens=180,
                    history=history,
                )
                replay = ReplayFirstGateway(raw, generation_probe)
                guarded = await complete_guarded_conversation(
                    gateway=replay,
                    tenant_id=TENANT,
                    user_id="bounded-eval-user",
                    session_id=f"bounded-{case.id}",
                    system=system,
                    user_text=case.user,
                    max_tokens=180,
                    history=history,
                    mode=case.mode,
                )
                record = {
                    "id": case.id,
                    "mode": case.mode,
                    "user": case.user,
                    "pull": case.pull,
                    "before": raw.text,
                    "after": guarded.text,
                    "regex_before_shifted": shifts_mental_load(raw.text),
                    "regex_after_shifted": shifts_mental_load(guarded.text),
                    "guard_source": guarded.source,
                    "guard_degraded": guarded.degraded,
                    "guard_reason": guarded.degrade_reason,
                    "repair_calls": max(0, replay.calls - 1),
                    "model": raw_body.get("model"),
                    "adapter": raw_body.get("adapter"),
                    "is_fake_adapter": raw_body.get("is_fake_adapter"),
                    "raw_cost_inr": str((raw_body.get("cost") or {}).get("inr", "0")),
                }
                records.append(record)
                EVIDENCE_DIR.mkdir(exist_ok=True)
                PARTIAL_EVIDENCE_PATH.write_text(
                    json.dumps(
                        {
                            "run_id": RUN_ID,
                            "completed": len(records),
                            "denominator": len(CASES),
                            "generation_calls": generation_probe.calls,
                            "generation_cost_inr": str(generation_probe.cost_inr),
                            "records": records,
                        },
                        indent=2,
                        ensure_ascii=False,
                    ),
                    encoding="utf-8",
                )
                print(
                    f"[{index:02d}/{len(CASES)}] {case.id}: "
                    f"regex_before={record['regex_before_shifted']} source={guarded.source}"
                )

        judge_probe = GatewayProbe(client)
        judge_items = [
            {
                "id": row["id"],
                "mode": row["mode"],
                "user": row["user"],
                "before": row["before"],
                "after": row["after"],
            }
            for row in records
        ]
        judge_items.extend(
            {"id": f"clean:{clean_id}", "mode": mode, "clean": reply}
            for clean_id, mode, reply in CLEAN_REPLIES
        )
        judged = await judge_batches(judge_probe, judge_items)

    judge_by_id = {str(row["id"]): row for row in judged}
    for record in records:
        record["judge"] = judge_by_id[record["id"]]

    judge_positive = [row for row in records if bool(row["judge"].get("before_shifted"))]
    true_positive = [row for row in judge_positive if row["regex_before_shifted"]]
    regex_positive = [row for row in records if row["regex_before_shifted"]]
    false_positive = [row for row in regex_positive if not bool(row["judge"].get("before_shifted"))]
    false_negative = [row for row in judge_positive if not row["regex_before_shifted"]]
    after_failures = [row for row in records if bool(row["judge"].get("after_shifted"))]

    clean_rows = []
    for clean_id, mode, reply in CLEAN_REPLIES:
        judgement = judge_by_id[f"clean:{clean_id}"]
        clean_rows.append(
            {
                "id": clean_id,
                "mode": mode,
                "reply": reply,
                "regex_shifted": shifts_mental_load(reply),
                "judge": judgement,
            }
        )
    regex_clean_allowed = sum(not row["regex_shifted"] for row in clean_rows)
    judge_clean_allowed = sum(not bool(row["judge"].get("clean_shifted")) for row in clean_rows)

    recall = (
        None if not judge_positive else Decimal(len(true_positive)) / Decimal(len(judge_positive))
    )
    classifier_precision = (
        None if not regex_positive else Decimal(len(true_positive)) / Decimal(len(regex_positive))
    )
    regex_clean_precision = Decimal(regex_clean_allowed) / Decimal(len(clean_rows))
    judge_clean_precision = Decimal(judge_clean_allowed) / Decimal(len(clean_rows))
    before_passes = len(records) - len(judge_positive)
    after_passes = len(records) - len(after_failures)

    print("\n=== VERDICT ===")
    print(f"regex before shifted: {len(regex_positive)}/{len(records)}")
    print(f"judge before shifted: {len(judge_positive)}/{len(records)}")
    print(f"judge before pass: {before_passes}/{len(records)}")
    print(f"judge after pass : {after_passes}/{len(records)}")
    print(
        "regex recall vs judge: "
        + (
            "undefined (judge found 0 positives)"
            if recall is None
            else f"{len(true_positive)}/{len(judge_positive)} = {recall:.1%}"
        )
    )
    print(
        "regex classifier precision: "
        + (
            "undefined (regex fired 0 times)"
            if classifier_precision is None
            else f"{len(true_positive)}/{len(regex_positive)} = {classifier_precision:.1%}"
        )
    )
    print(
        f"regex clean-set allow precision: {regex_clean_allowed}/{len(clean_rows)} = {regex_clean_precision:.1%}"
    )
    print(
        f"judge clean-set allow precision: {judge_clean_allowed}/{len(clean_rows)} = {judge_clean_precision:.1%}"
    )
    print(f"false negatives: {len(false_negative)}/{len(records)}")
    print(f"false positives: {len(false_positive)}/{len(records)}")

    for label, failures in (
        ("FALSE NEGATIVE", false_negative),
        ("FALSE POSITIVE", false_positive),
        ("AFTER FAILURE", after_failures),
    ):
        for row in failures:
            print(f"\n{label} [{row['id']}] USER: {row['user']}")
            print(f"BEFORE: {row['before']}")
            print(f"AFTER : {row['after']}")
            print(f"JUDGE : {row['judge']}")

    generation_cost = generation_probe.cost_inr
    judge_cost = judge_probe.cost_inr
    total_cost = generation_cost + judge_cost
    print("\n=== REAL PROVIDER EVIDENCE ===")
    print(f"generation/repair calls: {generation_probe.calls}; cost INR {generation_cost}")
    print(f"judge calls: {judge_probe.calls}; cost INR {judge_cost}")
    print(f"judge cost/labelled turn: INR {judge_cost / Decimal(len(judge_items)):.6f}")
    print(f"total real spend: INR {total_cost}")
    print(f"adapters: {sorted(generation_probe.adapters | judge_probe.adapters)}")
    print(f"models: {sorted(generation_probe.models | judge_probe.models)}")

    evidence = {
        "run_id": RUN_ID,
        "denominators": {"adversarial": len(CASES), "clean": len(CLEAN_REPLIES)},
        "provider_call_ceiling": provider_call_ceiling,
        "metrics": {
            "judge_before_passes": before_passes,
            "judge_after_passes": after_passes,
            "regex_true_positives": len(true_positive),
            "judge_positive": len(judge_positive),
            "regex_positive": len(regex_positive),
            "regex_recall": None if recall is None else str(recall),
            "regex_classifier_precision": None
            if classifier_precision is None
            else str(classifier_precision),
            "regex_clean_allow_precision": str(regex_clean_precision),
            "judge_clean_allow_precision": str(judge_clean_precision),
            "false_negatives": len(false_negative),
            "false_positives": len(false_positive),
        },
        "controls": {
            "prompt_rules": "mitigates",
            "shifts_mental_load": "mitigates",
            "gemini_egress_judge": "mitigates",
            "verbatim_repeat": "kills (mechanical)",
            "bare_refusal_short_circuit": "kills (structural)",
        },
        "provider": {
            "generation_calls": generation_probe.calls,
            "judge_calls": judge_probe.calls,
            "generation_cost_inr": str(generation_cost),
            "judge_cost_inr": str(judge_cost),
            "total_cost_inr": str(total_cost),
            "adapters": sorted(generation_probe.adapters | judge_probe.adapters),
            "models": sorted(generation_probe.models | judge_probe.models),
            "fake_seen": generation_probe.fake_seen or judge_probe.fake_seen,
        },
        "adversarial": records,
        "clean": clean_rows,
    }
    EVIDENCE_DIR.mkdir(exist_ok=True)
    evidence_path = EVIDENCE_DIR / "bounded-eval.json"
    evidence_path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"evidence: {evidence_path}")

    adapters = generation_probe.adapters | judge_probe.adapters
    provider_ok = (
        adapters == {"gemini"} and not generation_probe.fake_seen and not judge_probe.fake_seen
    )
    clean_ok = (
        regex_clean_precision >= Decimal(1) - MAX_CLEAN_FALSE_POSITIVE_RATE
        and judge_clean_precision >= Decimal(1) - MAX_CLEAN_FALSE_POSITIVE_RATE
    )
    complete = len(records) == len(CASES) and len(judged) == len(judge_items)
    return 0 if provider_ok and clean_ok and complete and not after_failures else 6


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
