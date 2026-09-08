# BLUEPRINT — `fleet-crew`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-crew`
- **One-line purpose:** Drive an operator's already-authenticated `claude`/`codex` CLI as a
  keyless, non-interactive subprocess with closed stdin and file-backed evidence, probe each
  adapter's six-capability contract, and — as the same package's intent layer — construct and
  validate Statements of Work, mirror the ledger/attestation/submission JSON contracts as strict
  Pydantic models, and run the prompter-parity TOST experiment.
- **Build branch:** `package` — the ONE Python package in the roster (MIGRATION-PLAN §3 row 14:
  `fleet/crew/` — keyless CLI adapters; package as a crate-dir Python pkg). Everything named in
  this blueprint already exists in `fleet/crew/` and already has a green `pytest` suite; this is a
  near-verbatim directory move with one hard change forced by the workspace-wide rule: `sow.py`
  (579 lines) and `parity.py` (311 lines) must be split into ≤80-line modules, and `base.py` (90)
  and `capability.py` (94) are already over the line and need the same treatment — see §8.
- **Imports:** `none` — this package imports no other `fleet-*` crate. It has no Rust dependency at
  all (pure Python + pydantic/numpy/statsmodels) and constructs no `fleet-types` value; the JSON
  contracts it mirrors (`schema.py`) are validated independently against the same
  `fleet/contracts/*.v1.json` files fleet-types mirrors on the Rust side, not against fleet-types
  itself — there is no cross-language import path, only "same JSON contract, two independent
  mirrors" (a fact worth a shared contract-fixture test, not a code dependency; see §9).
- **Imported by:** `fleet-worker` — but **only as a subprocess dependency, not a compile edge**
  (MIGRATION-PLAN §3: "`fleet-crew` is a runtime dependency of `fleet-worker` (subprocess), not a
  compile edge"). `fleet-worker`'s Rust code shells out to `python3 -m crew.adapters...` (or an
  equivalent CLI entry point this blueprint does not currently define — see the divergence note)
  the same way `route.rs`'s `adapter_contract`/`installed_non_interactive` shell out today; it
  never `import`s this package as a library at compile time because it is Rust.

## 2. Responsibility & non-goals

**Owns:** the operator-keyless model-invocation boundary — the `Adapter` protocol contract (six
capabilities: non-interactive invocation, file-backed output/exit-code, closed stdin,
resolved-model readback, local usage accounting, operator-owned credentials), its two concrete
implementations (`ClaudeAdapter`, `CodexAdapter`), the shared subprocess mechanics that make a
"successful" invocation auditable (log-size floor, non-empty diff requirement, closed-stdin
enforcement), and capability probing (`probe_adapter`/`CapabilityReport`) that turns those six
facts into a `builder_eligible`/`verifier_eligible` verdict without ever making a model call. In
the same package (today's `fleet/crew/` layout, not split across two crates — see the divergence
note on why this blueprint keeps them together): the intent layer (`build_sow`/`validate_sow` and
the `Sow`/`AtomicLeaf`/`Challenge`/`Clarification`/`Alternative` shapes, which encode "one
acceptance predicate per atomic leaf" and "every challenge/citation must resolve to a real
file:line" as refusals, not warnings), the strict Pydantic mirrors of the three versioned JSON
contracts (`attestation.v1.json`, `receipt.v1.json`, `submission.v1.json`), and the prompter-parity
Welch's-TOST equivalence experiment (`analyze_parity`).

**Non-goals (the seam):**
- Does **not** decide which adapter/model a task gets — that is `fleet-router`'s `decide()`
  (already Opus-approved). This crate only answers "can this adapter serve as builder/verifier"
  (`probe_adapter`), never "which one should".
- Does **not** append to the ledger, write a receipt, or persist an attestation — `schema.py`'s
  `Receipt`/`Attestation`/`Submission` are validation-only mirrors; the actual append-only
  blake3-chained write is `fleet-store`'s job (today's `main.rs::append_receipt`). A caller
  constructs a `Receipt(**payload)` to validate shape before handing the payload to the real ledger
  writer; this crate never opens the ledger file itself.
- Does **not** manage worktree isolation, lane leasing, or the fd-3 protocol — those are
  `fleet-merge`/`fleet-worker`'s job. This crate's adapters take a `cwd`/`env` the caller already
  prepared; they never create or tear down a worktree.
- Does **not** measure token quota or apply cooldowns — `UsageRecord` is "what one invocation says
  it used, read from its own transcript," never a running balance; balance tracking is
  `fleet-govern`'s job (today's `meter.rs`).
- Does **not** own the JSON contract files themselves (`fleet/contracts/*.v1.json` stay the source
  of truth) — `schema.py`'s models are a mirror that must change in the same commit as the
  contract, per PLAYBOOK.md rule 1; this crate never generates or writes a `.v1.json` file.
- Does **not** invent a `GeminiAdapter`. The build-briefing prompt for this crate names
  "claude/codex/gemini," but no `gemini` CLI adapter, command shape, or output-metadata convention
  exists anywhere in `fleet/` today (repo-wide grep found zero hits outside unrelated PDF-build
  scripts). Fabricating one without evidence would violate PLAYBOOK.md rule 2 ("ground every claim
  in real source") and the L8 rule against inventing checks that are cheaper to fake than to
  satisfy. See the divergence note — this is flagged for Opus, not silently dropped or silently
  invented.

## 3. Public API contract

> Real Python, importable as written (bodies below are either the existing implementation,
> unchanged, or `...` where the existing body is being moved verbatim into a new file location —
> never `pass`/bare `Exception`). Every fallible operation raises a **typed** exception subclass
> carrying an `exit_code: int` class attribute (this package's error-enum equivalent — Python has
> no `enum`-dispatched `Result`, so a dedicated exception type per failure family, never a bare
> `ValueError`/`Exception`/`assert`, is the rule here) — `AdapterError`, `SOWRefusal`,
> `ParityRefusal` already follow this; keep it.

```python
# ---------------------------------------------------------------------------
# crew/adapters/protocol.py, errors.py, records.py -- fleet/crew/crew/adapters/base.py:1-90
# ---------------------------------------------------------------------------
from __future__ import annotations
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Protocol, Sequence, runtime_checkable


class AdapterError(RuntimeError):
    """A model invocation failed an adapter invariant (log-size floor, empty diff, ...)."""
    exit_code: int = 6


@dataclass(frozen=True)
class UsageRecord:
    """Local, integer-only usage accounting emitted by one adapter invocation.

    Every field is `None` (unmeasured) or a non-negative `int` -- never a float, never negative.
    `__post_init__` raises `ValueError` (unchanged from today) for any other shape; this is the one
    place in the package that does NOT use a typed exception, because it fires only on a
    programmer error (a caller constructing `UsageRecord` with a bad literal), never on adapter
    output -- adapter-output parsing (`read_metadata`) already discards non-conforming values
    rather than constructing an invalid `UsageRecord` (see §6).
    """
    input_tokens: int | None = None
    output_tokens: int | None = None
    total_tokens: int | None = None
    def __post_init__(self) -> None: ...


@dataclass(frozen=True)
class InvocationResult:
    """Evidence from one non-interactive model invocation -- always file-backed."""
    returncode: int
    stdout_path: Path
    stderr_path: Path
    resolved_model: str | None
    usage: UsageRecord | None
    @property
    def ok(self) -> bool: ...
    @property
    def exit_code(self) -> int: ...


@runtime_checkable
class Adapter(Protocol):
    """Portability contract every model adapter in this package satisfies.

    The six capabilities `probe_adapter` (below) checks are represented here directly:
    non-interactive invocation, file-backed output/exit-code, closed stdin, resolved-model
    readback, local usage, operator-owned credentials.
    """
    name: str
    def invoke(
        self, prompt: str, *, stdout_path: Path, stderr_path: Path,
        cwd: Path | None = None, env: Mapping[str, str] | None = None,
        diff_path: Path | None = None,
    ) -> InvocationResult:
        """Run one model request with stdin connected to `/dev/null`. Never blocks on input."""
        ...
    def resolved_model(self) -> str | None:
        """The model the operator's CLI actually served, read from ITS OWN transcript -- never
        the requested alias echoed back (see §6's "already-exists" row for why this distinction
        is load-bearing, not decorative)."""
        ...
    def usage_record(self) -> UsageRecord | None: ...
    def operator_credentials(self) -> bool:
        """Whether credentials are sourced from the operator's own environment (keyless — this
        package never holds, requests, or transmits an API key)."""
        ...
    def command(self, prompt: str) -> Sequence[str]:
        """Build the non-interactive argv without executing it (testable without a subprocess)."""
        ...


# ---------------------------------------------------------------------------
# crew/adapters/metadata.py, runner.py -- fleet/crew/crew/adapters/_subprocess.py:1-134
# ---------------------------------------------------------------------------
from typing import Any

DEFAULT_LOG_SIZE_FLOOR: int = 40

def read_metadata(path: Path) -> tuple[str | None, UsageRecord | None]:
    """Parse optional `resolved_model`/`usage` JSON out of an invocation's own stdout transcript,
    without making a second model call. Never raises: a file that doesn't exist, isn't UTF-8, or
    contains no parseable JSON object yields `(None, None)`, not an error -- absence of metadata
    is an expected outcome for adapters that don't emit any."""
    ...

def run_model(
    command: Sequence[str], *, stdout_path: Path, stderr_path: Path,
    cwd: Path | None, env: Mapping[str, str] | None, resolved_model: str | None,
    usage: UsageRecord | None, diff_path: Path | None, log_size_floor: int,
) -> InvocationResult:
    """Execute `command` with stdin closed (`subprocess.DEVNULL`) and BOTH streams redirected to
    file handles (never a pipe an adapter's own logic could read back and forge). Raises
    `AdapterError` if a returncode-0 run's combined log is below `log_size_floor` bytes, or if
    `diff_path` is required but missing/empty after the run -- a "successful" invocation that
    changed nothing or logged nothing is refused, never reported as a pass."""
    ...


# ---------------------------------------------------------------------------
# crew/adapters/claude.py, codex.py -- fleet/crew/crew/adapters/{claude,codex}.py (unchanged
# behavior; both route through the shared `_cli_adapter.invoke_cli` helper introduced in §8 to
# remove the byte-for-byte duplicate `invoke()` body the two files carry today)
# ---------------------------------------------------------------------------
class ClaudeAdapter:
    name: str = "claude"
    supports_resolved_model_readback: bool = True
    def __init__(self, *, model: str | None = None, log_size_floor: int = DEFAULT_LOG_SIZE_FLOOR) -> None: ...
    def command(self, prompt: str) -> Sequence[str]: ...          # ["claude", "-p", ...prompt]
    def invoke(self, prompt: str, *, stdout_path: Path, stderr_path: Path,
               cwd: Path | None = None, env: Mapping[str, str] | None = None,
               diff_path: Path | None = None) -> InvocationResult: ...
    def resolved_model(self) -> str | None: ...
    def usage_record(self) -> UsageRecord | None: ...
    def operator_credentials(self) -> bool: ...                    # always True -- keyless contract

class CodexAdapter:
    name: str = "codex"
    supports_resolved_model_readback: bool = True
    def __init__(self, *, model: str | None = None, log_size_floor: int = DEFAULT_LOG_SIZE_FLOOR) -> None: ...
    def command(self, prompt: str) -> Sequence[str]: ...          # ["codex", "exec", ...prompt]
    def invoke(self, prompt: str, *, stdout_path: Path, stderr_path: Path,
               cwd: Path | None = None, env: Mapping[str, str] | None = None,
               diff_path: Path | None = None) -> InvocationResult: ...
    def resolved_model(self) -> str | None: ...
    def usage_record(self) -> UsageRecord | None: ...
    def operator_credentials(self) -> bool: ...


# ---------------------------------------------------------------------------
# crew/adapters/capability.py -- fleet/crew/crew/adapters/capability.py:1-94
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class CapabilityReport:
    adapter: str
    non_interactive_invoke: bool
    output_files_and_exit_code: bool
    closable_stdin: bool
    resolved_model_readback: bool
    local_usage_record: bool
    operator_credentials: bool
    @property
    def usable_as_builder(self) -> bool: ...
    @property
    def usable_as_verifier(self) -> bool: ...       # usable_as_builder AND resolved_model_readback
    @property
    def builder_eligible(self) -> bool: ...          # alias of usable_as_builder
    @property
    def verifier_eligible(self) -> bool: ...         # alias of usable_as_verifier
    @property
    def missing(self) -> tuple[str, ...]: ...

def probe_adapter(adapter: Adapter) -> CapabilityReport:
    """Inspect the six contract capabilities WITHOUT making a model call. `resolved_model_readback`
    is true only if the adapter declares `supports_resolved_model_readback = True` (a fresh,
    never-invoked adapter has no resolved identity yet -- this fn must never treat "hasn't served a
    request" as "cannot ever report one"; see §6)."""
    ...


# ---------------------------------------------------------------------------
# crew/sow/model.py, errors.py -- fleet/crew/crew/sow.py:1-145
# ---------------------------------------------------------------------------
class SOWRefusal(ValueError):
    """A mechanically detected intent defect (missing predicate, unresolvable citation, generic
    clarification, ...). `exit_code = 7` matches the repository-wide refusal convention."""
    exit_code: int = 7
    def __init__(self, reason: str) -> None: ...
    # self.reason: str; self.receipt: dict -- a ready-to-persist {"event": "refusal", ...} record

@dataclass(frozen=True)
class MachinePredicate:
    expression: str
    def __post_init__(self) -> None: ...             # raises SOWRefusal if expression is blank

@dataclass(frozen=True)
class AtomicLeaf:
    """Exactly one acceptance predicate per leaf -- never zero, never more than one."""
    id: str
    requirement: str
    predicates: tuple[MachinePredicate | str, ...]
    def __post_init__(self) -> None: ...             # raises SOWRefusal if count != 1
    @property
    def acceptance_predicates(self) -> tuple[MachinePredicate, ...]: ...
    @property
    def acceptance_predicate(self) -> MachinePredicate: ...

@dataclass(frozen=True)
class Challenge:
    text: str
    citation: str

@dataclass(frozen=True)
class Clarification:
    kind: str              # "technical" | "business" | ...
    question: str
    citation: str

@dataclass(frozen=True)
class Alternative:
    name: str
    approach: str
    tradeoff: str

@dataclass(frozen=True)
class Sow:
    restatement: tuple[str, ...]
    leaves: tuple[AtomicLeaf, ...]
    challenges: tuple[Challenge, ...]
    clarifications: tuple[Clarification, ...]
    alternatives: tuple[Alternative, ...]
    estimate: str
    edge_cases: tuple[str, ...]

def validate_sow(sow: Sow, *, repo_root: Path | None = None) -> Sow:
    """Re-check a constructed `Sow`'s citations resolve to real `path:line` targets under
    `repo_root` and that no clarification is generic (`_is_generic_question`). Raises `SOWRefusal`
    naming the first defect found; returns `sow` unchanged (identity) on success -- never mutates."""
    ...

def build_sow(task: str, *, repo_root: Path | None = None) -> Sow:
    """Parse a human/task string's fixed section convention (Challenges/Clarifications/
    Alternatives/Estimate/Edge cases) into a validated `Sow`. Raises `SOWRefusal` on any structural
    or citation defect -- never returns a partially-populated `Sow`."""
    ...

def main(argv: Sequence[str] | None = None) -> int:
    """`python -m crew.sow --task <text>` CLI entry point: prints the JSON payload on success (see
    `_sow_payload`), prints a refusal receipt to stderr and returns `SOWRefusal.exit_code` on
    failure. Never raises out of `main` itself."""
    ...


# ---------------------------------------------------------------------------
# crew/schema/*.py -- fleet/crew/crew/schema.py:1-134 (pydantic mirrors of fleet/contracts/*.v1.json)
# ---------------------------------------------------------------------------
from pydantic import BaseModel, ConfigDict, Field
from typing import Any, Literal

class ContractModel(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)

class Digest(ContractModel):
    blake3: str = Field(pattern=r"^[0-9a-f]{64}$")

class Subject(ContractModel):
    name: str
    digest: Digest

class AttestationElements(ContractModel):
    sow: dict[str, Any] | None = None
    blind_suite: dict[str, Any] | None = None
    independent_verification: dict[str, Any] | None = None
    adequacy: dict[str, Any] | None = None
    blast_radius: dict[str, Any] | None = None
    rollback: dict[str, Any] | None = None
    cost: dict[str, Any] | None = None
    oracle_independence: dict[str, Any] | None = None
    # field_validator reject_null_object: sow/blind_suite/independent_verification/blast_radius/cost
    # must be omitted, never explicit `null`, when absent -- unchanged from today.

class AttestationBuilder(ContractModel):
    id: str

class AttestationPredicate(ContractModel):
    tier: Literal["T-min", "T-std", "T-max"]
    elements: AttestationElements
    receipts: list[str]
    builder: AttestationBuilder

class Attestation(ContractModel):
    type_: Literal["https://in-toto.io/Statement/v1"] = Field(alias="_type")
    subject: list[Subject] = Field(min_length=1)
    predicate_type: Literal["https://fleet.local/DeliveryAttestation/v1"] = Field(alias="predicateType")
    predicate: AttestationPredicate

class Receipt(ContractModel):
    schema_version: Literal["1.0"]
    seq: int = Field(ge=0)
    prev_hash: str = Field(pattern=r"^(GENESIS|blake3:[0-9a-f]{64})$")
    hash: str = Field(pattern=r"^blake3:[0-9a-f]{64}$")
    ts_wall: str                              # validated ISO-8601 by field_validator, see §6
    event: Literal["run_start", "artifact_frozen", "attested", "refusal", "gate_verdict", "run_end"]
    actor: str
    resolved_model: str | None = None
    exit_code: int | None = None
    body: dict[str, Any]

class Submission(ContractModel):
    schema_version: Literal["1.0"]
    kind: Literal["note", "done", "refuse"]
    body: dict[str, Any]

DeliveryAttestation = Attestation
InTotoStatement = Attestation


# ---------------------------------------------------------------------------
# crew/parity/*.py -- fleet/crew/crew/parity.py:1-312
# ---------------------------------------------------------------------------
ALPHA: float = 0.05
MIN_TOTAL_N: int = 128
MIN_PER_PROMPTER: int = MIN_TOTAL_N // 2

class ParityRefusal(ValueError):
    """An invalid or underpowered TOST run. Carries the partial `ParityResult` (if one was
    computable, e.g. the UNDERPOWERED verdict) so a caller can still emit it before propagating."""
    exit_code: int = 7
    def __init__(self, reason: str, *, result: "ParityResult | None" = None) -> None: ...

@dataclass(frozen=True)
class Observation:
    prompter: str
    task: str
    run: int
    score: float

@dataclass(frozen=True)
class ParityResult:
    verdict: str            # "EQUIVALENT" | "NOT-EQUIVALENT" | "UNDERPOWERED"
    prompters: tuple[str, str]
    difference: float
    ci_90: tuple[float | None, float | None]
    margin: float
    margin_justification: str
    n: int
    checked: int
    total: int
    n_by_prompter: dict[str, int]
    required_n: int
    tost_p_value: float | None
    lower_test_p_value: float | None
    upper_test_p_value: float | None
    alpha: float = ALPHA
    def as_dict(self) -> dict[str, object]: ...

def read_observations(lines: Iterable[str]) -> list[Observation]:
    """Parse+validate JSONL observations. Raises `ParityRefusal` on the first malformed/blank/
    duplicate/non-finite-score line -- never silently drops a bad row."""
    ...

def analyze_parity(
    observations: Sequence[Observation], *, margin: float, margin_justification: str,
) -> ParityResult:
    """Welch's two-sample TOST at alpha=0.05 plus its 90% CI. Raises `ParityRefusal` (carrying an
    UNDERPOWERED `ParityResult` when one is computable) if N < 128 total or < 64 per prompter, if
    either group has fewer than 2 observations, or if TOST is undefined (zero variance in both
    groups). Deterministic for identical input -- no RNG anywhere in this fn."""
    ...

def main(argv: Sequence[str] | None = None) -> int: ...
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `UsageRecord` | Every field is `None` or a non-negative `int` (`bool` explicitly excluded via `isinstance(value, bool)` check, since `bool` is a subclass of `int` in Python). | A usage count silently being `True`/`False` (1/0) from a caller passing a boolean by mistake, or a negative token count implying a refund that never happened. |
| `InvocationResult` | `exit_code`/`ok` are read-only derived views of `returncode` — never independently settable, so they cannot drift from it. | A result claiming `ok=True` while `returncode != 0`, which would let a caller believe a failed CLI run succeeded. |
| `Adapter` (Protocol) | `resolved_model()` is documented as "read from the CLI's own transcript" — `probe_adapter` enforces this is only trusted from a `supports_resolved_model_readback = True` declaration, never from calling the method on a fresh, unserved adapter (§6). | An adapter that never actually reads back a model identity self-reporting one anyway, letting `usable_as_verifier` pass on a forged signal. |
| `AtomicLeaf` | Exactly one `MachinePredicate` — `__post_init__` normalizes bare `str` predicates into `MachinePredicate` then raises `SOWRefusal` if the resulting count isn't exactly 1. | A leaf with zero acceptance predicates (unverifiable) or with multiple (ambiguous which one gates "done"). |
| `Sow` | Every `Challenge`/`Clarification`'s citation must resolve to a real `path:line` under `repo_root` (`validate_sow`/`_validate_citation`); every `Clarification.question` must be non-generic (`_is_generic_question`). | A SOW whose challenges cite evidence that doesn't exist, or whose clarifications are rhetorical filler ("what are the requirements?") instead of a specific, answerable question. |
| `Receipt`/`Attestation`/`Submission` (pydantic) | `extra="forbid"` on every model — the schema's `additionalProperties: false` is enforced at parse time, not left to a downstream consumer to notice an extra key. `hash`/`prev_hash` patterns and `schema_version`/`predicateType`/`_type` `Literal`s make every wire-format deviation a parse-time `ValidationError`, not a silent pass-through. | A payload carrying an extra field the JSON schema forbids, a hash missing its `blake3:` prefix, or a `schema_version` fleet hasn't shipped — all rejected before this crate's caller ever sees them as valid. |
| `ParityResult` | `verdict` is one of exactly three strings, and `EQUIVALENT` is only reachable when `powered` is true AND both one-sided tests AND the 90% CI margin check all pass — no "equivalent but underpowered" state is representable (`analyze_parity` computes `verdict` from `powered`/`equivalent` jointly, never independently). | A result claiming equivalence from an underpowered sample, which would let a false-negative-prone comparison masquerade as a confirmed parity finding. |

**Money/precision:** no money type in this package. Token counts (`UsageRecord`) are `int`, never
float. `ParityResult`'s scores/margins/p-values are legitimately `float` (a statistical measurement,
not a precision-sensitive count) — this is the one place in the fleet roster where `float` is
correct, not a violation, and is called out explicitly so a reviewer doesn't flag it reflexively.

**Clock/RNG/IO injection points:** `run_model`/`invoke` are the package's only process-spawning
boundary (`subprocess.run`, always with `stdin=subprocess.DEVNULL`) — not injected as a trait
(Python has no compile-time trait-injection equivalent used elsewhere in this package), but
**monkeypatchable at the exact seam** `crew.adapters._subprocess.subprocess.run` (today's tests
already do this — see `test_adapters.py`), which is this package's accepted substitute for
dependency injection. `read_metadata`/`_resolve_file`/`_validate_citation` touch the filesystem
directly (`Path.read_text`, existence checks) — real IO, not injected, because the whole point of
those functions is reading the artifact/repo tree the caller names; no clock or RNG is read
anywhere in this package (`ts_wall`/`Receipt` timestamps are caller-supplied strings, matching
`fleet-types`' identical choice not to parse or generate wall-clock time itself).

## 5. Reuse map

Source read in full: `fleet/crew/crew/adapters/{base,claude,codex,capability,_subprocess,__init__}.py`,
`fleet/crew/crew/sow.py` (579 lines), `fleet/crew/crew/schema.py` (134 lines), `fleet/crew/crew/
parity.py` (312 lines), `fleet/crew/pyproject.toml`, `fleet/crew/uv.lock`, and
`fleet/crew/tests/{test_adapters,test_sow,test_sow_cli,test_parity}.py`.

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `adapters/base.py:1-49` (`AdapterError`, `UsageRecord`, `InvocationResult`) | Error type + evidence dataclasses. | yes, verbatim | Split into `errors.py` (10 lines) + `records.py` (~35 lines) — file alone is 90 lines, over the cap. |
| `adapters/base.py:52-91` (`Adapter` Protocol) | The 6-capability contract. | yes, verbatim | Own file `protocol.py`. |
| `adapters/claude.py:1-67`, `adapters/codex.py:1-67` | Two near-identical adapters (command-building differs, `invoke()` body is byte-for-byte the same 15 lines). | yes, logic unchanged | Extract the shared `invoke()` body into `_cli_adapter.invoke_cli(adapter, prompt, ...)` (§8's `_cli_adapter.py`) so `claude.py`/`codex.py` each shrink to ~25 lines (constructor + `command()` + a 2-line `invoke()` that calls the shared helper) — removes the duplication without changing either adapter's observable behavior; every existing `test_adapters.py` assertion still applies unchanged. |
| `adapters/capability.py:1-53` (`CapabilityReport`) | Report dataclass + 4 derived properties. | yes, verbatim | Own file (~50 lines, under cap already). |
| `adapters/capability.py:55-94` (`probe_adapter`) | The two nested closures + dataclass construction. | yes, verbatim | Own file `capability_probe.py` (~45 lines) if kept separate from the dataclass, or both together if the combined file stays ≤80 (94 total today is 14 over — splitting report/probe into two files is the simplest fix; see §8). |
| `adapters/_subprocess.py:16-53` (`read_metadata`) | JSON-metadata scraping from a transcript. | yes, verbatim | Own file `metadata.py` (~40 lines). |
| `adapters/_subprocess.py:56-134` (`run_model`) | Subprocess exec + log-floor/diff invariants. | yes, verbatim | Own file `runner.py` (~80 lines — right at the cap; split further into `runner.py` (exec) + `_invariants.py` (floor/diff checks) if it lands over). |
| `sow.py:1-37` (`SOWRefusal`) | Typed refusal + receipt-shaped payload. | yes, verbatim | Own file `sow/errors.py`. |
| `sow.py:40-80` (`MachinePredicate`, `AtomicLeaf`) | One-predicate-per-leaf invariant. | yes, verbatim | Into `sow/model.py` along with the other dataclasses below. |
| `sow.py:84-145` (`Challenge`, `Clarification`, `Alternative`, `Sow`, `_default_repo_root`) | Remaining SOW dataclasses + repo-root default. | yes, verbatim | `sow/model.py` (dataclasses) + `sow/_paths.py` (`_default_repo_root`) if the combined file would exceed 80. |
| `sow.py:146-237` (`_clean_task`, `_split_sections`, `_strip_bullet`, `_build_leaves`) | Task-string tokenizing + leaf construction. | yes, verbatim | `sow/parse_sections.py` + `sow/parse_leaves.py`. |
| `sow.py:238-334` (`_find_citations`, `_parse_challenges`, `_parse_clarifications`, `_parse_alternatives`, `_parse_estimate`, `_parse_edge_cases`, `_is_generic_question`) | Per-section parsing + the generic-question heuristic. | yes, verbatim | `sow/parse_meta.py` (may itself need a second file if all six don't fit in 80 lines — split parse_meta.py / parse_meta_ext.py by section group). |
| `sow.py:335-366` (`_resolve_file`, `_validate_citation`) | Citation → real `path:line` resolution. | yes, verbatim | `sow/citations.py`. |
| `sow.py:367-401` (`validate_sow`) | Re-validates a constructed `Sow`. | yes, verbatim | `sow/validate.py`. |
| `sow.py:402-463` (`build_sow`) | Parses task text into a validated `Sow`. | yes, verbatim | `sow/build.py`. |
| `sow.py:464-512` (`_sow_payload`) | JSON-serializable view of a `Sow`. | yes, verbatim | `sow/payload.py`. |
| `sow.py:513-579` (`main`, argparse, `if __name__`) | CLI entry point. | yes, verbatim | `sow/cli.py`. |
| `schema.py:1-58` (`ContractModel`, `Digest`, `Subject`, `AttestationElements`) | Base config + attestation leaf models. | yes, verbatim | `schema/contract_model.py` + `schema/attestation.py` (attestation.py alone would be ~75 lines with the rest of the attestation tree — fits). |
| `schema.py:61-78` (`AttestationBuilder`, `AttestationPredicate`, `Attestation`) | Attestation root + predicate. | yes, verbatim | Same `schema/attestation.py`. |
| `schema.py:81-109` (`Receipt`) | Ledger row model + `ts_wall` validator. | yes, verbatim | `schema/receipt.py`. |
| `schema.py:112-134` (`Submission`, aliases, `__all__`) | Submission model + friendly names. | yes, verbatim | `schema/submission.py` (aliases/`__all__` move to `schema/__init__.py`). |
| `parity.py:1-38` (module docstring, constants, `ParityRefusal`) | Design constants + typed refusal. | yes, verbatim | `parity/types.py` (constants + `ParityRefusal` + `Observation` + `ParityResult` may need 2 files — see §8). |
| `parity.py:41-94` (`Observation`, `ParityResult`) | Value dataclasses + `as_dict()`. | yes, verbatim | Same as above, split if combined file exceeds 80. |
| `parity.py:97-140` (`_finite_number`, `read_observations`) | JSONL parsing + validation. | yes, verbatim | `parity/read.py`. |
| `parity.py:143-247` (`analyze_parity`) | The TOST computation itself — this file's entire reason to exist. | yes, verbatim | `parity/analyze.py` (105 lines as one fn — over cap; this is the one function in the whole package that cannot trivially split without breaking a single statistical derivation mid-computation. See the divergence note: this is the file flagged as unable to get under 80 lines without an awkward split.) |
| `parity.py:250-312` (`_open_input`, `main`, `__all__`, entry point) | CLI plumbing. | yes, verbatim | `parity/cli.py`. |
| `pyproject.toml` (whole file) | Package metadata + 3 runtime deps + 1 dev dep. | yes, verbatim | Path/name updates only (`crates/fleet-crew/pyproject.toml`); dependency versions unchanged (§7). |
| `tests/test_adapters.py`, `test_sow.py`, `test_sow_cli.py`, `test_parity.py` (whole files) | Existing green pytest suite exercising every fn above. | yes, as inspiration + kept | Import paths update to the new module layout (`crew.adapters.claude` unchanged since `adapters/` keeps its name; `crew.sow.build` etc. for the split-out SOW/schema/parity modules) — assertions themselves are unchanged, this is a rename, not a rewrite. |

## 6. Behavior spec

### `fn invoke(self, prompt, *, stdout_path, stderr_path, cwd=None, env=None, diff_path=None) -> InvocationResult` (both adapters, via the shared `_cli_adapter.invoke_cli` helper)

| Input dimension | Behavior |
|---|---|
| empty | `prompt=""` → still builds a valid command (`["claude", "-p", ""]`) and runs it; the CLI itself decides whether an empty prompt is meaningful — this layer never rejects on prompt content. |
| null / `None` | `cwd=None`/`env=None` → `subprocess.run` receives Python's own defaults (inherit the current working directory / current environment) — not an error, the documented default. `diff_path=None` → the diff/empty-diff invariant is skipped entirely (only enforced when a diff path is supplied), matching `run_model`'s `if diff_path is None or not diff_path.exists()...` short-circuit already in the reused body. |
| wrong-type | Not reachable through the typed signature; a caller passing a non-`Path` for `stdout_path` fails at the first `.open()`/`.mkdir()` call with a Python `TypeError`/`AttributeError` — not specially caught, since this is a programmer error at the call site, not a runtime input to validate. |
| huge | A multi-megabyte `prompt` string → passed as a single argv element to `subprocess.run`; no length cap in this layer (the OS/CLI's own argv-length limit is the real ceiling, and hitting it surfaces as the subprocess's own `FileNotFoundError`/`OSError`, propagated, never silently truncated). |
| negative | n/a — no numeric input to `invoke` itself; `log_size_floor` is set at adapter construction and validated only by comparison (`size < log_size_floor`), so a caller-supplied negative floor would make every invocation trivially pass the floor check — a real but low-severity latent gap, flagged here rather than silently left implicit (see §9's `rejects_negative_log_size_floor` test, a new addition this blueprint proposes). |
| duplicate | Calling `invoke` twice with the same `stdout_path`/`stderr_path` → the second call's `.open("wb")` truncates and overwrites the first's evidence — by design (each invocation is a fresh attempt); a caller needing to preserve both must pass distinct paths, which is the caller's responsibility, not this fn's. |
| concurrent | Two `invoke()` calls from different threads/processes writing to the SAME `stdout_path` race at the OS file-handle level (last writer wins, no locking) — callers must give each concurrent invocation its own path (exactly what `fleet-worker`'s per-lane worktree isolation already guarantees upstream; this fn does not itself serialize). |
| unicode / non-ASCII | `prompt` containing full Unicode (including emoji, RTL text) → passed through `subprocess.run`'s argv encoding untouched; stdout/stderr are read back as UTF-8 by `read_metadata` with `encoding="utf-8"` (no `errors="replace"` today — a non-UTF-8 transcript byte raises `UnicodeDecodeError` inside `read_metadata`'s `try/except OSError`, which does NOT catch it — see §9's proposed `metadata_survives_non_utf8_transcript` test, a real gap this blueprint surfaces rather than silently accepting). |
| already-exists | `resolved_model()` called on a freshly constructed, never-invoked adapter → returns `None` (the constructor sets `self._resolved_model = None` and nothing populates it before an `invoke()` call) — this is the exact case `probe_adapter`'s `has_resolved_model_readback` closure must NOT treat as "incapable" when `supports_resolved_model_readback = True` is declared; the class-level flag, not a live call, is the source of truth for probing (already correct in today's code; §9 pins it with a named test). |
| partial-failure | `subprocess.run` raising `FileNotFoundError` (CLI binary missing) → caught explicitly, returns a well-formed `InvocationResult(returncode=3, ...)` (fleet's `EXIT_ENV` convention) instead of propagating the exception — the ONE place this package translates an OS exception into a typed result rather than a typed exception, because "the CLI isn't installed" is `fleet-worker`'s capability-probing concern to detect via this exact returncode, not a crash. |

### `fn probe_adapter(adapter: Adapter) -> CapabilityReport`

| Input dimension | Behavior |
|---|---|
| empty | An adapter object with no attributes at all (a bare `object()`) → every `getattr(..., None)` returns `None`, every `callable(None)` is `False` — `CapabilityReport` with every field `False`, `missing` lists all 6, `usable_as_builder`/`usable_as_verifier` both `False`. Never raises. |
| null / `None` | `adapter.name` absent → `getattr(adapter, "name", type(adapter).__name__)` falls back to the Python class name — the report is still constructible, never `None`-valued in the `adapter` field. |
| wrong-type | `adapter.resolved_model` present but not callable (e.g. a plain string attribute) → `callable(method)` is `False`, `has_resolved_model_readback` returns `False` — no `TypeError` from attempting to call a non-callable. |
| huge | n/a — probing is O(1) regardless of adapter internals; no collection is traversed. |
| negative | n/a — no numeric input. |
| duplicate | Probing the same adapter instance twice → identical `CapabilityReport` both times (the probe reads only static attributes/flags, never invocation history) — idempotent by construction. |
| concurrent | Read-only attribute access on a shared adapter instance from multiple threads — safe as long as the adapter itself doesn't mutate `_resolved_model` concurrently with a probe (adapters are not documented `Send`-equivalent in this package; today's single-threaded CLI usage makes this a documented, not yet enforced, assumption — flagged for `fleet-worker`'s scheduler to serialize per adapter instance). |
| unicode / non-ASCII | `adapter.name` containing non-ASCII text → passed through into `CapabilityReport.adapter` unchanged, no normalization; this is a display field only, never compared for equality against a fixed set inside this fn. |
| already-exists | Re-probing an adapter after it HAS served a request (so `resolved_model()` would now return a real string even without the class flag) → the flag-based branch still takes precedence when `supports_resolved_model_readback = True`; when that flag is absent, the fn falls back to actually calling `resolved_model()` and checking for a non-blank string — so an adapter that never declares the flag but happens to have served a request would probe as capable that one time and incapable before it — a known asymmetry this blueprint documents rather than silently inherits: the flag-first design exists specifically so a caller need not have already spent a real invocation just to probe eligibility. |
| partial-failure | `adapter.operator_credentials()` raising `RuntimeError`/`OSError`/`TypeError`/`ValueError` → caught by `has_operator_credentials`'s explicit `except`, treated as `False` (not capable) — never propagates out of `probe_adapter` and never crashes the caller's eligibility check. |

### `fn build_sow(task: str, *, repo_root: Path | None = None) -> Sow`

| Input dimension | Behavior |
|---|---|
| empty | `task=""` → `_clean_task` yields no lines, `_split_sections` finds no preamble/sections, `_build_leaves` produces zero leaves → `SOWRefusal` (an empty task cannot restate intent or name a single atomic leaf; the existing behavior already refuses here — confirmed by `test_sow.py`'s structure, though no test today names this exact case — §9 proposes `empty_task_is_refused` to pin it explicitly). |
| null / `None` | `repo_root=None` → `_default_repo_root(None)` resolves a fallback (the crate's own root) rather than raising — only an explicit, wrong `repo_root` (one under which no cited file exists) triggers `SOWRefusal` via `_validate_citation`. |
| wrong-type | n/a — `task: str` is enforced by the type signature; a caller passing bytes/None fails at the first `.strip()`/regex call with a Python `TypeError`, not a `SOWRefusal` (a programmer-error boundary, not a data-validation one). |
| huge | A task string with thousands of "Challenges:" bullet lines → `_build_leaves`/`_parse_challenges` are linear scans over the line list; no quadratic blowup, no cap on leaf count — a caller wanting a sane upper bound on SOW size enforces it themselves before calling `build_sow`. |
| negative | n/a — no numeric input. |
| duplicate | Two challenges citing the identical `path:line` → both are kept (citations are not deduplicated); this is intentional — the SAME piece of evidence may legitimately support two distinct challenges. |
| concurrent | Pure function over its string input plus read-only filesystem checks (citation resolution) — safe to call concurrently from multiple threads/processes as long as `repo_root`'s files aren't being concurrently deleted mid-call (a real but accepted TOCTOU window, matching the filesystem's own guarantees, not specially handled). |
| unicode / non-ASCII | A task string containing non-ASCII bullet text / citations → parsed identically to ASCII (Python `str` regex operations are Unicode-aware by default); a citation path itself containing non-ASCII characters resolves via `Path`, which is UTF-8-native on Linux/macOS. |
| already-exists | Calling `build_sow` twice with identical `task`/`repo_root` → produces two structurally-equal `Sow` values (no hidden state, no counter) — idempotent. |
| partial-failure | `_validate_citation` finding the FIRST unresolvable citation raises immediately — no partially-validated `Sow` is ever returned; a caller never sees a `Sow` with some citations checked and others silently skipped. |

### `fn analyze_parity(observations, *, margin, margin_justification) -> ParityResult`

| Input dimension | Behavior |
|---|---|
| empty | `observations=[]` → `SORefusal`-equivalent: `ParityRefusal("input contains zero observations")` raised immediately, before any statistics are attempted. |
| null / `None` | `margin<=0` or non-finite → `ParityRefusal("equivalence margin must be a finite positive number")`; `margin_justification=""`/whitespace-only → `ParityRefusal(...)` — both preregistration requirements are checked BEFORE any observation is read, so a bad margin can never be "explained away" by looking at the data first (the module docstring's whole design point). |
| wrong-type | n/a at this fn's boundary — `Observation.score` is already validated `float` by `read_observations`/`_finite_number` upstream; `analyze_parity` itself trusts its typed input. |
| huge | Tens of thousands of observations per prompter → `numpy` arrays scale linearly; no quadratic step in `CompareMeans`/`ttost_ind`. `n_by_prompter`'s dict has exactly 2 keys regardless of N. |
| negative | A negative `score` is representable (scores are arbitrary real-valued rubric numbers, not counts) — no special handling, this is not the "negative" case that needs refusal; a negative `margin` IS refused (see "null/None" row above, which is the actual guard). |
| duplicate | Two prompters with the literal same name → `len(prompters) != 2` fails to trigger only if `dict.fromkeys` collapses them to ONE name, which then fails the "expected exactly two prompters; found 1" `ParityRefusal` — duplicate identity is caught, not silently merged. |
| concurrent | Pure function over its (already fully materialized) input sequence plus `numpy`/`statsmodels` calls with no shared mutable state — safe to call concurrently for independent observation sets. |
| unicode / non-ASCII | Prompter/task names containing non-ASCII text → carried through into `n_by_prompter`'s dict keys and `ParityResult.prompters` unchanged; no normalization, no equality-folding (two visually-identical but differently-encoded Unicode strings are treated as distinct prompters — a known, documented limit, not a silent bug). |
| already-exists | Calling `analyze_parity` twice with identical input → identical `ParityResult` (no RNG, no clock) — deterministic, matching this package's "no randomness anywhere in the statistics path" property. |
| partial-failure | Every early-exit path (`ParityRefusal`) fires BEFORE `ttost_ind`/`CompareMeans` are called, or (for the `UNDERPOWERED` verdict) AFTER computing a full, well-formed `ParityResult` that is then attached to the raised `ParityRefusal` — never a half-computed result silently returned as if it were a pass; the caller always gets either a complete `ParityResult` (return) or a `ParityRefusal` (possibly carrying a complete, but refused, result). |

## 7. Dependencies

| Package | Version | Why |
|---|---|---|
| `pydantic` | `2.13.5` (matches `crates/fleet-crew/uv.lock`'s resolved version; `pyproject.toml` pins `>=2.0,<3.0`) | `schema/*.py`'s strict, `extra="forbid"` mirrors of the three JSON contracts — this package's only validation framework. |
| `numpy` | `2.4.6`/`2.5.3` (two resolutions in `uv.lock`, split by Python-version marker; `pyproject.toml` pins `>=2.0,<3.0`) | `parity/analyze.py`'s array math (`np.asarray`, `np.mean`, `np.var`). |
| `statsmodels` | `0.15.0` (matches `uv.lock`; `pyproject.toml` pins `>=0.14,<1.0`) | `CompareMeans`/`DescrStatsW`/`ttost_ind` — the Welch's-TOST implementation itself; reimplementing this by hand would be exactly the kind of "adopting a tool is not the tool working" risk this repo's own `PRINCIPLES.md` warns against — use the maintained library. |
| `pytest` (dev) | `8.4.2` (matches `uv.lock`; `pyproject.toml` pins `>=8.0,<9.0`) | The existing test suite's runner. |
| `ruff` (dev) | `0.16.6` (matches `uv.lock`; `pyproject.toml` pins `>=0.6,<1.0`) | Lint gate run by §10's verification recipe. Named as a divergence-note "new addition" in the pre-build version of this blueprint — confirmed added to the built `pyproject.toml`'s `[dependency-groups] dev` list, along with a `[tool.ruff] line-length = 100` config block. |

Every package above is resolved in `crates/fleet-crew/uv.lock` today; the three runtime deps are
unchanged from the pre-build proposal, `ruff` was added as a dev dependency exactly as the
divergence note below anticipated.

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines**, `.py` included. `base.py` (90), `capability.py` (94),
> `sow.py` (579), and `parity.py` (312) all currently exceed this and MUST split. `lib.rs`'s Rust
> equivalent here is each subpackage's thin `__init__.py` (re-exports only, no logic).

> **As built** (`find crates/fleet-crew/{crew,tests} -name '*.py' \| sort`, line counts via `wc -l`),
> the layout matches the pre-build sketch below closely, with a few extra files the split produced:
> `adapters/_diff.py` (empty-diff check factored out of `runner.py`), `sow/leaf.py` (`MachinePredicate`/
> `AtomicLeaf` factored out of `model.py`), and `parity/_grouping.py` + `parity/_tost.py` (the
> `analyze_parity` derivation §5/the divergence note flagged as "may not fit in 80 lines" was in fact
> split further, resolving that flag rather than needing an exception). `tests/` also grew a
> `conftest.py` plus several new test files (`test_adapter_metadata.py`, `test_parity_refusals.py`,
> `test_runner_floor_boundary.py`, `test_sow_cli_valid.py`, `test_sow_validate_clarify.py`) beyond
> what §9 named.

```
crates/fleet-crew/
  pyproject.toml
  crew/
    __init__.py                # 3  — package marker only
    adapters/
      __init__.py              # 25 — re-exports (Adapter, AdapterError, ClaudeAdapter, CodexAdapter, ...)
      protocol.py               # 49 — Adapter Protocol
      errors.py                  # 9  — AdapterError
      records.py                  # 50 — UsageRecord, InvocationResult
      metadata.py                  # 57 — read_metadata
      runner.py                     # 73 — run_model (subprocess exec + log-floor invariant)
      _diff.py                       # 27 — the empty-diff invariant, factored out of runner.py
      _cli_adapter.py                 # 47 — invoke_cli(): the shared body claude.py/codex.py both call
      claude.py                        # 56 — ClaudeAdapter (command() + thin invoke())
      codex.py                          # 56 — CodexAdapter (command() + thin invoke())
      capability_report.py               # 50 — CapabilityReport dataclass + 4 properties
      capability_probe.py                 # 50 — probe_adapter()
    sow/
      __init__.py                         # 19 — re-exports (build_sow, validate_sow, Sow, ...)
      errors.py                            # 23 — SOWRefusal
      leaf.py                               # 48 — MachinePredicate, AtomicLeaf
      model.py                               # 48 — Challenge, Clarification, Alternative, Sow
      _paths.py                               # 12 — _default_repo_root
      parse_sections.py                        # 65 — _clean_task, _split_sections, _strip_bullet
      parse_leaves.py                           # 46 — _build_leaves
      parse_meta.py                               # 57 — _find_citations, _parse_challenges, _parse_clarifications
      parse_meta_ext.py                            # 71 — _parse_alternatives, _parse_estimate, _parse_edge_cases, _is_generic_question
      citations.py                                  # 47 — _resolve_file, _validate_citation
      validate.py                                    # 46 — validate_sow
      build.py                                        # 80 — build_sow
      payload.py                                       # 50 — _sow_payload
      cli.py                                            # 68 — main() + argparse
    schema/
      __init__.py                                       # 38 — re-exports + DeliveryAttestation/InTotoStatement aliases
      contract_model.py                                  # 11 — ContractModel
      attestation.py                                       # 58 — Digest, Subject, AttestationElements, AttestationBuilder, AttestationPredicate, Attestation
      receipt.py                                            # 36 — Receipt (+ ts_wall validator)
      submission.py                                          # 13 — Submission
    parity/
      __init__.py                                            # 17 — re-exports
      types.py                                                # 40 — ALPHA/MIN_TOTAL_N/MIN_PER_PROMPTER, ParityRefusal, Observation
      result.py                                                # 55 — ParityResult (+ as_dict)
      read.py                                                  # 53 — _finite_number, read_observations
      _grouping.py                                              # 28 — per-prompter grouping/count checks, factored out of analyze.py
      _tost.py                                                   # 25 — the TOST call itself, factored out of analyze.py
      analyze.py                                                  # 74 — analyze_parity: orchestrates _grouping/_tost, computes the verdict
      cli.py                                                       # 61 — _open_input, main, __all__, entry point
  tests/
    conftest.py                    # 58 — shared pytest fixtures
    test_adapter_claude.py         # 75 — ClaudeAdapter invoke/resolved-model/log-floor/diff cases
    test_adapter_codex.py          # 35 — CodexAdapter command-shape parity with claude
    test_adapter_metadata.py       # 57 — read_metadata cases, including the non-UTF-8-transcript gap named in §9
    test_capability_probe.py       # 67 — probe_adapter's builder/verifier eligibility cases
    test_runner_floor_boundary.py  # 28 — the negative-log-size-floor gap named in §9
    test_sow_build.py              # 79 — build_sow happy path + structural refusals
    test_sow_validate.py           # 39 — validate_sow citation refusals
    test_sow_validate_clarify.py   # 52 — validate_sow's generic-clarification refusals
    test_sow_cli.py                # 62 — python -m crew.sow.cli end-to-end (refusal cases)
    test_sow_cli_valid.py          # 31 — python -m crew.sow.cli end-to-end (valid-task cases)
    test_schema_roundtrip.py       # 78 — Receipt/Attestation/Submission parse + extra="forbid" rejection
    test_parity_analyze.py         # 71 — analyze_parity equivalence/underpowered cases
    test_parity_read.py            # 56 — read_observations malformed-line cases
    test_parity_refusals.py        # 29 — analyze_parity's ParityRefusal cases (bad margin, zero observations, ...)
```
> Any file still over 80 lines once real bodies land splits further — the §10 `wc -l` gate is the
> actual check. `sow/build.py` landed exactly at the 80-line ceiling.

`pyproject.toml` (as built):
```toml
[project]
name = "crew"
version = "0.1.0"
description = "Fleet intent-layer SOW construction, keyless CLI adapters, and contract models"
requires-python = ">=3.11"
dependencies = [
  "numpy>=2.0,<3.0",
  "pydantic>=2.0,<3.0",
  "statsmodels>=0.14,<1.0",
]

[dependency-groups]
dev = [
  "pytest>=8.0,<9.0",
  "ruff>=0.6,<1.0",
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.ruff]
line-length = 100
```

## 9. Test plan

**Unit tests** (ported verbatim in spirit from the existing suite, split by module per §8):
- `test_successful_invocation_closes_stdin_and_returns_file_evidence` — `stdin=subprocess.DEVNULL`
  every call; `resolved_model` stays `None` when the fake CLI emits no metadata (never echoes the
  requested model).
- `test_resolved_model_is_read_back_from_output_not_from_the_request` — a fake CLI emitting
  `{"resolved_model": "claude-actually-served"}` on its own JSON line yields exactly that string,
  never the constructor's `model=` argument.
- `test_log_floor_refuses_a_false_success` / `test_empty_diff_refuses_a_false_success` — both
  `AdapterError` invariants from `runner.py`.
- `test_missing_resolved_model_is_builder_only` — a hand-rolled `Adapter` with no readback support
  probes `usable_as_builder=True`, `usable_as_verifier=False`, `"resolved_model_readback"` in
  `missing`.
- `build_sow_has_one_machine_predicate_per_leaf` — every leaf of a valid task has exactly one.
- `generic_clarification_is_refused` — a "what are the requirements?"-style clarification raises
  `SOWRefusal`.
- `receipt_round_trips_and_rejects_extra_field` (**new**) — a valid `Receipt` payload parses;
  adding one undeclared key raises pydantic's `ValidationError` (`extra="forbid"` is enforced, not
  merely declared).
- `positive_control_same_distribution_is_equivalent` / underpowered-sample verdict tests (existing
  `test_parity.py` synthetic-observation fixtures, unchanged).

**Integration tests:**
- `test_sow_cli.py` (existing, ported) — `python -m crew.sow.cli --task ...` end-to-end, asserting
  exit codes and stderr refusal receipts for each parametrized structural defect.
- `adapter_and_probe_agree_on_verifier_eligibility` (**new**) — construct both `ClaudeAdapter` and
  `CodexAdapter`, run each through `probe_adapter`, assert both report `verifier_eligible=True`
  (both declare `supports_resolved_model_readback = True`) — a cross-module contract test that
  would catch a future adapter forgetting to set the flag.

**New tests this blueprint adds to close gaps found while writing §6** (not present in today's
suite — named here so they aren't lost between blueprint and build):
- `rejects_negative_log_size_floor` — constructing an adapter with `log_size_floor=-1` and running
  a near-empty invocation must still raise `AdapterError`, not silently pass (§6's flagged gap;
  requires a small guard added to `runner.py`'s floor check, e.g. `max(log_size_floor, 0)` or an
  explicit refusal on a negative floor — Opus to pick one, noted in the divergence section).
- `metadata_survives_non_utf8_transcript` — a transcript file containing invalid UTF-8 bytes must
  make `read_metadata` return `(None, None)`, not raise `UnicodeDecodeError` out of `invoke()` (§6's
  flagged gap; requires widening `read_metadata`'s `except OSError` to also catch
  `UnicodeDecodeError`).
- `empty_task_is_refused` — `build_sow("")` raises `SOWRefusal` (behavior almost certainly already
  correct today; this pins it as a named, explicit case rather than an implicit one).

**Mutation-testing targets** (`mutmut`/manual — this package has no `cargo mutants` equivalent
configured today; see §10 for the concrete substitute):
- Flipping `size < log_size_floor` to `size <= log_size_floor` in `runner.py` must be killed by
  `test_log_floor_refuses_a_false_success`'s exact-floor-minus-one fixture.
- Flipping `len(normalized) != 1` to `< 1` in `AtomicLeaf.__post_init__` (so two predicates would
  silently pass) must be killed by a dedicated `two_predicates_on_one_leaf_is_refused` test — not
  present in today's suite; **add it**.
- Flipping `powered = n >= MIN_TOTAL_N and all(...)` to use `or` instead of `and` in
  `analyze_parity` must be killed by a fixture with total N above 128 but one prompter below 64 —
  **add `underpowered_by_imbalance_despite_high_total_n`**, a gap in today's suite (existing tests
  vary total N, not the per-prompter split independently).

**Property tests:** not applicable in the strict `proptest`/`quickcheck` sense (no Rust runtime
here); `hypothesis` is a reasonable substitute for `read_observations`/`Blake3`-pattern-style fields
in `schema.py` (e.g. "any string not matching `^[0-9a-f]{64}$` is rejected by `Digest`") but is a
**new** dev-dependency this blueprint does NOT add without Opus sign-off — flagged, not silently
introduced, per §7's "no new dependency" claim.

## 10. Verification recipe

```bash
cd crates/fleet-crew
uv sync --group dev                                  # or: pip install -e '.[dev]'
uv run pytest -q                                      # unit + integration, publish N/N, 0 skipped
uv run ruff check crew tests                          # lint clean, 0 warnings (ruff is NOT in
                                                       # pyproject.toml today — add as a dev dep;
                                                       # this is a NEW addition the brief requires,
                                                       # flagged here rather than silently assumed)
find crew tests -name '*.py' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'   # must print nothing
```
Expected: all tests pass — publish as `<passed>/<total>` (e.g. `31/31`, not "tests pass"), 0
skipped. `ruff check`: 0 errors. File-size gate: no output. This package has no `cargo mutants`
equivalent wired up in fleet today; until one is chosen (`mutmut` is the closest Python analogue),
the mutation-testing targets in §9 are verified by **manually introducing each named mutant and
confirming the named test fails**, recording the result as `<caught>/<total named mutants>` in the
PR — never skipped as "no mutation tool configured."

## 11. L8 checklist

- [ ] Every fallible path raises a typed exception subclass carrying `exit_code`
      (`AdapterError`/`SOWRefusal`/`ParityRefusal`) — none swallowed into a bare `assert`, bare
      `Exception`, or a silently-`None`-returning failure path, EXCEPT the two documented
      programmer-error boundaries in §6 (`UsageRecord.__post_init__`'s `ValueError`, `invoke`'s
      wrong-type `TypeError`) which are call-site bugs, not data-validation outcomes.
- [ ] Clock/RNG: none read anywhere in this package (confirmed by grep: no `time.time()`,
      `datetime.now()`, `random`, or `uuid4` call in `crew/` outside test fixtures) — `ts_wall` is
      always caller-supplied.
- [ ] IO/subprocess boundary documented and monkeypatchable: `crew.adapters._subprocess.subprocess.run`
      is the sole spawn point; `read_metadata`/citation-resolution are the sole direct-filesystem
      reads, both already exercised via `tmp_path` in tests, never the repo tree or `$HOME`.
- [ ] No float used for any count (`UsageRecord`, `Receipt.seq`) — `float` appears only in
      `ParityResult`'s legitimate statistical fields (§4 note), never as a token/money surrogate.
- [ ] No self-grading: `uv run pytest` alone is not the gate — §10's ruff + file-size + named-mutant
      manual verification all run before this crate is proposed DONE; the denominator is published,
      not just "green".
- [ ] The verify command's pass/fail denominator (`x/y`) is stated in this file (§10, template) and
      will be restated in the PR — not just "green".
- [ ] Tests that touch the filesystem use `tmp_path` (pytest's built-in tempdir fixture) exclusively
      — confirmed already true in `test_adapters.py`; carry the same discipline into every new/split
      test file.
- [ ] Every non-goal in §2 is actually absent from the code: no `append_receipt`/ledger-write call,
      no worktree creation, no quota/cooldown read, and — per the explicit non-goal — **no
      `GeminiAdapter`** anywhere in `crew/adapters/`.
- [ ] **No source file exceeds 80 lines** (verified: `find crew tests -name '*.py' | xargs wc -l` —
      every file ≤ 80, including the previously-over-cap `base.py`/`capability.py`/`sow.py`/
      `parity.py` after the §8 split). `analyze.py` at ~80 is the tightest fit in the roster — flag
      to Opus if the real body lands at 81+ (see divergence note).

## 12. Definition of Done

`fleet-crew` is DONE when: §10's commands all pass with a published denominator (pytest `N/N`, 0
skipped; ruff clean, 0 errors; file-size gate silent) run from `crates/fleet-crew/`; every §9 named
test (existing-ported AND the four new gap-closing tests) exists and passes; every §9 mutation
target has been manually reproduced and killed, recorded as `<caught>/<total>` in the PR; every
unchecked box in §11 is checked with its real numbers; `registry/services/REGISTRY.md` (infra
package, not a product feature — `services/`, per C1/L2) lists the crate; and Opus has independently
re-derived the six-capability `Adapter` contract and the one-predicate-per-leaf `Sow` invariant from
this blueprint alone, reproduced the `size <= log_size_floor` mutation by hand, and driven one real
`ClaudeAdapter.invoke()` call end-to-end confirming `resolved_model` is never the requested alias.

---

## Divergence from MIGRATION-PLAN / build-briefing (for Opus)

1. **No `gemini` adapter exists anywhere in fleet, despite the build brief naming
   "claude/codex/gemini."** A repo-wide grep for `gemini` found zero hits in `fleet/crew/` or
   anywhere else in `fleet/` except unrelated PDF-build scripts under `fleet/tmp/pdfs/`. This
   blueprint does not invent a `GeminiAdapter` — doing so without a real CLI's non-interactive
   flag/output-metadata shape to cite would be exactly the kind of unevidenced claim PLAYBOOK.md
   rule 2 and this repo's `PRINCIPLES.md` warn against. **Needs an Opus decision**: either (a) drop
   "gemini" from the crate's scope entirely (my recommendation — nothing in MIGRATION-PLAN's row 14
   or roster DAG requires it), or (b) scope a follow-up build-new task once the real `gemini` CLI's
   contract is confirmed by hand (install it, run `gemini --help`, observe actual output shape)
   rather than guessed from this blueprint alone.
2. **`sow.py`/`schema.py`/`parity.py` living in the same package as the adapters** was the build
   brief's explicit citation list, and MIGRATION-PLAN row 14's evidence is generically "`fleet/
   crew/`" (the whole directory) — but the row's own prose ("keyless CLI adapters") reads narrower
   than what this blueprint scopes. I kept all four modules together because that is what exists
   today as one working, one-`pytest`-suite package, and PLAYBOOK.md's iterate-don't-rebuild
   decision (§1) argues against splitting a cohesive, already-tested unit without a concrete reason
   to. If Opus wants `sow`/`schema`/`parity` as a separate `fleet-intent`-style crate instead, that
   is a crate-boundary call this blueprint defers, flagged rather than silently assumed.
3. **Resolved as built:** `parity/analyze.py`'s ~105-line body was in fact split further, exactly
   along the axis this note worried would be awkward — `parity/_grouping.py` (per-prompter
   grouping/count checks) and `parity/_tost.py` (the `ttost_ind` call itself) were factored out,
   leaving `analyze.py` at 74 lines orchestrating both plus the joint verdict logic. No named
   exception file was needed; the split this note called "awkward" turned out to be the actual
   solution.
4. **Adding `ruff` as a dev dependency** (§10) — not in today's `pyproject.toml`. The build brief
   names `ruff` as part of §10's verification recipe explicitly, so this blueprint adds it as a new
   dev-dependency rather than silently reusing only `pytest`; flagged here since §7 otherwise claims
   "no new dependency," which is true for the three runtime deps but not for this one dev-only
   addition.
5. **Resolved as built:** both gaps surfaced while writing §6 were fixed. `crew/adapters/runner.py`
   now computes an `_effective_floor()` (`log_size_floor if log_size_floor > 0 else
   DEFAULT_LOG_SIZE_FLOOR`), so a caller-supplied negative/zero floor can no longer trivially pass
   the check, and `crew/adapters/metadata.py`'s `read_metadata` catches `(OSError,
   UnicodeDecodeError)` explicitly (with a comment citing exactly this gap) instead of only
   `OSError`. `tests/test_runner_floor_boundary.py` and `tests/test_adapter_metadata.py` cover the
   two cases.
