"""lld.v1 — the frozen decision-and-freeze artifact (Speed-of-Thought P0 contract §2,
docs/SPEED-OF-THOUGHT-P0-CONTRACT.md).

Canonical source: contracts/lld.v1.json. This module is a hand mirror of that JSON Schema (no
codegen step exists in this repo's build); the TypeScript mirror is
apps/mobile/src/build/lld-v1.ts. The two are kept honest by tests/test_lld_schema_conformance.py
(this unit) and apps/mobile/src/build/lld-v1.test.ts, which run both validators against the same
two fixtures on every change.

§2.1: ModuleBrief is the only object a proposer (the decomposer, the dialogue, an LLM) may
construct. It carries no content_hash, freeze_id, depth_score, stamped_by, state, or version —
those fields are absent by design, not merely optional. `ModuleBrief`'s pydantic config forbids
extra fields, so any of those appearing on the input is a validation error, not a silently ignored
extra key (the same property apps/mobile/src/build/lld-v1.ts's U1-T3 proves for the TS mirror).

`validate_module_brief` checks *shape* only (types, required fields, formats, the array-length
floors stated directly in the type docstrings below). It does not implement the lld-ready gate's
business-rule checks (contract §3 — C1-OPEN, C2-OWNER, R17-DERIV, etc.); that gate is a separate
unit (U3) and runs only on briefs that are already schema-valid.
"""

from __future__ import annotations

import hashlib
import json
import re
from typing import Annotated, Literal, Union

from pydantic import BaseModel, ConfigDict, Field, model_validator

LLD_SCHEMA_VERSION = "1.0"

Grain = Literal["module", "leaf"]
GuaranteeLabel = Literal["kills_structural", "kills_mechanical", "mitigates"]
OracleKind = Literal["test", "property", "metric"]

_NODE_ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]{2,63}$")
_NUMBER_CALC_OPERATOR_RE = re.compile(r"[+\-*/÷×=≈%]")
_STRUCTURAL_ENFORCED_BY_RE = re.compile(r"^(type:|gate:|[A-Za-z0-9_\-/.]+\.(ts|tsx|py|rs|json)(:[0-9]+)?$)")


class _Strict(BaseModel):
    """Every lld.v1 shape is a closed object: an unknown field is a schema violation, matching the
    TS mirror's forbidden-field check for ModuleBrief and the "no proposer-writable field" rule.
    """

    model_config = ConfigDict(extra="forbid")


class InstallVerdict(_Strict):
    kind: Literal["install"]
    matched_path: Annotated[str, Field(min_length=1)]


class ExtractVerdict(_Strict):
    kind: Literal["extract"]
    matched_path: Annotated[str, Field(min_length=1)]


class BuildNewVerdict(_Strict):
    kind: Literal["build_new"]
    searched: Annotated[list[Annotated[str, Field(min_length=1)]], Field(min_length=1)]  # MUST be non-empty


RegistryVerdict = Annotated[
    Union[InstallVerdict, ExtractVerdict, BuildNewVerdict],
    Field(discriminator="kind"),
]


class NumberDerivation(_Strict):
    kind: Literal["number"]
    value: str
    calc: str
    source: str | None = None

    @model_validator(mode="after")
    def _calc_carries_working(self) -> "NumberDerivation":
        # R17: a numeric claim must show its arithmetic, not just assert a value.
        if not self.calc.strip() or not re.search(r"[0-9]", self.calc) or not _NUMBER_CALC_OPERATOR_RE.search(self.calc):
            raise ValueError("a numeric derivation needs non-empty arithmetic (a digit and an operator) in calc")
        return self


class StructuralDerivation(_Strict):
    kind: Literal["structural"]
    invariant: Annotated[str, Field(min_length=1)]
    enforced_by: str

    @model_validator(mode="after")
    def _names_what_enforces_it(self) -> "StructuralDerivation":
        if not self.enforced_by.strip() or not _STRUCTURAL_ENFORCED_BY_RE.match(self.enforced_by):
            raise ValueError("a structural derivation must name a file, type:, or gate:")
        return self


Derivation = Annotated[Union[NumberDerivation, StructuralDerivation], Field(discriminator="kind")]


class Guarantee(_Strict):
    claim: Annotated[str, Field(min_length=1)]
    derivation: Derivation
    label: GuaranteeLabel


class KilledAlt(_Strict):
    option: Annotated[str, Field(min_length=1)]
    why_killed: Annotated[str, Field(min_length=1)]
    revive_trigger: Annotated[str, Field(min_length=1)]


class FailureStory(_Strict):
    trigger: Annotated[str, Field(min_length=10)]
    blast_radius: Annotated[str, Field(min_length=10)]
    fail_safe: Annotated[str, Field(min_length=10)]


class InterfaceDecl(_Strict):
    name: Annotated[str, Field(min_length=1)]
    signature: Annotated[str, Field(min_length=1)]


class DataDecl(_Strict):
    """owned_by_node MUST === the enclosing brief's node_id — checked cross-field in ModuleBrief,
    not expressible on this model alone.
    """

    store: Annotated[str, Field(min_length=1)]
    owned_by_node: Annotated[str, Field(min_length=1)]


class AcceptanceLine(_Strict):
    """artifact must start with owner_path, and `then` must contain the artifact substring
    (contract §3 C3-ACC-GROUND) — cross-field, checked by the lld-ready gate (U3), not here.
    """

    given: Annotated[str, Field(min_length=3)]
    when: Annotated[str, Field(min_length=3)]
    then: Annotated[str, Field(min_length=3)]
    oracle_kind: OracleKind
    artifact: Annotated[str, Field(min_length=1)]

    @model_validator(mode="after")
    def _non_blank_after_trim(self) -> "AcceptanceLine":
        if not self.given.strip() or not self.when.strip() or not self.then.strip():
            raise ValueError("given/when/then must be non-blank after trimming")
        return self


class ModuleBrief(_Strict):
    """The proposer-authored half. Everything here is content; none of it is authority (§2.1)."""

    node_id: Annotated[str, Field(pattern=_NODE_ID_RE.pattern)]
    grain: Grain
    purpose: Annotated[str, Field(min_length=1, max_length=200)]
    owner: Annotated[str, Field(min_length=1)]  # MUST resolve in contracts/owners.v1.json (gate C2)
    owner_path: Annotated[str, Field(min_length=1)]  # the ONE repo path prefix this module owns
    interface: Annotated[list[InterfaceDecl], Field(min_length=1)]
    data_owned: list[DataDecl]
    deps: list[Annotated[str, Field(min_length=1)]]  # other node_ids, interface-only
    registry: RegistryVerdict
    acceptance: AcceptanceLine
    non_goals: list[str]
    open_questions: list[str]  # MUST be [] to freeze
    guarantees: Annotated[list[Guarantee], Field(min_length=1)]
    alternatives: list[KilledAlt]  # >=2 at grain='module', >=1 at grain='leaf'
    failure_story: FailureStory

    @model_validator(mode="after")
    def _cross_field_invariants(self) -> "ModuleBrief":
        if ".." in self.owner_path:
            raise ValueError("owner_path must contain no '..'")
        for i, d in enumerate(self.data_owned):
            if d.owned_by_node != self.node_id:
                raise ValueError(f"data_owned[{i}].owned_by_node must equal the brief's own node_id")
        floor = 1 if self.grain == "leaf" else 2
        if len(self.alternatives) < floor:
            raise ValueError(f"alternatives must have at least {floor} entries at grain='{self.grain}'")
        return self


class DepthScore(_Strict):
    """REPORTED, NEVER THE PASS CRITERION — see LldReadyGate.ts (contract §3.4)."""

    checks_total: Annotated[int, Field(ge=0)]
    checks_passed: Annotated[int, Field(ge=0)]
    ratio: Annotated[float, Field(ge=0, le=1)]
    failed_check_ids: list[str]


class FreezeRecord(_Strict):
    """The gate-authored half. NO proposer-emittable form exists — see §2.1."""

    schema_version: Literal["1.0"]
    freeze_id: Annotated[str, Field(pattern=r"^fz-[0-9a-f]{16}$")]
    version: Annotated[int, Field(ge=1)]
    node_id: Annotated[str, Field(pattern=_NODE_ID_RE.pattern)]
    content_hash: Annotated[str, Field(pattern=r"^[0-9a-f]{64}$")]
    decision: Annotated[str, Field(min_length=1)]
    why: Annotated[str, Field(min_length=1)]
    killed_alternatives: list[KilledAlt]
    acceptance_test: AcceptanceLine
    owner: Annotated[str, Field(min_length=1)]
    depth_score: DepthScore
    supersedes: Annotated[str, Field(pattern=r"^fz-[0-9a-f]{16}$")] | None
    stamped_by: Literal["orb:lld-ready"]


def validate_module_brief(raw: dict) -> ModuleBrief | None:
    """The one call site for shape validation. Returns the parsed, validated ModuleBrief, or None
    on any schema violation — never a partially-trusted object. The lld-ready gate (U3) only ever
    runs on a value this function has already returned non-None for.
    """
    try:
        return ModuleBrief.model_validate(raw)
    except Exception:  # noqa: BLE001 - any pydantic ValidationError collapses to "invalid"
        return None


def canonical_json(value: object) -> str:
    """Deterministic JSON serialisation: object keys sorted recursively so the same content in a
    different key order produces byte-identical output. Array element order is preserved. Mirrors
    apps/mobile/src/build/lld-v1.ts's `canonicalJson` byte-for-byte (both walk dict/list the same
    way and both use `json.dumps`/`JSON.stringify`-equivalent scalar encoding).
    """
    if isinstance(value, list):
        return "[" + ",".join(canonical_json(v) for v in value) + "]"
    if isinstance(value, dict):
        keys = sorted(value.keys())
        body = ",".join(f"{json.dumps(k)}:{canonical_json(value[k])}" for k in keys)
        return "{" + body + "}"
    return json.dumps(value, ensure_ascii=False)


def content_hash(value: object) -> str:
    """SHA-256 of canonical_json(value), hex-encoded (64 chars). Uses the stdlib `hashlib`, no new
    dependency. The TypeScript mirror (`lld-v1.ts`'s `contentHash`) implements the same standard
    SHA-256 algorithm by hand (Metro/React Native has no `node:crypto`), so a hash computed on one
    side is bit-for-bit comparable to the other over the same canonical bytes.
    """
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()
