"""lld.v1 — the frozen decision-and-freeze artifact (F02 lane contract; blueprint `02` §2.2 /
`03` §4.1/§6).

Canonical source: fleet/contracts/{module-brief,freeze,lld}.v1.json (schema ownership moved
fleet-ward in F02 -- the lld-ready gate is fleet's, and the stamped_by const that defends against a
hand-authored freeze must not live with the proposer it defends against). This module is a hand
mirror of those three JSON Schemas (no codegen step exists in this repo's build); the TypeScript
mirror is apps/mobile/src/build/lld-v1.ts, and the Rust mirror is fleet/keel/fleet/src/lld.rs. The
three are kept honest not just by each one's own test file but by
fleet/tests/acceptance/lld-crosslang.sh, which diffs all three mirrors' reports against the same
fixture corpus (fleet/contracts/fixtures/lld/) -- the property nothing checked before F02 (each
mirror only ever validated fixtures against itself).

§2.1: ModuleBrief is the only object a proposer (the decomposer, the dialogue, an LLM) may
construct. It carries no content_hash, freeze_id, depth_evidence, stamped_by, state, or version --
those fields are absent by design, not merely optional. `ModuleBrief`'s pydantic config forbids
extra fields, so any of those appearing on the input is a validation error, not a silently ignored
extra key (the same property apps/mobile/src/build/lld-v1.ts's U1-T3 proves for the TS mirror).

`validate_module_brief` (pydantic-based) and `validate_module_brief_detailed` /
`validate_lld_v1_detailed` (hand-written, below) check *shape* only (types, required fields,
formats, the array-length floors stated directly in the type docstrings below). They do not
implement the lld-ready gate's business-rule checks (contract §3 -- C1-OPEN, C2-OWNER, R17-DERIV,
etc.); that gate is a separate unit (LldReadyGate.ts) and runs only on briefs that are already
schema-valid.
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Annotated, Literal, Union

from pydantic import BaseModel, ConfigDict, Field, model_validator

LLD_SCHEMA_VERSION = "1.0"

Grain = Literal["module", "leaf"]
GuaranteeLabel = Literal["kills_structural", "kills_mechanical", "mitigates"]
OracleKind = Literal["test", "property", "metric"]

_NODE_ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]{2,63}$")
_FREEZE_ID_RE = re.compile(r"^fz-[0-9a-f]{16}$")
_CONTENT_HASH_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
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
    """owned_by_node MUST === the enclosing brief's node_id -- checked cross-field in ModuleBrief,
    not expressible on this model alone.
    """

    store: Annotated[str, Field(min_length=1)]
    owned_by_node: Annotated[str, Field(min_length=1)]


class AcceptanceLine(_Strict):
    """artifact must start with owner_path, and `then` must contain the artifact substring
    (contract §3 C3-ACC-GROUND) -- cross-field, checked by the lld-ready gate, not here.
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

    schema_version: Literal["1.0"]
    node_id: Annotated[str, Field(pattern=_NODE_ID_RE.pattern)]
    grain: Grain
    purpose: Annotated[str, Field(min_length=1, max_length=200)]
    owner: Annotated[str, Field(min_length=1)]  # MUST resolve in fleet/contracts/owners.v1.json (gate C2)
    owner_path: Annotated[str, Field(min_length=1)]  # the ONE repo path prefix this module owns
    interface: Annotated[list[InterfaceDecl], Field(min_length=1)]
    data_owned: list[DataDecl]
    deps: list[Annotated[str, Field(min_length=1)]]  # other node_ids, interface-only
    registry: RegistryVerdict
    acceptance: AcceptanceLine
    non_goals: list[str]
    open_questions: list[str]  # MUST be [] to freeze
    guarantees: Annotated[list[Guarantee], Field(min_length=1)]
    alternatives: Annotated[list[KilledAlt], Field(min_length=2)]  # >=2, flat -- no grain exemption (F02 §3.4)
    failure_story: FailureStory

    @model_validator(mode="after")
    def _cross_field_invariants(self) -> "ModuleBrief":
        if ".." in self.owner_path:
            raise ValueError("owner_path must contain no '..'")
        for i, d in enumerate(self.data_owned):
            if d.owned_by_node != self.node_id:
                raise ValueError(f"data_owned[{i}].owned_by_node must equal the brief's own node_id")
        return self


class DepthScore(_Strict):
    """REPORTED, NEVER THE PASS CRITERION -- see LldReadyGate.ts (contract §3.4)."""

    checks_total: Annotated[int, Field(ge=0)]
    checks_passed: Annotated[int, Field(ge=0)]
    ratio: Annotated[float, Field(ge=0, le=1)]
    failed_check_ids: list[str]


class DepthEvidence(_Strict):
    """freeze.v1's depth_evidence (blueprint `03` §4.1's name: "the R17-R21 block the gate
    validated"). The only place a JSON number lives in this whole contract (§6.3) -- this is why
    the freeze itself is never hashed.
    """

    score: DepthScore
    checked_check_ids: list[str]


class Predicate(_Strict):
    """freeze.v1 / lld.v1 sow_seed's shared predicate shape -- distinct from AcceptanceLine: here
    `artifact` is a sibling of given/when/then/oracle_kind, not nested inside it (see AcceptsWhen).
    """

    given: Annotated[str, Field(min_length=3)]
    when: Annotated[str, Field(min_length=3)]
    then: Annotated[str, Field(min_length=3)]
    oracle_kind: OracleKind


class AcceptsWhen(_Strict):
    """Blueprint `03` §4.1's `accepts_when` / §6's `blind_suite_seed` -- reverts the orb's
    `acceptance_test` name back to the blueprint's. `03` §6 maps accepts_when.{predicate,artifact}
    by name into the blind-suite seed -- the field name is load-bearing, not cosmetic.
    """

    predicate: Predicate
    artifact: Annotated[str, Field(min_length=1)]


class Freeze(_Strict):
    """The gate-authored half (freeze.v1.json). NO proposer-emittable form exists -- see §2.1.

    Named `Freeze`, not `FreezeRecord` -- F02-T13 (this module's frozen acceptance test) imports
    this exact name.
    """

    schema_version: Literal["1.0"]
    freeze_id: Annotated[str, Field(pattern=_FREEZE_ID_RE.pattern)]
    version: Annotated[int, Field(ge=1)]
    node_id: Annotated[str, Field(pattern=_NODE_ID_RE.pattern)]  # MUST equal the sibling module_brief's node_id
    content_hash: Annotated[str, Field(pattern=_CONTENT_HASH_RE.pattern)]
    decision: Annotated[str, Field(min_length=1)]
    why: Annotated[str, Field(min_length=1)]
    killed_alternatives: Annotated[list[KilledAlt], Field(min_length=2)]  # F02 §4.2 -- the floor did not exist before F02
    accepts_when: AcceptsWhen  # renamed from the orb's `acceptance_test` (F02 §5.2)
    owner: Annotated[str, Field(min_length=1)]
    depth_evidence: DepthEvidence  # renamed from `depth_score` (F02 §5.2)
    supersedes: Annotated[str, Field(pattern=_FREEZE_ID_RE.pattern)] | None  # required-and-nullable
    stamped_by: Literal["keel:lld-ready"]  # const -- no proposer path writes this string (F02 §4.1)


class SowSeed(_Strict):
    """Blueprint `03` §6's SOW seed, five fields verbatim."""

    restatement: Annotated[str, Field(min_length=1)]
    blind_suite_seed: AcceptsWhen
    blast_radius: Annotated[str, Field(min_length=1)]
    owner: Annotated[str, Field(min_length=1)]
    registry_verdict: RegistryVerdict


class LldV1(_Strict):
    """The ONE thing that crosses the orb->fleet seam (blueprint `03` §6; F02 lane contract §4.3 --
    `lld.v1` used to denote four different shapes across the two blueprints and the code; it now
    denotes exactly this one, everywhere, in every language mirror). `module_brief` is added beyond
    `03` §6's literal {freeze, sow_seed} pair (F02 §5.3): a consumer holding only the freeze can
    never verify freeze.content_hash, since that hash is computed over the brief.
    """

    schema_version: Literal["1.0"]
    module_brief: ModuleBrief
    freeze: Freeze
    sow_seed: SowSeed

    @model_validator(mode="after")
    def _freeze_node_id_matches_brief(self) -> "LldV1":
        if self.freeze.node_id != self.module_brief.node_id:
            raise ValueError("freeze.node_id must equal the sibling module_brief.node_id")
        return self


def validate_module_brief(raw: dict) -> ModuleBrief | None:
    """The one call site for shape validation (pydantic-based). Returns the parsed, validated
    ModuleBrief, or None on any schema violation -- never a partially-trusted object. The
    lld-ready gate only ever runs on a value this function has already returned non-None for.
    """
    try:
        return ModuleBrief.model_validate(raw)
    except Exception:  # noqa: BLE001 - any pydantic ValidationError collapses to "invalid"
        return None


# ---------------------------------------------------------------------------------------------
# Hand-written "detailed" validators (F02) -- NOT derived from pydantic's own ValidationError
# shape. The cross-language comparator (fleet/tests/acceptance/lld-crosslang.sh) diffs error PATH
# SETS across the TS/Python/Rust mirrors for the same fixture, so this module's path-naming
# convention must match apps/mobile/src/build/lld-v1.ts's hand-written validator byte-for-byte
# (e.g. "alternatives[0]", "guarantees[0].derivation.calc"). Relying on pydantic's automatic
# `.errors()` `loc` tuples (which have their own, framework-specific conventions for discriminated
# unions) would not give that control, so this walks the same shape by hand, mirroring the TS
# validator's own logic and branching order.
# ---------------------------------------------------------------------------------------------


@dataclass
class ValidationErrorDetail:
    path: str
    message: str


@dataclass
class ValidationResult:
    ok: bool
    errors: list[ValidationErrorDetail] = field(default_factory=list)


def _is_plain_object(v: object) -> bool:
    return isinstance(v, dict)


def _is_non_empty_str(v: object) -> bool:
    return isinstance(v, str) and len(v.strip()) > 0


def _is_string_array(v: object) -> bool:
    return isinstance(v, list) and all(isinstance(x, str) for x in v)


# Mirrors apps/mobile/src/build/lld-v1.ts's FORBIDDEN_MODULE_BRIEF_FIELDS / ALLOWED_MODULE_BRIEF_FIELDS.
_FORBIDDEN_MODULE_BRIEF_FIELDS = ("content_hash", "freeze_id", "depth_evidence", "depth_score", "stamped_by", "state", "version")
_ALLOWED_MODULE_BRIEF_FIELDS = (
    "schema_version", "node_id", "grain", "purpose", "owner", "owner_path", "interface", "data_owned",
    "deps", "registry", "acceptance", "non_goals", "open_questions", "guarantees", "alternatives", "failure_story",
)
_ALLOWED_FREEZE_FIELDS = (
    "schema_version", "freeze_id", "version", "node_id", "content_hash", "decision", "why",
    "killed_alternatives", "accepts_when", "owner", "depth_evidence", "supersedes", "stamped_by",
)
_ALLOWED_SOW_SEED_FIELDS = ("restatement", "blind_suite_seed", "blast_radius", "owner", "registry_verdict")
_ALLOWED_LLD_V1_FIELDS = ("schema_version", "module_brief", "freeze", "sow_seed")


def validate_module_brief_detailed(raw: object) -> ValidationResult:
    """Hand-written mirror of module-brief.v1.json, collecting every violation (not just the
    first) so the cross-language comparator can diff full error-path SETS, matching
    `validateModuleBrief` in apps/mobile/src/build/lld-v1.ts field-for-field.
    """
    errors: list[ValidationErrorDetail] = []

    def fail(path: str, message: str) -> None:
        errors.append(ValidationErrorDetail(path, message))

    if not _is_plain_object(raw):
        return ValidationResult(False, [ValidationErrorDetail("", "ModuleBrief must be a JSON object")])
    b = raw

    for key in b.keys():
        if key not in _ALLOWED_MODULE_BRIEF_FIELDS:
            if key in _FORBIDDEN_MODULE_BRIEF_FIELDS:
                fail(key, f'"{key}" is gate-authored (Freeze-only) and must not appear on a ModuleBrief')
            else:
                fail(key, f'unexpected property "{key}" (additionalProperties: false)')

    if b.get("schema_version") != LLD_SCHEMA_VERSION:
        fail("schema_version", f'schema_version must be "{LLD_SCHEMA_VERSION}"')
    node_id = b.get("node_id")
    if not _is_non_empty_str(node_id) or not _NODE_ID_RE.match(node_id):
        fail("node_id", "node_id must match ^[a-z0-9][a-z0-9-]{2,63}$")
    if b.get("grain") not in ("module", "leaf"):
        fail("grain", 'grain must be "module" or "leaf"')
    purpose = b.get("purpose")
    if not isinstance(purpose, str) or len(purpose) < 1 or len(purpose) > 200:
        fail("purpose", "purpose must be a string of 1..200 characters")
    if not _is_non_empty_str(b.get("owner")):
        fail("owner", "owner must be a non-empty string")
    owner_path = b.get("owner_path")
    if not _is_non_empty_str(owner_path) or ".." in owner_path:
        fail("owner_path", 'owner_path must be a non-empty path containing no ".."')

    iface = b.get("interface")
    if not isinstance(iface, list) or len(iface) < 1:
        fail("interface", "interface must be a non-empty array")
    else:
        for i, item in enumerate(iface):
            if not _is_plain_object(item) or not _is_non_empty_str(item.get("name")) or not _is_non_empty_str(item.get("signature")):
                fail(f"interface[{i}]", "each interface entry needs a non-empty name and signature")

    data_owned = b.get("data_owned")
    if not isinstance(data_owned, list):
        fail("data_owned", "data_owned must be an array")
    else:
        for i, item in enumerate(data_owned):
            if not _is_plain_object(item) or not _is_non_empty_str(item.get("store")) or not _is_non_empty_str(item.get("owned_by_node")):
                fail(f"data_owned[{i}]", "each data_owned entry needs a non-empty store and owned_by_node")
            elif item.get("owned_by_node") != node_id:
                fail(f"data_owned[{i}].owned_by_node", "owned_by_node must equal the brief's own node_id")

    if not _is_string_array(b.get("deps")):
        fail("deps", "deps must be an array of strings")

    registry = b.get("registry")
    if not _is_plain_object(registry) or not isinstance(registry.get("kind"), str):
        fail("registry", "registry must be a RegistryVerdict object")
    else:
        kind = registry.get("kind")
        if kind in ("install", "extract"):
            if not _is_non_empty_str(registry.get("matched_path")):
                fail("registry.matched_path", "matched_path must be a non-empty string")
        elif kind == "build_new":
            searched = registry.get("searched")
            if not isinstance(searched, list) or len(searched) < 1 or not all(_is_non_empty_str(s) for s in searched):
                fail("registry.searched", "searched must be a non-empty array of non-empty strings")
        else:
            fail("registry.kind", "kind must be one of install | extract | build_new")

    acceptance = b.get("acceptance")
    if not _is_plain_object(acceptance):
        fail("acceptance", "acceptance must be an AcceptanceLine object")
    else:
        a = acceptance
        if not isinstance(a.get("given"), str) or len(a.get("given", "").strip()) < 3:
            fail("acceptance.given", "given must be >=3 non-blank characters")
        if not isinstance(a.get("when"), str) or len(a.get("when", "").strip()) < 3:
            fail("acceptance.when", "when must be >=3 non-blank characters")
        if not isinstance(a.get("then"), str) or len(a.get("then", "").strip()) < 3:
            fail("acceptance.then", "then must be >=3 non-blank characters")
        if a.get("oracle_kind") not in ("test", "property", "metric"):
            fail("acceptance.oracle_kind", "oracle_kind must be one of test | property | metric")
        if not _is_non_empty_str(a.get("artifact")):
            fail("acceptance.artifact", "artifact must be a non-empty string")

    if not _is_string_array(b.get("non_goals")):
        fail("non_goals", "non_goals must be an array of strings")
    if not _is_string_array(b.get("open_questions")):
        fail("open_questions", "open_questions must be an array of strings")

    guarantees = b.get("guarantees")
    if not isinstance(guarantees, list) or len(guarantees) < 1:
        fail("guarantees", "guarantees must be a non-empty array")
    else:
        for i, g in enumerate(guarantees):
            if not _is_plain_object(g) or not _is_non_empty_str(g.get("claim")):
                fail(f"guarantees[{i}].claim", "claim must be a non-empty string")
            if g.get("label") not in ("kills_structural", "kills_mechanical", "mitigates") if _is_plain_object(g) else True:
                fail(f"guarantees[{i}].label", "label must be one of kills_structural | kills_mechanical | mitigates")
            d = g.get("derivation") if _is_plain_object(g) else None
            if not _is_plain_object(d):
                fail(f"guarantees[{i}].derivation", "derivation must be a Derivation object")
            elif d.get("kind") == "number":
                calc = d.get("calc")
                if not isinstance(calc, str) or not calc.strip() or not re.search(r"[0-9]", calc) or not _NUMBER_CALC_OPERATOR_RE.search(calc):
                    fail(f"guarantees[{i}].derivation.calc", "a numeric derivation needs non-empty arithmetic (a digit and an operator)")
            elif d.get("kind") == "structural":
                enforced_by = d.get("enforced_by")
                if not isinstance(enforced_by, str) or not enforced_by.strip() or not _STRUCTURAL_ENFORCED_BY_RE.match(enforced_by):
                    fail(f"guarantees[{i}].derivation.enforced_by", "a structural derivation must name a file, type:, or gate:")
            else:
                fail(f"guarantees[{i}].derivation.kind", 'kind must be "number" or "structural"')

    alternatives = b.get("alternatives")
    if not isinstance(alternatives, list) or len(alternatives) < 2:
        fail("alternatives", "alternatives must have at least 2 entries (flat floor -- no grain exemption, F02 §3.4)")
    else:
        for i, alt in enumerate(alternatives):
            if not _is_plain_object(alt) or not _is_non_empty_str(alt.get("option")) or not _is_non_empty_str(alt.get("why_killed")) or not _is_non_empty_str(alt.get("revive_trigger")):
                fail(f"alternatives[{i}]", "each alternative needs a non-empty option, why_killed, and revive_trigger")

    failure_story = b.get("failure_story")
    if not _is_plain_object(failure_story):
        fail("failure_story", "failure_story must be a FailureStory object")
    else:
        f_ = failure_story
        if not isinstance(f_.get("trigger"), str) or len(f_.get("trigger", "").strip()) < 10:
            fail("failure_story.trigger", "trigger must be >=10 non-blank characters")
        if not isinstance(f_.get("blast_radius"), str) or len(f_.get("blast_radius", "").strip()) < 10:
            fail("failure_story.blast_radius", "blast_radius must be >=10 non-blank characters")
        if not isinstance(f_.get("fail_safe"), str) or len(f_.get("fail_safe", "").strip()) < 10:
            fail("failure_story.fail_safe", "fail_safe must be >=10 non-blank characters")

    return ValidationResult(len(errors) == 0, errors)


def _check_accepts_when(v: object, path_prefix: str, errors: list[ValidationErrorDetail]) -> None:
    """Shared by freeze.accepts_when and sow_seed.blind_suite_seed (identical shape)."""
    if not _is_plain_object(v):
        errors.append(ValidationErrorDetail(path_prefix, "must be an AcceptsWhen object"))
        return
    for key in v.keys():
        if key not in ("predicate", "artifact"):
            errors.append(ValidationErrorDetail(f"{path_prefix}.{key}", f'unexpected property "{key}" (additionalProperties: false)'))
    p = v.get("predicate")
    if not _is_plain_object(p):
        errors.append(ValidationErrorDetail(f"{path_prefix}.predicate", "predicate must be an object"))
    else:
        for key in p.keys():
            if key not in ("given", "when", "then", "oracle_kind"):
                errors.append(ValidationErrorDetail(f"{path_prefix}.predicate.{key}", f'unexpected property "{key}" (additionalProperties: false)'))
        if not isinstance(p.get("given"), str) or len(p.get("given", "").strip()) < 3:
            errors.append(ValidationErrorDetail(f"{path_prefix}.predicate.given", "given must be >=3 non-blank characters"))
        if not isinstance(p.get("when"), str) or len(p.get("when", "").strip()) < 3:
            errors.append(ValidationErrorDetail(f"{path_prefix}.predicate.when", "when must be >=3 non-blank characters"))
        if not isinstance(p.get("then"), str) or len(p.get("then", "").strip()) < 3:
            errors.append(ValidationErrorDetail(f"{path_prefix}.predicate.then", "then must be >=3 non-blank characters"))
        if p.get("oracle_kind") not in ("test", "property", "metric"):
            errors.append(ValidationErrorDetail(f"{path_prefix}.predicate.oracle_kind", "oracle_kind must be one of test | property | metric"))
    if not _is_non_empty_str(v.get("artifact")):
        errors.append(ValidationErrorDetail(f"{path_prefix}.artifact", "artifact must be a non-empty string"))


def _check_registry_verdict(v: object, path_prefix: str, errors: list[ValidationErrorDetail]) -> None:
    """Shared by module_brief.registry and sow_seed.registry_verdict (identical shape)."""
    if not _is_plain_object(v) or not isinstance(v.get("kind"), str):
        errors.append(ValidationErrorDetail(path_prefix, "must be a RegistryVerdict object"))
        return
    kind = v.get("kind")
    if kind in ("install", "extract"):
        if not _is_non_empty_str(v.get("matched_path")):
            errors.append(ValidationErrorDetail(f"{path_prefix}.matched_path", "matched_path must be a non-empty string"))
    elif kind == "build_new":
        searched = v.get("searched")
        if not isinstance(searched, list) or len(searched) < 1 or not all(_is_non_empty_str(s) for s in searched):
            errors.append(ValidationErrorDetail(f"{path_prefix}.searched", "searched must be a non-empty array of non-empty strings"))
    else:
        errors.append(ValidationErrorDetail(f"{path_prefix}.kind", "kind must be one of install | extract | build_new"))


def _check_freeze(v: object, errors: list[ValidationErrorDetail]) -> None:
    if not _is_plain_object(v):
        errors.append(ValidationErrorDetail("freeze", "freeze must be a Freeze object"))
        return
    for key in v.keys():
        if key not in _ALLOWED_FREEZE_FIELDS:
            errors.append(ValidationErrorDetail(f"freeze.{key}", f'unexpected property "{key}" (additionalProperties: false)'))
    if v.get("schema_version") != LLD_SCHEMA_VERSION:
        errors.append(ValidationErrorDetail("freeze.schema_version", f'schema_version must be "{LLD_SCHEMA_VERSION}"'))
    freeze_id = v.get("freeze_id")
    if not _is_non_empty_str(freeze_id) or not _FREEZE_ID_RE.match(freeze_id):
        errors.append(ValidationErrorDetail("freeze.freeze_id", "freeze_id must match ^fz-[0-9a-f]{16}$"))
    version = v.get("version")
    if not isinstance(version, int) or isinstance(version, bool) or version < 1:
        errors.append(ValidationErrorDetail("freeze.version", "version must be an integer >= 1"))
    node_id = v.get("node_id")
    if not _is_non_empty_str(node_id) or not _NODE_ID_RE.match(node_id):
        errors.append(ValidationErrorDetail("freeze.node_id", "node_id must match ^[a-z0-9][a-z0-9-]{2,63}$"))
    content_hash_val = v.get("content_hash")
    if not _is_non_empty_str(content_hash_val) or not _CONTENT_HASH_RE.match(content_hash_val):
        errors.append(ValidationErrorDetail("freeze.content_hash", "content_hash must match ^sha256:[0-9a-f]{64}$"))
    if not _is_non_empty_str(v.get("decision")):
        errors.append(ValidationErrorDetail("freeze.decision", "decision must be a non-empty string"))
    if not _is_non_empty_str(v.get("why")):
        errors.append(ValidationErrorDetail("freeze.why", "why must be a non-empty string"))
    killed = v.get("killed_alternatives")
    if not isinstance(killed, list) or len(killed) < 2:
        errors.append(ValidationErrorDetail("freeze.killed_alternatives", "killed_alternatives must have at least 2 entries"))
    else:
        for i, alt in enumerate(killed):
            if not _is_plain_object(alt) or not _is_non_empty_str(alt.get("option")) or not _is_non_empty_str(alt.get("why_killed")) or not _is_non_empty_str(alt.get("revive_trigger")):
                errors.append(ValidationErrorDetail(f"freeze.killed_alternatives[{i}]", "each killed_alternatives entry needs a non-empty option, why_killed, and revive_trigger"))
    _check_accepts_when(v.get("accepts_when"), "freeze.accepts_when", errors)
    if not _is_non_empty_str(v.get("owner")):
        errors.append(ValidationErrorDetail("freeze.owner", "owner must be a non-empty string"))
    de = v.get("depth_evidence")
    if not _is_plain_object(de):
        errors.append(ValidationErrorDetail("freeze.depth_evidence", "depth_evidence must be a DepthEvidence object"))
    else:
        for key in de.keys():
            if key not in ("score", "checked_check_ids"):
                errors.append(ValidationErrorDetail(f"freeze.depth_evidence.{key}", f'unexpected property "{key}" (additionalProperties: false)'))
        score = de.get("score")
        if not _is_plain_object(score):
            errors.append(ValidationErrorDetail("freeze.depth_evidence.score", "score must be a DepthScore object"))
        else:
            checks_total = score.get("checks_total")
            if not isinstance(checks_total, int) or isinstance(checks_total, bool) or checks_total < 0:
                errors.append(ValidationErrorDetail("freeze.depth_evidence.score.checks_total", "checks_total must be an integer >= 0"))
            checks_passed = score.get("checks_passed")
            if not isinstance(checks_passed, int) or isinstance(checks_passed, bool) or checks_passed < 0:
                errors.append(ValidationErrorDetail("freeze.depth_evidence.score.checks_passed", "checks_passed must be an integer >= 0"))
            ratio = score.get("ratio")
            if not isinstance(ratio, (int, float)) or isinstance(ratio, bool) or ratio < 0 or ratio > 1:
                errors.append(ValidationErrorDetail("freeze.depth_evidence.score.ratio", "ratio must be a number in [0,1]"))
            if not _is_string_array(score.get("failed_check_ids")):
                errors.append(ValidationErrorDetail("freeze.depth_evidence.score.failed_check_ids", "failed_check_ids must be an array of strings"))
        if not _is_string_array(de.get("checked_check_ids")):
            errors.append(ValidationErrorDetail("freeze.depth_evidence.checked_check_ids", "checked_check_ids must be an array of strings"))
    supersedes = v.get("supersedes")
    if supersedes is not None and (not _is_non_empty_str(supersedes) or not _FREEZE_ID_RE.match(supersedes)):
        errors.append(ValidationErrorDetail("freeze.supersedes", "supersedes must be null or match ^fz-[0-9a-f]{16}$"))
    # The one runtime comparison against the gate's const (F02 §4.1).
    if v.get("stamped_by") != "keel:lld-ready":
        errors.append(ValidationErrorDetail("freeze.stamped_by", "stamped_by must equal the gate's const (only the gate may write it)"))


def _check_sow_seed(v: object, errors: list[ValidationErrorDetail]) -> None:
    if not _is_plain_object(v):
        errors.append(ValidationErrorDetail("sow_seed", "sow_seed must be a SowSeed object"))
        return
    for key in v.keys():
        if key not in _ALLOWED_SOW_SEED_FIELDS:
            errors.append(ValidationErrorDetail(f"sow_seed.{key}", f'unexpected property "{key}" (additionalProperties: false)'))
    if not _is_non_empty_str(v.get("restatement")):
        errors.append(ValidationErrorDetail("sow_seed.restatement", "restatement must be a non-empty string"))
    _check_accepts_when(v.get("blind_suite_seed"), "sow_seed.blind_suite_seed", errors)
    if not _is_non_empty_str(v.get("blast_radius")):
        errors.append(ValidationErrorDetail("sow_seed.blast_radius", "blast_radius must be a non-empty string"))
    if not _is_non_empty_str(v.get("owner")):
        errors.append(ValidationErrorDetail("sow_seed.owner", "owner must be a non-empty string"))
    _check_registry_verdict(v.get("registry_verdict"), "sow_seed.registry_verdict", errors)


def validate_lld_v1_detailed(raw: object) -> ValidationResult:
    """Hand-written mirror of lld.v1.json, the wrapper that is the ONLY thing crossing the
    orb->fleet seam (blueprint `03` §6; F02 §5.3). Delegates to `validate_module_brief_detailed`
    for `module_brief` (paths re-prefixed `module_brief.`), and hand-written freeze/sow_seed checks
    matching the same path-naming convention the TS and Rust mirrors use.
    """
    errors: list[ValidationErrorDetail] = []
    if not _is_plain_object(raw):
        return ValidationResult(False, [ValidationErrorDetail("", "lld.v1 must be a JSON object")])

    for key in raw.keys():
        if key not in _ALLOWED_LLD_V1_FIELDS:
            errors.append(ValidationErrorDetail(key, f'unexpected property "{key}" (additionalProperties: false)'))
    if raw.get("schema_version") != LLD_SCHEMA_VERSION:
        errors.append(ValidationErrorDetail("schema_version", f'schema_version must be "{LLD_SCHEMA_VERSION}"'))

    brief_result = validate_module_brief_detailed(raw.get("module_brief"))
    for e in brief_result.errors:
        errors.append(ValidationErrorDetail(f"module_brief.{e.path}" if e.path else "module_brief", e.message))

    _check_freeze(raw.get("freeze"), errors)
    _check_sow_seed(raw.get("sow_seed"), errors)

    module_brief = raw.get("module_brief")
    freeze = raw.get("freeze")
    if _is_plain_object(module_brief) and _is_plain_object(freeze):
        brief_node_id = module_brief.get("node_id")
        freeze_node_id = freeze.get("node_id")
        if isinstance(brief_node_id, str) and isinstance(freeze_node_id, str) and brief_node_id != freeze_node_id:
            errors.append(ValidationErrorDetail("freeze.node_id", "freeze.node_id must equal the sibling module_brief.node_id"))

    return ValidationResult(len(errors) == 0, errors)


# ---------------------------------------------------------------------------------------------
# Canonical JSON + content hash (§6.2: "content_hash = sha256(canonical_json(module_brief)), the
# brief and nothing else -- ever").
# ---------------------------------------------------------------------------------------------


def canonical_json(value: object) -> str:
    """Deterministic JSON serialisation: object keys sorted recursively so the same content in a
    different key order produces byte-identical output. Array element order is preserved. Mirrors
    apps/mobile/src/build/lld-v1.ts's `canonicalJson` byte-for-byte (both walk dict/list the same
    way and both use `json.dumps`/`JSON.stringify`-equivalent scalar encoding).

    REFUSES any JSON number, at any depth (F02 §6.3) -- json.dumps(1.0) == "1.0" in Python vs
    JSON.stringify(1.0) === "1" in JS vs serde_json rendering 1.0f64 as "1.0" in Rust. `bool` is
    checked BEFORE `(int, float)` -- Python's `bool` is a subclass of `int`, so `isinstance(True,
    int)` is true, and checking numeric-ness first would wrongly refuse every boolean too.
    """
    if isinstance(value, list):
        return "[" + ",".join(canonical_json(v) for v in value) + "]"
    if isinstance(value, dict):
        keys = sorted(value.keys())
        body = ",".join(f"{json.dumps(k)}:{canonical_json(value[k])}" for k in keys)
        return "{" + body + "}"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        raise TypeError("numbers are not canonicalizable (F02 §6.3)")
    return json.dumps(value, ensure_ascii=False)


def _hash_canonical_string(canonical: str) -> str:
    return "sha256:" + hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def content_hash(value: object) -> str:
    """"sha256:" + hex(sha256(canonical_json(value))) (F02 §4.4 -- the estate runs two 256-bit hex
    hashes; an unlabelled ^[0-9a-f]{64}$ pattern already produced one wrong alg/digest mapping in
    blueprint `03` §6's hand-off table). Uses the stdlib `hashlib`, no new dependency. Raises (via
    `canonical_json`) on any numeric leaf, same as `canonical_json` itself.
    """
    return _hash_canonical_string(canonical_json(value))


# ---------------------------------------------------------------------------------------------
# Cross-language report (F02 §7.4) -- consumed by fleet/tests/acceptance/lld-crosslang.sh, which
# diffs this mirror's report against the TS and Rust mirrors' reports for the same fixture corpus.
# ---------------------------------------------------------------------------------------------

# The one literal string every mirror's report uses for a numeric-leaf refusal (F02 §6.3). Fixed
# and language-agnostic (no dynamic path interpolation) so the three reports are byte-identical for
# this field regardless of which specific numeric leaf a given canonicalizer meets first.
_NUMERIC_REFUSAL_MESSAGE = "numbers are not canonicalizable (F02 §6.3)"


def cross_lang_report(fixtures_dir: str | Path) -> dict:
    """Builds this mirror's cross-language report over every *.json fixture in `fixtures_dir`.
    Dict keys are inserted in sorted order at every level (top-level contract/fixtures/mirror; each
    fixture entry's canonical_json/content_hash/error_paths/hash_refusal/valid; and the fixture
    names themselves) so `json.dumps(report, indent=2, sort_keys=False)` -- keys are ALREADY sorted
    on insertion, so `sort_keys` need not be set -- is byte-identical to the TS mirror's
    `JSON.stringify(report, null, 2)` and the Rust mirror's `serde_json::to_string_pretty` (whose
    default `Map` is a `BTreeMap` and sorts automatically). This is the whole mechanism the
    comparator relies on, not a stylistic choice.
    """
    fixtures_path = Path(fixtures_dir)
    names = sorted(p.stem for p in fixtures_path.iterdir() if p.suffix == ".json")

    fixtures: dict[str, dict] = {}
    for name in names:
        raw = json.loads((fixtures_path / f"{name}.json").read_text())
        is_wrapper = _is_plain_object(raw) and "module_brief" in raw
        validation = validate_lld_v1_detailed(raw) if is_wrapper else validate_module_brief_detailed(raw)
        brief_payload = raw.get("module_brief") if is_wrapper else raw

        canonical_json_val: str | None
        content_hash_val: str | None
        hash_refusal_val: str | None
        try:
            canonical_json_val = canonical_json(brief_payload)
            content_hash_val = _hash_canonical_string(canonical_json_val)
            hash_refusal_val = None
        except TypeError:
            canonical_json_val = None
            content_hash_val = None
            hash_refusal_val = _NUMERIC_REFUSAL_MESSAGE

        error_paths = sorted({e.path for e in validation.errors})
        fixtures[name] = {
            "canonical_json": canonical_json_val,
            "content_hash": content_hash_val,
            "error_paths": error_paths,
            "hash_refusal": hash_refusal_val,
            "valid": validation.ok,
        }

    return {"contract": "lld.v1", "fixtures": fixtures, "mirror": "py"}
