# F04 — Server-side build-mode session + depth-completeness belief registers (lane contract)

**Lane:** `lane/F04-server-side-build-mode` · worktree `.worktrees/F04-server-side-build-mode`
**Repo target:** `orb/backend/relay-py` (server-side — reachable by *any* client)
**Author:** lead-architect (F04). **This file is the spec.** The acceptance suite in §5 is written
here, in full, before any implementation exists. It is *supposed to be red* until the builder makes
it green.
**The builder may not edit §5's test files.** Any diff touching
`orb/backend/relay-py/tests/test_f04_*.py` other than creating them verbatim from §5 is a contract
violation — flag it, do not merge it.

---

## 0. Restatement — what is actually being asked, in this codebase's own terms

The owner corrected the architecture at 02:20 IST (`FEATURES.md`, "ARCHITECTURE DECISION"):
`apps/macos` (OrbMac, Swift) is Light's real frontend; `orb/apps/mobile` (React Native) is the old
ADHD-focus-orb product's own UI. **Swift cannot import TypeScript.** F01 built a build-mode toggle
that lives entirely in `orb/apps/mobile/src/{runtime/T0FocusSession.ts,runtime/LaunchMode.ts,App.tsx}`.
It is real, tested and independently verified — and it is unreachable from the app that matters.

F04 moves that capability to where both clients can reach it: the relay. Concretely, two coupled
things:

1. **Build-mode session state, server-side.** A session opened in build mode gets its opening line
   and its system prompt *from the backend*, not from a client constant. The behavioral reference is
   F01's `BUILD_GREETING` / `T0FocusSession.start()` — port the *behavior*, not the code.
2. **Depth-completeness belief registers, server-side.** `orb/apps/mobile/src/cognitive/BeliefModel.ts`'s
   log-odds math, ported to Python and re-aimed from *user state* (attention/energy) to *spec state*
   (coverage/ambiguity/depth/alternatives/contradiction/reuse), per
   `blueprints/Speed-of-Thought-L8-Deep-Dive/02-DIALOGUE-PLANE-BUILD-MODE.md` §4.

### 0.1 What I proved before designing (not assumed)

Everything below was run in this worktree, not inferred from reading.

| Claim | How proven | Result |
|---|---|---|
| relay-py has never run in `Light/` | `ls .venv` in worktree and main tree | **No venv anywhere.** I created one: `orb/backend/relay-py/.venv` (gitignored — `git status` stayed clean) |
| **relay-py S0 baseline** | `.venv/bin/pytest -q` | **399 passed, 0 failed, 5.93s.** Fully green — unlike the JS suite's 2 pre-existing reds (F01's evidence). Any later "still green" claim on this lane is now falsifiable |
| `ResponseMode.BUILD` exists on the wire | `schemas.py:103`, and enum introspection | True: `['converse','focus','teach','build']` |
| …and therefore `mode=build` works | **Drove the real handler** with a scripted gateway | **FALSE. `mode=build` → HTTP 500 today.** |
| root cause | `app.py:100-104` `_MODE_PROMPT_NAMES` | `KeyError(<ResponseMode.BUILD: 'build'>)` — the dict has FOCUS/CONVERSE/TEACH only |
| second root cause | `ls orb/domain/agents/` | No `build.v1.md`. Files present: `atomizer.v1`, `check-in.v1`, `converse.v1`, `focus-companion.v1`, `teach.v1`, `wait-companion.v1.json` |
| focus/converse/teach unaffected | same probe | all three → HTTP 200 |

**This is the single most important finding in this contract.** "The wire already carries `build`" is
true at the *type* level and false at the *behavior* level. Pydantic validates `"build"` — so the
caller gets no 422, no typed refusal, no named reason. It gets a 500. That is worse than an
unsupported mode: it is an *advertised* mode that crashes. `ResponseMode.BUILD` shipped in the enum
with no handler and no test, which is the same "computed everywhere, transmitted nowhere" shape this
repo's own memory already records once
(`one-missing-field-invalidated-three-test-paths`) — one layer further along.

### 0.2 The reachability gap, stated plainly (do not let this get buried)

The task brief asked me to confirm what frame shape `apps/macos` already sends for mode selection.
The honest answer is worse than "it doesn't send `mode`":

> **`apps/macos` does not dial the relay at all today.**

- `apps/macos/Sources/OrbMacCore/Protocol/` contains `WireProtocol.swift`, `RelaySocket.swift`,
  `URLSessionWebSocketTransport.swift` — a faithful, unit-tested mirror of `relay-rs`'s
  `ClientFrame`/`ServerFrame`.
- `grep -rn "RelaySocket(\|URLSessionWebSocketTransport(" apps/macos/ | grep -v Tests` returns
  **nothing**. There is no non-test construction site.
- `OrbMacApp.swift`'s `ContentView` is `OrbView(state: orbState)` with `@State private var orbState:
  OrbVisualState = .booting`, and its own doc comment says driving it "from the protocol client's
  real session state is a later integration unit, not this one's scope."
- Neither Swift's `ClientFrame` nor Rust's `ClientFrame` has a `mode` field at all.
- `relay-rs` (the WebSocket audio plane) does **not** call `relay-py` (the HTTP orchestration plane).
  `main.rs:891` says usage is reported "out-of-band — never a synchronous call on the hot path."
  They are two independent processes with no request path between them.

So the conversation/LLM surface — `POST /v1/respond` on relay-py:8765 — has exactly **one** caller in
the whole estate today: the RN mobile client, over HTTP. `swift run OrbMac` opens a real, stable
window that renders an orb and talks to nothing.

**Consequence for this lane, and it is not negotiable:** F04 makes build mode *real and reachable
over the wire*. It does **not** make OrbMac speak. The manual end-to-end drive in §6 therefore runs
against a real relay-py process with a raw HTTP client, and I say so rather than dressing a Swift
unit test up as an integration. The Swift-side work — give `ClientFrame` a `mode`, give OrbMacCore an
HTTP client for `/v1/respond`, construct a `RelaySocket` from `OrbMacApp` — is a **real, named,
separate follow-up (proposed F04b)**, out of scope for me to build and out of the question to hide.
Without it, `apps/macos` still cannot reach build mode after F04 merges. F37 (two-pane UI) inherits
this gap; F22 (WKWebView pane) does too.

---

## 1. C1 / L2 registry verdict, said out loud

**Verdict: `install` (the mode plumbing) + `extract`-shaped port (the belief math) + `build-new`
(the evidence log and the build session store).**

`Light/registry/{features,services,modules}/` are all **empty** (F01's lead entry in FLEET-LEARNINGS
already established this, and it is why `@pe/llm-gateway` and `@pe/realtime-voice` resolve nowhere).
So the C1 pass runs against the copied tree, not the registry. What it found:

| Capability | Already exists? | Verdict |
|---|---|---|
| `ResponseMode.BUILD` on the wire | **Yes** — `proxy/schemas.py:103` | **install.** Zero new lines. Do not re-add it. |
| `mode` forwarded by the RN client | **Yes** — `ConversationPort.ts:90-93` | **install.** Untouched by this lane. |
| Mode→prompt dispatch | **Yes, but incomplete** — `app.py:100` `_MODE_PROMPT_NAMES` | **extend, 1 line + 1 prompt file.** |
| Domain-prompt loader | **Yes** — `proxy/prompts.py`, `@cache`, fail-closed | **install.** `build.v1` goes through it unchanged. |
| `ModuleBrief`/`DepthScore`/`FreezeRecord`/`KilledAlt`/`Guarantee` | **Yes** — `proxy/lld_schemas.py` (241 lines, from the initial import) | **install.** F02 owns this schema; F04 imports the slot *names* and adds nothing to it. |
| Append-only session ledger with SQLite | **Yes** — `store/freeze_store.py` (no update/delete/prune by design) | **extract the pattern**, new table. Do not extend `FreezeStore` itself — that is F05/F12's surface. |
| Per-session SQLite store keyed `(tenant, user, session)` | **Yes** — `store/conversation_store.py` | **extract the pattern.** Same `ORB_CONTEXT_DB_PATH` db file, new table. |
| Bounded structured-decode → validate → repair-once → fail-closed | **Yes** — `proxy/atomizer.py`, `proxy/lld_decomposer.py` | **install / do not touch.** F03 owns `atomizer.py`. F04 must not open it. |
| **A belief model in Python** | **No.** `grep -rln "belief" orb/backend/ fleet/ apps/macos/ registry/` → 3 files, all eval-gate *thresholds*, zero implementation | **build-new** — a port, not a re-derivation. |

### 1.1 A fake I found while doing the C1 pass — do not wire into it

`eval/gates.py:71-72` declares two gates that read as if a belief model already exists and is
calibrated:

```
EvalGate("belief-calibration.brier",  "belief_brier",                0.15, LE, "score")
EvalGate("belief-replay.coverage",    "belief_replay_coverage_rate", 1.0,  EQ, "ratio")
```

`gates.py:92` sets `"belief_replay_cases": 5_000`, and `gates.py:269` synthesizes those 5,000 cases
by **cycling five hardcoded number pairs**:

```python
"prediction": [0.92, 0.14, 0.48, 0.86, 0.21][index % 5],
"actual":     [1,    0,    0.5,  1,    0   ][index % 5],
```

A Brier score over five values repeated a thousand times is a constant. The gate cannot fail and
measures nothing. This is exactly `fleet/PRINCIPLES.md`'s "a check cheaper to fake than to satisfy
will be faked" and the 447-of-521-receipts scar.

**Instruction to the builder: do not connect F04's registers to `belief_brier` or
`belief_replay_coverage_rate`.** Feeding a real model into a synthetic corpus would launder the fake
into looking earned. F04 ships its own assertions (§5). Retiring or re-basing those two gates on a
real corpus is a separate ticket — flagged here, deliberately not fixed in this lane (the
`assert-the-retirement-not-just-the-replacement` discipline says fixing it properly means proving the
old metric fails on the old data, which is its own unit of work).

---

## 2. Killed alternatives

### 2.1 KILLED — port the FSM/belief logic to Swift as well, and keep both copies in sync

The shape F01 accidentally created, doubled. `apps/macos` would get a Swift `BeliefModel` and a Swift
build FSM; `relay-py` would get the Python one; `apps/mobile` keeps the TS one. Three
implementations of one log-odds update law.

**Why killed.** The update law is arithmetic, and arithmetic drifts silently across ports: this repo
has already paid for exactly that once — F02's lead found the numeric-encoding trap between
`JSON.stringify` and `json.dumps` in the *canonical-JSON hashing* of `lld.v1`, in a mirror pair that
was written deliberately and reviewed. A three-way mirror of a clamped sigmoid with a κ and a λ in it
has no fixture that proves agreement unless someone builds a cross-language comparator, which is F02's
whole scope for a *schema* — far more work than the feature. Worse, the failure mode is invisible:
a Swift `Float` vs a Python `float` diverging at the 7th decimal never crashes, it just makes the orb
ask a different next question on the two clients, and no test anywhere would notice.

The blueprint's own §4.2 requires the dialogue to "replay bit-identically from its evidence stream."
Bit-identical replay across three languages is a research project. Across one, it is a `for` loop.

**Revive trigger:** offline build mode becomes a product requirement (the Mac app must run a design
dialogue with no relay reachable). Then port *and* build the cross-language comparator F02 built for
`lld.v1`, as a C2 extract — never as a hand-copy.

### 2.2 KILLED — a stateful in-process Python object (`dict[(tenant, session)] -> BuildSession`)

The `_meters` pattern in `app.py:46`. Cheapest possible: one module-level dict, a TTL sweep, done.

**Why killed.** Three reasons, in ascending order of how much they cost:

1. `app.py:49-55` already documents this pattern's own defect in its own comment — "the mobile client
   mints a new session_id on every mount and there is no session-end signal today, so without this
   `_meters` grows by one permanent entry per app launch for the life of the process." Copying a
   pattern whose owner wrote a paragraph apologizing for it is a poor default.
2. **A voice client reconnects.** `RelaySocket.swift` has an explicit single-reconnect path
   (`RelayUnexpectedCloseError`, the reconnect transport factory at line ~115). A design dialogue that
   loses every register on a dropped socket is not a feature, it is a demo. F12's acceptance line is
   already "a session's freeze ledger survives a reconnect within the same session" — F04 is where
   that property is either designed in or permanently designed out.
3. **It makes replay unprovable.** In-process mutable state has no artifact to replay *from*. §4.2's
   determinism claim would become untestable at exactly the moment it is asserted.

### 2.3 CHOSEN, and why it beats the obvious middle option — persist the **evidence log**, not the derived registers

The middle option (also killed): a SQLite row per register holding `(value, confidence)`, updated in
place on each piece of evidence. Survives restart, cheap to read.

**Why the log wins.**

- **Replay is free and real.** The registers become `fold(apply_evidence, log, initial())` — a pure
  function of an append-only list. "Same evidence stream replays bit-identically" stops being a claim
  and becomes the definition. Restart-survival and replay-determinism are then *the same property*,
  proven by one test, not two.
- **Dependency invalidation (§4.2) is itself an event.** A revised premise is appended to the log like
  any other entry, so the truth-maintenance history is auditable: you can answer "why is
  `Coverage[acceptance]` only 0.32 confident?" by reading the log, not by guessing. With in-place
  updates that history is destroyed on write.
- **It matches the house style.** `freeze_store.py`'s class docstring is explicit: "No `update`,
  `delete`, `edit`, `set_superseded`, or `prune` method exists on this class." F04's store follows the
  same discipline in the same db file, so there is one persistence idiom in this service, not two.
- **Cost:** the fold is O(n) in evidence count per read. A design dialogue is bounded by
  `SPLIT_TRIGGER_TURNS = 12` (P0 contract §1.4) — n is tens, not thousands. Measure it; do not
  pre-optimize it. If it ever matters, a snapshot row is an additive change that does not invalidate
  the log.

### 2.4 KILLED — invent a second wire contract for mode selection (a new `/v1/build/...` endpoint family)

Tempting because build mode is conceptually a different conversation. Killed because the brief's own
constraint is right: `mode` already flows over the existing wire from the RN client, and the whole
point of F04 is that **one** server-side surface serves both clients. A second endpoint family means
`apps/macos` would be built against a contract `apps/mobile` does not speak, re-creating the fork this
feature exists to close. The existing `/v1/respond` + `/v1/session/warmup` pair already has the exact
two shapes needed (a turn, and a session-open). Extending them additively costs less and forks
nothing.

### 2.5 KILLED — keep `last_evidence_ts` and wall-clock decay "for parity with the TS reference"

`BeliefModel.ts` exports `decay(beliefs, now, table)` with `ATTENTION_CONFIDENCE_HALF_LIFE_MS = 90_000`.
A literal port would carry it over.

**Why killed — this is the "porting the wrong half" trap the task brief warned about, and it is
load-bearing.** §4.2 is explicit: *"A spec does not fade on the clock: a slot filled four minutes ago
is still filled."* Wall-clock decay on a coverage register means a founder who thinks for two minutes
gets asked about a slot they already filled. That is not a tuning error, it is the opposite of the
feature.

So F04 does not port `decay()`. And it goes one step further, which is a **deliberate, disclosed
deviation from §4.1's "keep the representation `(value, confidence, last_evidence_ts)`"**:

> `last_evidence_ts: EpochMs | None` is replaced by `last_evidence_seq: int | None` — the index of
> the evidence-log entry that last moved this register.

**Why.** A timestamp inside a structure whose defining property is bit-identical replay is a live
impurity: it is either injected (and then two replays of the same log differ), or read from a clock
(and then the fold is not pure at all). A sequence number preserves everything `last_evidence_ts` was
actually *used* for here — provenance and "has this ever seen evidence" — while making wall-clock
decay **unrepresentable rather than merely unimplemented**. There is no `now` parameter anywhere in
`belief.py` to pass a clock into. That is the structural version of the rule, and structural beats
documented every time (`enforcement-over-documentation`).

**Revive trigger:** a register is introduced whose quantity genuinely is time-dependent (e.g. "is the
user still in this session?"). That register does not belong in the *spec* vector; it belongs in the
focus product's user vector, which already has the clock.

### 2.6 KILLED — let the belief registers gate the freeze

The obvious "smart" design: when `Coverage` crosses θ and `Contradiction` is low, freeze.

**Why killed — §4.1 names this the firewall and it is the whole reason the design is safe:**
*"beliefs steer the conversation; they never freeze anything."* The freeze verdict is a pure function
of `ModuleBrief` **fields**, so the same fields yield the same verdict regardless of the probabilistic
path that reached them. A wrong belief must be able to waste a question and unable to freeze a bad
spec. F04 therefore ships registers that are **advisory only**, and §5's A5 enforces it *structurally*
(an import-graph assertion), not by comment. Freeze itself is F05's — not mine.

---

## 3. The interface — exactly what changes

Seven files. Three new production modules, one new prompt, two touched files, one new test-support
script. **Estimated ~430 net new production lines** (belief.py ~180, build_session.py ~120,
build_session_store.py ~90, app.py ~35, schemas ~15). Report the real number; do not round into
compliance.

### 3.1 NEW — `orb/backend/relay-py/src/orb_relay/cognitive/__init__.py` + `belief.py`

New package (there is no `cognitive/` in relay-py today). Pure module: **no I/O, no clock, no model
call, no import of `fastapi`, `lld_schemas`, or any `store.*`.**

```python
# Constants — ported verbatim from apps/mobile/src/cognitive/contracts.ts. Same names, same values,
# so a reader can diff the two files by eye.
PRIOR_VALUE = 0.5                  # BeliefModel.ts:23
MAX_ABS_LOGIT = 6.0                # BeliefModel.ts:31 — without it, repeated same-direction
                                   # evidence drives value to exactly 0/1, logit returns ±inf, and
                                   # every later update is NaN: the register silently stops
                                   # responding forever.
CONFIDENCE_GAIN_KAPPA = 0.4        # contracts.ts:59 — blueprint 02 §4.2's "κ ≈ 0.4 [orb]"
MAX_TIER2_EVIDENCE_LOGITS = 0.8    # contracts.ts:57 — |w·r| ≤ 0.8 for model-sourced evidence
DEPENDENCY_INVALIDATION_LAMBDA = 0.6   # blueprint 02 §4.2's "λ = 0.6 [A]". NEW — no TS counterpart.
```

**Registers** (blueprint §4.1, re-aimed from the 9 user beliefs — none of the focus names survive):

```python
class Register(str, Enum):
    COVERAGE      = "Coverage"       # vector over slots — see CoverageSlot
    AMBIGUITY     = "Ambiguity"
    DEPTH         = "Depth"
    ALTERNATIVES  = "Alternatives"
    CONTRADICTION = "Contradiction"  # HARD: 0 to freeze (consumed by F05, not enforced here)
    REUSE_RESOLVED = "ReuseResolved"

class CoverageSlot(str, Enum):
    """The required-slot set. Imported by NAME from the P0 contract §1.4 / lld_schemas.ModuleBrief;
    F04 adds no field to that schema."""
    INTERFACE = "interface"
    DATA_OWNED = "data_owned"
    ACCEPTANCE = "acceptance"
    DEPS = "deps"
    NON_GOALS = "non_goals"
    REGISTRY_VERDICT = "registry_verdict"
```

`Coverage` is keyed per-slot; the other five are scalar. Represent a register key as
`RegisterKey = tuple[Register, CoverageSlot | None]` — one flat dict, no special-casing.

**State and evidence** (frozen dataclasses; `slots=True`):

```python
@dataclass(frozen=True, slots=True)
class RegisterState:
    value: float                    # 0..1
    confidence: float               # 0..1
    last_evidence_seq: int | None   # §2.5 — the log index, NOT a timestamp

@dataclass(frozen=True, slots=True)
class Evidence:
    register: Register
    slot: CoverageSlot | None
    weight: float
    reliability: float              # declared 0..1; CLAMPED on entry, see below
    tier: EvidenceTier              # TIER0 | TIER1 | TIER2

@dataclass(frozen=True, slots=True)
class PremiseRevised:
    """§4.2's truth-maintenance event. Appended to the same log as Evidence."""
    premise: CoverageSlot
    dependents: tuple[CoverageSlot, ...]   # from the caller's dep graph (F05/03 owns the graph)
```

**Functions — all pure, all total:**

| Function | Signature | Law |
|---|---|---|
| `initial_registers()` | `-> Registers` | every key at `(0.5, 0.0, None)` |
| `apply_evidence(regs, ev, seq)` | `-> Registers` | §3.1a below |
| `invalidate(regs, event, seq)` | `-> Registers` | §3.1b below |
| `fold(log)` | `-> Registers` | `reduce` over `Evidence | PremiseRevised`, `seq = index` |

**§3.1a `apply_evidence` — ported verbatim from `BeliefModel.ts:86-118`:**

```
r      = 0.0 if not isfinite(reliability) else min(1.0, max(0.0, reliability))
delta  = weight * r
if tier is TIER2: delta = copysign(min(abs(delta), MAX_TIER2_EVIDENCE_LOGITS), delta)
logit' = clamp(logit(value) + delta, -MAX_ABS_LOGIT, +MAX_ABS_LOGIT)
value' = sigmoid(logit')
conf'  = min(1.0, confidence + CONFIDENCE_GAIN_KAPPA * r)
last_evidence_seq' = seq
```

Keep the TS reliability clamp and its reason — `contracts.ts` declares `[0,1]` but Tier-1 starts from
cosine similarity, whose raw range is `[-1,1]`, so a malformed upstream score must not create negative
confidence. Keep `logit()`'s `1e-6` epsilon clamp. **Only Tier-2 is capped**; Tier-0/1 are
deterministic and uncapped, same as the TS.

**§3.1b `invalidate` — NEW, blueprint §4.2, no TS counterpart:**

```
for s in event.dependents:                 # and ONLY these
    conf_s'  = conf_s * (1 - λ)
    logit_s' = logit_s + (logit(PRIOR_VALUE) - logit_s) * λ      # logit(0.5) == 0.0
    value_s' = sigmoid(logit_s')
    last_evidence_seq' = seq
```

Two properties the implementation must have, both asserted in §5:
- a register **not** in `dependents` is returned **identical** (same object is fine; equal is required);
- `invalidate` never *raises* a register's confidence — λ ∈ (0,1) makes it a contraction toward the
  prior in both coordinates.

### 3.2 NEW — `orb/backend/relay-py/src/orb_relay/build/build_session.py`

The build-mode session behavior, ported from F01's `T0FocusSession.ts`. Pure; no store, no HTTP.

```python
BUILD_OPENING = "Build mode. What are we making?"
```

Ported from `T0FocusSession.ts:106`'s `BUILD_GREETING` — **same string, deliberately**, so the two
clients say the same thing during the transition period and a diff between them is a one-line grep.

**Carry F01's trap forward.** `T0FocusSession.ts:98-105`'s comment is the reason this is a separate
constant and not an entry in a greetings array: `GREETINGS` is indexed
`greetingIndex % GREETINGS.length` with `greetingIndex` seeded from `Date.now()`, so appending build
copy there makes ~1/3 of ordinary *focus* launches speak build copy **with no test failure anywhere**.
The server-side analogue is `app.py:799-804`'s `phrase_manifest`. **Do not add `BUILD_OPENING` to
`phrase_manifest`, and do not give it a `phrase_id`.** F01's lead entry in FLEET-LEARNINGS is explicit
that client `phrase_id`s have no asset table and that `presence.here.v1` already maps to *different*
text client-side vs. server-side — a new `phrase_id` here would claim a cached recording that does
not exist. It is novel speech.

Also exported: `build_slot_dependencies() -> Mapping[CoverageSlot, tuple[CoverageSlot, ...]]` — the
static dep edges F04 needs to construct `PremiseRevised` events. **P0 shape only:** `acceptance` and
`data_owned` depend on `interface`; `acceptance` depends on `data_owned`. The blueprint's real
dependency structure is the design graph in `03` §4, which F05/F02 own — this is a named, minimal
stand-in, and it must carry a docstring saying so rather than pretending to be the graph.

### 3.3 NEW — `orb/backend/relay-py/src/orb_relay/store/build_session_store.py`

Append-only, mirroring `freeze_store.py`'s discipline in the same SQLite file
(`ORB_CONTEXT_DB_PATH`, default `backend/relay-py/.data/context.db`).

```python
class BuildSessionStore:
    """Append-only evidence log for build-mode belief registers.

    No `update`, `delete`, `edit`, or `prune` method exists on this class — the registers are a
    fold over the log (see cognitive/belief.py:fold), so mutating a row would make the current
    state unreproducible from its own history. Same discipline as FreezeStore, same db file.
    """
    def append_evidence(self, *, tenant_id, user_id, session_id, entry) -> int: ...   # -> seq
    def log(self, *, tenant_id, user_id, session_id) -> list[Evidence | PremiseRevised]: ...
    def registers(self, *, tenant_id, user_id, session_id) -> Registers: ...          # == fold(log())
```

Table `build_evidence(tenant_id, user_id, session_id, seq, kind, payload_json)`, primary key
`(tenant_id, user_id, session_id, seq)`. `seq` is per-session and monotonic — **assigned by the
store, never by the caller**, so two concurrent turns cannot collide on an index the fold depends on.
Tenant-scoped on every read (C8) exactly like `ConversationStore.recent`.

### 3.4 NEW — `orb/domain/agents/build.v1.md`

The build-mode system prompt. **Must live in `domain/`** — `prompts.py`'s module docstring states the
placement rule and the reason it exists ("before this module existed, `/v1/respond`'s system prompt
was a literal string in `app.py` — this is the fix"). Loaded by the unchanged
`load_agent_prompt("build.v1")`.

Content requirements (the builder writes the prose; these are the checkable properties):
- Establishes the collaborator register of blueprint §5.1: a proposal carries `why` / `achieves` /
  `pros` / `cons` / `killed alternatives` / one `ask`. `cons` non-empty; ≥2 killed alternatives.
- **One question per turn**, targeting one named slot.
- Never invents a slot value the user did not give. Fail-closed to a clarify — `lld_decomposer.py`'s
  docstring already argues this precisely ("a guessed module brief costs a whole lane, not 30
  seconds") and build mode inherits it.
- Does **not** carry the focus-mode ADHD load-bearing rules from `converse.v1.md` — this is a design
  dialogue with a builder, not a stuck-moment companion. Say so in the file so a future reader does
  not "harmonize" the two.
- Follows the existing files' shape: `# <title>, v1`, a stated purpose, an explicit BAD/GOOD table.

### 3.5 EDIT — `orb/backend/relay-py/src/orb_relay/proxy/schemas.py` (additive only)

```python
class RegisterReading(BaseModel):
    register: str        # "Coverage" | "Ambiguity" | ... ; slot-qualified as "Coverage[interface]"
    value: float
    confidence: float

class BuildTurnState(BaseModel):
    """Advisory only. Blueprint 02 §4.1's firewall: beliefs steer the conversation, they never
    freeze anything. Nothing in this object is an input to any gate."""
    registers: list[RegisterReading]
    next_slot: str | None        # which slot the orb intends to ask about next
    contradiction_blocking: bool # Contradiction register above threshold — F05 consumes; advisory here
```

`ResponseMode` is **not** touched — `BUILD` is already there (§1, install).

### 3.6 EDIT — `orb/backend/relay-py/src/orb_relay/app.py` (four small, additive changes)

**(a) The one-line fix that turns the 500 into a 200:**

```python
_MODE_PROMPT_NAMES: dict[ResponseMode, str] = {
    ResponseMode.FOCUS: "focus-companion.v1",
    ResponseMode.CONVERSE: "converse.v1",
    ResponseMode.TEACH: "teach.v1",
    ResponseMode.BUILD: "build.v1",          # F04
}
```

**(b) Make the class of defect unrepresentable — this is required, not optional.** A dict keyed by an
enum that is missing a member is a 500 waiting to happen, and it just happened. Add, at import time:

```python
_MISSING_MODE_PROMPTS = set(ResponseMode) - set(_MODE_PROMPT_NAMES)
if _MISSING_MODE_PROMPTS:
    raise RuntimeError(
        f"every ResponseMode needs a domain prompt; missing: "
        f"{sorted(m.value for m in _MISSING_MODE_PROMPTS)}"
    )
```

The next person who adds `ResponseMode.REVIEW` gets an import-time crash naming the missing member,
not a 500 in production three weeks later. §5's A6 pins this.

**(c) `respond_to_user`:** when `body.mode is ResponseMode.BUILD` — append this turn's Tier-0 evidence
to the store, fold the log, and populate `ConversationResponse.build`. Token ceiling: build mode uses
`_FOCUS_CONVERSE_MAX_TOKENS` (180) for P0 — a design turn is a proposal plus one question, not a
teach-mode explanation. If it proves too tight, that is a measured follow-up with a number attached,
not a guess now.

**(d) `WarmupRequest`/`WarmupResponse`, additive:**

```python
class WarmupRequest(BaseModel):
    ...
    mode: Annotated[ResponseMode, Field(default=ResponseMode.FOCUS)] = ResponseMode.FOCUS

class WarmupResponse(BaseModel):
    ...
    opening: str | None = None   # non-null iff mode is BUILD; BUILD_OPENING
```

`/v1/session/warmup` is the existing "a session opens" endpoint and already carries
`(tenant, user, session)`. Defaulting `mode` to FOCUS keeps every existing caller byte-compatible
apart from one added `null` field — the same backward-compatibility reasoning `ConversationRequest.mode`
already documents at `app.py:291-298`.

**The response envelope addition:**

```python
class ConversationResponse(BaseModel):
    ...
    build: BuildTurnState | None = None   # non-null iff mode is BUILD
```

### 3.7 NEW (test support, not production) — `orb/backend/relay-py/tests/manual/stub_gateway.py`

A ~40-line `http.server` that answers the gateway's `/v1/complete` contract, echoes a canned
completion, and **writes every received request body to a JSONL file**. Needed because §5's A1 and
§6's manual drive both run a *real relay-py process*, and the real gateway is a Node sidecar whose
`node_modules` is absent in `Light/` (F01's lead entry). Stubbing the upstream **model** while running
the real **relay** is correct here: the claim under test is which system prompt the relay sends, not
what a model replies. `relay-rs/src/provider.rs:736-877` already establishes this exact idiom in this
codebase (raw `TcpStream` HTTP stubs).

It must record the system prompt, because "the relay returned 200" is a proxy; "the build prompt
reached the model boundary" is the property (`proxy-is-not-the-property`).

### 3.8 What does NOT change — the untouched list, mechanically checkable

The lane base is **`d305689`** (= `master` at lane creation; `git merge-base master HEAD` confirms
it). Diff against that explicit hash — **not** against the lane branch, which would compare the
branch to itself and print nothing no matter what changed. (F01's contract got this right with
`e421a59...HEAD`; an earlier draft of this section got it wrong, which is precisely the class of
lead-authored defect F01's verifier caught.)

```bash
cd ".../.worktrees/F04-server-side-build-mode"
git diff --numstat d305689...HEAD -- \
  orb/apps/mobile/ \
  orb/backend/relay-rs/ \
  orb/backend/relay-py/src/orb_relay/proxy/atomizer.py \
  orb/backend/relay-py/src/orb_relay/proxy/lld_schemas.py \
  orb/backend/relay-py/src/orb_relay/proxy/lld_decomposer.py \
  orb/backend/relay-py/src/orb_relay/proxy/conversation_guard.py \
  orb/backend/relay-py/src/orb_relay/store/freeze_store.py \
  orb/backend/relay-py/src/orb_relay/store/conversation_store.py \
  orb/backend/relay-py/src/orb_relay/eval/gates.py \
  orb/domain/agents/converse.v1.md \
  orb/domain/agents/focus-companion.v1.md \
  orb/domain/agents/teach.v1.md \
  apps/macos/ fleet/ registry/
# MUST print nothing. Run it before writing code and again against the committed hash.
```

`atomizer.py` is F03's lane and `freeze_store.py` is F05/F12's — a diff there is a lane collision, not
a bonus.

---

## 4. Reuse inside this repo (do not reimplement)

| Need | Call this | Do not |
|---|---|---|
| Load the build system prompt | `proxy.prompts.load_agent_prompt("build.v1")` | inline the prompt in `app.py` |
| SQLite connect/schema-init idiom | copy `store/conversation_store.py`'s `_connect`/`__init__` | invent a new db path or a second db file |
| Append-only discipline + docstring | `store/freeze_store.py` | add an `update`/`prune` |
| Slot names | `proxy/lld_schemas.py`'s `ModuleBrief` fields | define a parallel slot vocabulary |
| Cost metering | `_get_session_meter` + `reserve_remaining`/`settle`, as `/v1/respond` already does | skip metering on the build path (C12) |
| Safety guard | `complete_guarded_conversation` — the existing call in `respond_to_user` | call `gateway.complete` directly |
| Structured logging | `observability.devlog.dev_log` | `print` |
| Test double for the gateway | `tests/test_app_routes.py`'s `ScriptedGateway` + `app.dependency_overrides` | mock `httpx` |

Note on the safety guard: `complete_guarded_conversation` takes `mode=body.mode.value`, and build mode
is a **new value** reaching it. I traced all four mode-gated branches — see §5's A7, which states the
expected behavior exactly and why it is correct. **Do not edit `conversation_guard.py`**; it is a
safety surface with its own 8/8 distress-case suite. If A7 cannot pass without touching it, escalate.

---

## 5. Acceptance suite — write these FIRST; they are the spec

Three new files. **All are red until the implementation exists — that is correct.** Style follows
`tests/test_app_routes.py` (pytest, `asyncio_mode = "auto"`, `TestClient` + `dependency_overrides`).
Run with `cd orb/backend/relay-py && .venv/bin/pytest -q`.

**The builder may not edit these files.** If a test looks wrong, stop and escalate to the lead —
F08's builder did exactly that on a fixture race and was right to.

### A1 — `tests/test_f04_build_mode_wire.py` — against a REAL relay-py process

Not `TestClient`. A real `uvicorn` subprocess on an ephemeral port, with `ORB_GATEWAY_URL` pointed at
`tests/manual/stub_gateway.py`, driven over real HTTP by `httpx`.

Rationale: `TestClient` runs the ASGI app in-process and would not catch an import-time failure, a
uvicorn-only path, or a broken `ORB_GATEWAY_URL`. The brief requires a real server process, and
`app.py`'s module-level singletons (`_context_store`, `_conversation_store`, `_rates`,
`_MODE_PROMPT_NAMES`) all execute at import — exactly the layer that is broken today.

| Case | Assertion |
|---|---|
| **A1.1** | `POST /v1/respond` with `mode="build"` → **HTTP 200** (today: 500 — this is the regression pin) |
| **A1.2** | The stub gateway's recorded request for that call has a `system` prompt whose text **equals the contents of `orb/domain/agents/build.v1.md`** (modulo the grounding/duplicate suffixes `app.py` appends). Not "contains the word build" — byte equality against the file. This is the "test the wire, not the component" assertion. |
| **A1.3** | The same call with `mode="focus"` sends `focus-companion.v1.md` — the two prompts are **different strings**. A build prompt that silently equals the focus prompt passes A1.1/A1.2-by-substring and is worthless. |
| **A1.4** | Response body has `mode == "build"` and `build` is **non-null** with ≥1 register reading. |
| **A1.5** | `mode="focus"` response has `build == None` — the field is absent-shaped for non-build callers. |
| **A1.6** | `mode="banana"` → **422**, not 500 and not a silent default (pins the existing typed-error contract). |
| **A1.7** | `POST /v1/session/warmup` with `mode="build"` → `opening == "Build mode. What are we making?"`; with `mode` **omitted** → `opening is None` **and every other field byte-identical** to the pre-F04 response for the same input. |

### A2 — `tests/test_f04_belief_registers.py` — the update law, exact values

Pure-function tests, no HTTP, no store. **The expected values below were computed for this contract,
not copied from an implementation** — they are the oracle, so a wrong implementation cannot define
them. Assert with `pytest.approx(..., abs=1e-12)`.

**A2.1 — the scripted evidence sequence on `Coverage[interface]`, from `initial_registers()`:**

| # | Evidence | Expected `logit` | Expected `value` | Expected `confidence` |
|---|---|---|---|---|
| e1 | tier0, w=+1.2, r=1.0 | +1.2 | `0.768524783499` | `0.4` |
| e2 | tier0, w=+1.2, r=0.5 | +1.8 | `0.858148935100` | `0.6` |
| e3 | **tier2, w=+3.0, r=1.0** | **+2.6** | `0.930861579657` | `1.0` |

e3 is the Tier-2 clamp: raw `delta = 3.0` must be capped to `0.8`. If the clamp is missing, `logit`
becomes 4.8 and `value` `0.991837` — a visibly different number, so this case genuinely discriminates.
`confidence` accumulates `κ·r` = 0.4, 0.2, 0.4 → 1.0, pinning **κ = 0.4**.

**A2.2 — saturation, no NaN.** From `logit = 2.6`, apply `(tier0, w=+2.0, r=1.0)` five times.
Expected `logit == 6.0` exactly (clamped at `MAX_ABS_LOGIT`), `value == 0.997527376843`. Then apply
ten more; `value` must be **unchanged and finite** — `math.isnan(value) is False`. This is the exact
defect `BeliefModel.ts:26-32` documents; a port that drops the clamp fails here and nowhere else.

**A2.3 — Tier-0/1 are NOT capped.** `(tier0, w=+3.0, r=1.0)` from the prior gives `logit == 3.0`
(not 0.8). Asymmetry with A2.1's e3 is the point.

**A2.4 — reliability clamp.** `r = -0.5` → treated as `0.0`: `value` unchanged from the prior,
`confidence` unchanged (no negative confidence). `r = float("nan")` → same. `r = 2.0` → treated as
`1.0`.

**A2.5 — λ = 0.6 dependency invalidation.** Drive `Coverage[acceptance]` to `logit = 2.0,
confidence = 0.8`, and `Coverage[deps]` to any distinct state. Apply
`PremiseRevised(premise=data_owned, dependents=(acceptance,))`. Expected:
- `Coverage[acceptance]`: `confidence == 0.32` (`0.8 × 0.4`), `logit == 0.8`,
  `value == 0.689974481128`
- `Coverage[deps]`: **exactly equal to its pre-event state** — invalidation touches only listed
  dependents. This is the assertion that separates dependency-directed backtracking from a blanket
  decay, and it is the one that fails if someone ports `decay()` by reflex.

**A2.6 — no wall-clock decay, structurally.** `inspect.signature` of every public function in
`cognitive.belief` must contain **no parameter named `now`, `timestamp`, `ts`, or `time`**, and the
module source must not import `time` or `datetime`. Blunt, and blunt is right: it makes the §2.5
deviation enforced rather than documented (`enforcement-over-documentation`).

**A2.7 — replay determinism.** `fold(log)` twice over a 40-entry mixed log of `Evidence` and
`PremiseRevised` → results compare `==`. Then `fold` a log with two entries **transposed** and assert
the result **differs** — otherwise A2.7's first half passes trivially on a fold that ignores order.
(Pick a transposition of two entries that touch the same register; the negative control is the point.)

**A2.8 — every register is reachable.** All six `Register` members plus all six `CoverageSlot`
members appear in `initial_registers()`, each at `(0.5, 0.0, None)`. Catches a half-finished port.

### A3 — `tests/test_f04_build_session_store.py` — persistence and restart

| Case | Assertion |
|---|---|
| **A3.1** | `append_evidence` returns `seq` 0,1,2,… monotonic per session; two different `session_id`s each start at 0. |
| **A3.2** | `registers(...) == fold(log(...))` — the store's derived read is exactly the pure fold, not a second implementation. |
| **A3.3** | **Restart survival.** Append 5 entries; construct a **brand-new `BuildSessionStore` against the same file path** (this is what a relay restart is, in-process); `registers(...)` is byte-identical. |
| **A3.4** | **Tenant isolation (C8).** Same `session_id` under two `tenant_id`s → disjoint logs; neither sees the other's evidence. |
| **A3.5** | **Append-only, structurally.** `BuildSessionStore` exposes no attribute named `update`, `delete`, `edit`, `prune`, or `set_*` — same assertion shape `test_freeze_store_append_only.py` already uses. |

### A4 — the focus-mode-unchanged witnesses (no new file; these must stay green)

The **399-test relay-py baseline I established (§0.1)** must be 399 passed with F04's additions on
top, zero regressions. Named witnesses, checked by name not by category:
`test_app_routes.py`, `test_prompts.py`, `test_prompts_carry_the_load.py`, `test_conversation_guard.py`,
`test_conversation_store.py`, `test_freeze_store_append_only.py`, `test_lld_schema_conformance.py`.
`git diff` must show **zero changes** to any of them.

### A5 — the firewall (in `test_f04_belief_registers.py`)

`cognitive/belief.py`'s source must import **none** of: `lld_schemas`, `freeze_store`, `fastapi`,
`gateway_client`, `store.*`. Assert by parsing the module's AST for `Import`/`ImportFrom` nodes —
not by grepping a comment. Blueprint §4.1's firewall ("beliefs steer, they never freeze") becomes a
property of the import graph: there is no code path from a belief value to a freeze verdict because
the module cannot name one.

### A6 — mode/prompt exhaustiveness (in `test_f04_build_mode_wire.py`)

`set(ResponseMode) == set(_MODE_PROMPT_NAMES)`, **and** every value in it resolves through
`load_agent_prompt` without raising. Directly pins §3.6(b) and makes today's defect
non-reintroducible.

### A7 — the safety-guard mode branch (in `test_f04_build_mode_wire.py`)

I traced this rather than leaving it open. `conversation_guard.py` has exactly **four** mode-gated
branches, and `"build"` matches none of them:

| Line | Branch | Applies to build? |
|---|---|---|
| `936` | `mode in ("converse","focus")` → `is_explicit_refusal` short-circuit | **No** |
| `1001` | `mode in ("converse","focus")` → `shifts_mental_load` egress veto | **No** |
| `1086` | `mode == "teach"` → `invites_comprehension_check` requirement | **No** |
| `1201` | `mode in ("converse","focus")` → `shifts_mental_load` on the repair retry | **No** |

So build mode falls through to the **unmoded** path: it keeps every universal control (empty output,
format/markup, shame-adjacent egress, the single bounded repair) and skips the two focus/converse
-specific ones.

**That is the correct behavior, and it must be asserted so it stays true rather than remaining true
by accident.** `shifts_mental_load` vetoes text that hands a decision back to the user — which in
focus mode is the core ADHD defect (`converse.v1.md`'s "you carry the load, never hand it back"), and
in build mode is **the entire point**: blueprint §5.1's `Ask` type exists so every design turn hands
exactly one decision back to the human. If that veto fired on build mode, every legitimate clarify
question would be suppressed and the dialogue could never fill a slot.

Assertions:
- **A7.1** A build-mode turn whose model output is a clarify question of the `shifts_mental_load`
  shape (e.g. "Which storage do you want — Postgres or SQLite?") returns **200, unvetoed**, with
  `degraded is False`. Drive the same text through `mode="converse"` and assert it **is** vetoed —
  the contrast is the assertion; A7.1 alone would pass on a guard that vetoes nothing.
- **A7.2** A build-mode turn whose model output is shame-adjacent **is still vetoed** (universal
  control intact). Reuse an existing distress case from `test_conversation_guard.py`'s corpus by
  reference — do not invent a new one and do not copy the corpus.

**Do not edit `conversation_guard.py`.** It is a safety surface with its own 8/8 distress-case suite
and a red-team history in `app.py`'s comments. If A7 cannot be made to pass without touching it,
stop and escalate to the lead.

### A8 — mutation test (required before claiming green)

Green tests prove nothing until they can go red. Before handoff, apply each mutation, record the
observed failure, revert, confirm green again. Paste the real failure text in the evidence file.

| # | Mutation | Must break |
|---|---|---|
| M1 | `CONFIDENCE_GAIN_KAPPA` 0.4 → 0.5 | A2.1 |
| M2 | delete the Tier-2 clamp branch | A2.1 e3 |
| M3 | delete the `MAX_ABS_LOGIT` clamp | A2.2 |
| M4 | `DEPENDENCY_INVALIDATION_LAMBDA` 0.6 → 0.5 | A2.5 |
| M5 | `invalidate` applies to all registers, not just `dependents` | A2.5's second half |
| M6 | `_MODE_PROMPT_NAMES[BUILD]` → `"converse.v1"` | **A1.3** (not A1.1 — confirms A1.3 earns its place) |
| M7 | `WarmupResponse.opening` always returns `BUILD_OPENING` | A1.7 |

M6 is the one that matters most: it is the "silent fallback to a plausible-looking wrong answer"
failure this estate has been bitten by repeatedly.

---

## 6. How the orchestrator manually drives this end to end (NOT optional)

Every command is literal and runs from the worktree root
`/Users/rachitsrivastava/youtube/Principal Engineering/Light/.worktrees/F04-server-side-build-mode`.
**Foreground or `timeout N`; never background-and-wait.**

### Step 0 — the venv (I already did this; recorded so it is reproducible)

```bash
cd "orb/backend/relay-py"
python3 -m venv .venv                       # already exists in this worktree
.venv/bin/pip install -q -e ".[dev]"
timeout 300 env PYTHONDONTWRITEBYTECODE=1 .venv/bin/pytest -q -o cache_dir=/tmp/f04-pytest-cache
# Baseline recorded 2026-09-07 by the lead: 399 passed, 0 failed, 5.93s
```

### Step 1 — terminal A: the stub gateway (real process)

```bash
cd ".../.worktrees/F04-server-side-build-mode/orb/backend/relay-py"
.venv/bin/python tests/manual/stub_gateway.py --port 8082 --record /tmp/f04-gateway.jsonl
```

### Step 2 — terminal B: the real relay-py process

```bash
cd ".../.worktrees/F04-server-side-build-mode/orb/backend/relay-py"
ORB_GATEWAY_URL=http://127.0.0.1:8082 \
ORB_CONTEXT_DB_PATH=/tmp/f04-context.db \
ORB_DEV_LOGGING=1 \
  .venv/bin/python -m uvicorn orb_relay.app:app --host 127.0.0.1 --port 8765
```

Port 8765 is relay-py's real port (`orb/scripts/dev.sh:23`, `ORB_RELAY_PORT`), so this is the same
process the product runs, not a test harness.

### Step 3 — terminal C: drive it, as a client with no RN and no Swift involved

```bash
# 3a — the session opens in build mode. Expect: "Build mode. What are we making?"
curl -s -X POST http://127.0.0.1:8765/v1/session/warmup \
  -H 'content-type: application/json' \
  -d '{"tenant_id":"t1","user_id":"u1","session_id":"s1","mode":"build"}' | python3 -m json.tool

# 3b — REGRESSION PIN. Run this BEFORE the builder starts: it returns 500 today.
#      After F04 it must return 200 with a non-null "build" object.
curl -s -o /dev/null -w '%{http_code}\n' -X POST http://127.0.0.1:8765/v1/respond \
  -H 'content-type: application/json' \
  -d '{"tenant_id":"t1","user_id":"u1","session_id":"s1","text":"I want a rate limiter","mode":"build"}'

# 3c — the real turn, and the belief registers on it
curl -s -X POST http://127.0.0.1:8765/v1/respond \
  -H 'content-type: application/json' \
  -d '{"tenant_id":"t1","user_id":"u1","session_id":"s1","text":"I want a rate limiter","mode":"build"}' \
  | python3 -m json.tool

# 3d — THE PROXY CHECK. Did the BUILD prompt actually reach the model boundary?
#      Compare what the relay sent against the domain file, byte for byte.
python3 - <<'PY'
import json, pathlib
sent = [json.loads(l) for l in open('/tmp/f04-gateway.jsonl')][-1]['system']
disk = pathlib.Path('../../domain/agents/build.v1.md').read_text().strip()
print("build prompt reached the gateway:", sent.startswith(disk))
print("first 80 chars sent:", sent[:80].replace("\n", " "))
PY

# 3e — focus mode is untouched (the same server, the same session store)
curl -s -X POST http://127.0.0.1:8765/v1/respond \
  -H 'content-type: application/json' \
  -d '{"tenant_id":"t1","user_id":"u1","session_id":"s2","text":"I feel stuck","mode":"focus"}' \
  | python3 -m json.tool     # expect mode:"focus", build:null

# 3f — the registers MOVE across turns (a static object is not a belief model)
for t in "The interface is allow(key) -> bool" \
         "It owns its own redis counter table" \
         "Acceptance: 100 calls in 1s, the 101st is refused"; do
  curl -s -X POST http://127.0.0.1:8765/v1/respond -H 'content-type: application/json' \
    -d "{\"tenant_id\":\"t1\",\"user_id\":\"u1\",\"session_id\":\"s1\",\"text\":\"$t\",\"mode\":\"build\"}" \
    | python3 -c 'import sys,json; b=json.load(sys.stdin)["build"]; print([(r["register"],round(r["value"],4),round(r["confidence"],3)) for r in b["registers"]])'
done
```

### Step 4 — restart survival, driven for real (the §2.3 claim, not asserted)

```bash
# Ctrl-C terminal B. Restart it with the SAME ORB_CONTEXT_DB_PATH. Then:
curl -s -X POST http://127.0.0.1:8765/v1/respond -H 'content-type: application/json' \
  -d '{"tenant_id":"t1","user_id":"u1","session_id":"s1","text":"and it must be per-tenant","mode":"build"}' \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["build"]["registers"])'
# The registers must CONTINUE from step 3f's values, not restart at 0.5/0.0.
# This is the property 2.2's in-process dict would have failed. Watch for it.
```

### Step 5 — `apps/macos`, and the honest result

```bash
cd ".../.worktrees/F04-server-side-build-mode/apps/macos"
swift test 2>&1 | tail -5      # OrbMacCore's own suite — expected green (owner reports 55/55)
swift run OrbMac               # a real macOS window with a rendered orb
```

**State the outcome plainly and do not dress it up:** OrbMac opens a window, renders the orb, and
makes **no network call of any kind** — there is no `RelaySocket` construction site outside tests
(§0.2). Nothing in step 5 exercises F04. Steps 1–4 are the real end-to-end proof, between two real
live processes, with a client that is neither RN nor Swift.

**The OrbMac↔relay drive becomes possible only after the proposed F04b.** If step 5 is ever reported
as "build mode working in the Mac app," that is a false claim — the deliverable is a window that does
not dial anything.

### Step 6 — the gate

```bash
cd ".../.worktrees/F04-server-side-build-mode/orb"
timeout 600 npm run test:py     # requires backend/relay-py/.venv — step 0 created it
```

`npm run verify` (the full orb gate) also runs `lint`/`typecheck`/`test`/`test:rs`, which need
`node_modules` that do not exist in this worktree and are red for reasons disjoint from F04
(F01's evidence: 2 pre-existing failures in `backend/gateway-sidecar`). **Run `test:py` as the F04
gate and say so** — do not claim a full-gate pass this lane cannot produce.

---

## 7. Done-definition

F04 is done when **all** of these hold, each with pasted real output:

1. §5's A1–A8 all pass against a **real uvicorn process**, and the pre-existing **399** relay-py tests
   still pass — 399 + new, zero regressions, named witnesses byte-identical.
2. The A8 mutation table has been run: each mutation's real failure text recorded, reverted, green
   re-confirmed. **M6 especially.**
3. §3.8's untouched-list `git diff --numstat` prints **nothing**, run against the committed hash.
4. §6 steps 1–4 have been driven by the orchestrator on two real live processes, with the actual
   terminal output pasted — including **step 3d** (the build prompt byte-matched at the gateway
   boundary) and **step 4** (registers continued across a real restart).
5. `docs/evidence/F04-server-side-build-mode.md` exists with real command output, the real net-new
   line count (not "~430"), and every judgment call disclosed.
6. `status/F04-Depth-completeness-belief-registers.status` → `verifying`, then `verified` by an
   independent verifier who re-derived rather than re-read.
7. Committed to `lane/F04-server-side-build-mode`, **not merged**. Author ≠ integrator (A15/D4).
   This lane touches a contract surface (`schemas.py`) — human-merge, ship to a PR and stop.
8. A FLEET-LEARNINGS entry at
   `/Users/rachitsrivastava/youtube/Principal Engineering/FLEET-LEARNINGS.md` — **that exact absolute
   path, never one containing `Light/`.**

---

## 8. Assumptions, out-of-scope, and landmines already paid for

### 8.1 Assumptions (falsify before relying on them)

- `orb/domain/agents/` is reachable from relay-py at `parents[5]` — **verified live** by me:
  `DOMAIN_AGENTS_DIR` resolved correctly in this worktree and listed all 6 files.
- Python 3.14.7 is what runs here (`pyproject` requires ≥3.12). The 399-test baseline was taken on it.
  The `starlette`/`httpx` deprecation warnings in that run are pre-existing and unrelated.
- `ORB_CONTEXT_DB_PATH` is shared by `ContextStore` and `ConversationStore` today; F04 adds a third
  table to the same file. If a future lane splits the file per store, F04's table follows — it holds
  no cross-table foreign key.
- The dep graph in §3.2 is a **named P0 stand-in**, not the design graph. `03` §4 owns the real one.

### 8.2 Explicitly OUT of scope

- **F05** — the freeze protocol, propose→pushback→❄, `BuildClarifyProtocol`'s `SPLIT_TRIGGER_TURNS`,
  the `FREEZE_CONFIRM`/`FROZEN` states. F04 supplies registers that *steer*; it freezes nothing (§2.6).
- **F03** — the atomizer repoint. Do not open `proxy/atomizer.py`.
- **F09** — the orb→fleet handoff. F04 emits no SOW and calls no fleet binary.
- **F02** — the `lld.v1` schema and its cross-language mirrors. F04 imports slot names, adds no field.
- **F06** — the `lld-ready` gate and `depth_score`. F04's `Depth` register is a *conversational
  steering signal*; F06's `depth()` is a pure predicate over fields. Same word, different objects, and
  conflating them would breach §2.6's firewall.
- **The build FSM.** `BuildStateMachine.ts` exists client-side (P0 contract §4). Porting the 8-state
  table server-side is real work and is **not** F04 — F04 is session state + registers. Flagged so it
  is not silently assumed done.
- **Swift-side work (proposed F04b).** `ClientFrame.mode`, an HTTP client in `OrbMacCore`, a
  `RelaySocket` construction site in `OrbMacApp`. Named in §0.2, not built here.
- **Retiring the fake `belief-*` eval gates** (§1.1). Flagged, deliberately not fixed.
- **Streaming.** F18–F20. `/v1/respond` stays request/response.

### 8.3 Landmines already paid for — do not rediscover

1. **`mode=build` is a 500, not a 422.** Proven (§0.1). Any test asserting "the wire supports build"
   by checking the enum is asserting nothing.
2. **The greeting-array trap.** `T0FocusSession.ts:98-105` — appending build copy to an
   index-modulo-length greetings array makes ~1/3 of *focus* launches speak build copy with no test
   failure. Server-side analogue: `app.py`'s `phrase_manifest`. Keep `BUILD_OPENING` separate, no
   `phrase_id` (§3.2).
3. **`npm install` from a worktree eats a sibling's `node_modules` symlink** — `file:` deps resolve
   from the real path (F01's lead entry). The Python venv has no such hazard; §6 step 0 is safe.
4. **Never `git stash` in this tree.** Other lanes have live worktrees on it.
5. **`_meters`' own TTL comment** (`app.py:49-55`) is the argument against §2.2, written by the person
   who shipped it.
6. **The eval gates named `belief-*` are fixture-synthesized** (§1.1). Do not connect to them.
7. **The A3-style contract defect F01's verifier caught.** F01's contract specified an acceptance test
   that was *impossible* because the lead assumed a render path that did not exist. Countermeasures
   taken here: every §5 assertion is grounded in something I ran or read in this worktree — the 500 was
   driven, the prompt dir was listed, the four guard branches were grepped by line number, the A2
   numbers were computed, and §3.8's diff command was executed to confirm it can print nothing on a
   clean tree *and* is anchored to a real base hash. **If any assertion in §5 still turns out to be
   impossible as written, that is my defect, not the builder's: stop and escalate.**

---

## 9. Handoff

**Route:** mid-engineer (Sonnet) — this is integration + math-port + debugging across three layers,
not mechanical scaffold. Then an independent verifier (Sonnet, never Haiku), who must re-derive rather
than re-read, and must personally run §6 steps 1–4 against two live processes.

**Sequencing:** F04 collides with nobody as written. F03 owns `atomizer.py`, F05 owns `freeze_store.py`,
F02 owns `lld_schemas.py` — §3.8's untouched list keeps all three clear. `schemas.py` is additive-only
(new models, `ResponseMode` untouched), so an F02 lane editing `lld_schemas.py` and an F04 lane editing
`schemas.py` do not overlap. Confirm before starting.

**Order of work:** (1) create §5's three test files verbatim and watch them go red for the *expected*
reasons — a test red for the wrong reason is not a starting point; (2) `cognitive/belief.py` until A2
and A5 are green (pure, fastest feedback, no server); (3) the store until A3 is green; (4) the prompt
file and the `app.py` wiring until A1/A6 are green; (5) A7 after reading the guard, escalating first;
(6) the A8 mutation table; (7) evidence file; (8) commit, do not merge.
