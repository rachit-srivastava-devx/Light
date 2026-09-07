# F09 — Orb→Fleet handoff wiring: the missing stamped-`Freeze` producer

**Lane:** `lane/F09-handoff` · **Worktree:** `Light/.worktrees/F09-handoff` · **Repos:** orb + fleet
**Author:** lead-architect (Opus 5) · **Date:** 2026-09-07 · **Phase:** contract only, no implementation
**Merge policy:** human-merge always (A15/D4) — this lane appends a `verify.sh` stage, adds a keel
authority command, and edits the orb wire contract. Ship to a PR and stop.

---

## 0. Preflight — what I measured, not what the dispatch claimed

F07's own learnings entry records that *"worktree state claimed in a dispatch is a claim, not a
fact."* Measured here before anything else:

| check | command | result |
|---|---|---|
| worktree ancestry | `git rev-list --count HEAD..master` / `master..HEAD` | `0` / `0` — **exactly on master** |
| ancestry predicate (not `git log`) | `git merge-base --is-ancestor master HEAD` | YES |
| tree clean | `git status --porcelain` | empty |
| F06 present | `ls fleet/keel/fleet/src/lld_ready.rs` | present, 19504 bytes |
| F05 present | `ls orb/backend/relay-py/src/orb_relay/build/freeze_protocol.py` | present, 15198 bytes |
| F07 present | `grep '"--lld"' fleet/keel/fleet/src/main.rs` | `main.rs:265` — the arm exists |
| F07/F06/F08 suites present | `ls fleet/keel/fleet/tests/` | `f06_lld_ready.rs`, `f07_sow_intake.rs`, `f08_pr_emit.rs` |

So F01–F08 really are merged here and this lane is not blocked. **Two corrections to the dispatch,
both recorded rather than silently worked around:**

1. **The status filename in the dispatch does not exist.** The dispatch says
   `status/F09-Orb-Fleet-handoff-wiring.status`; the real file is
   `status/F09-Orb--Fleet-handoff-wiring.status` (**double** dash — the slugifier turned `Orb→Fleet`
   into `Orb--Fleet`). This is the identical class of error F05's lead recorded for its own lane
   (`--free.status`, not `--freeze-.status`). Reading before writing avoided creating a second,
   orphan status file that `render.sh` would have counted as a 45th feature.
2. **`/usr/bin/cat` does not exist on this box.** F07's entry already records this and records that
   `>` truncates the target *before* the command on its left resolves, destroying a tracked file.
   Every read in this contract used the `Read` tool or `python3`; no `cat`, no `>` onto a tracked path.

---

## 1. Restatement

F05 stops at a **`FreezeProposal`** (`proposed_by: Literal["orb:dialogue"]`) and its own §8.2 says so
in as many words: *"Emitting a stamped `Freeze` / an `lld.v1`. §2.1. The missing keel producer is
flagged in §9."* F07 accepts a **real `lld.v1` file** via `fleet sow --lld <PATH>` and turns it into a
SOW at exit 9. Between those two merged, independently-verified lanes there is **nothing**:
`fleet gate lld-ready` prints a human verdict line and exits — it emits no JSON, writes no file, and
allocates no `freeze_id` (verified by reading `main.rs:3834-3862`, `print_gate_verdict`, which returns
`Ok(())` after a single `println!`).

F09 builds exactly that missing producer and wires it, so that **a real ❄ freeze in a live orb
build-mode session ends with a real fleet SOW record on disk that keel itself will load and accept.**

The authority constraint is not negotiable. Blueprint `03` §3.4: *"The gate owns the graph. It is the
single writer of every stamped field… Authority ops (`freeze.stamp`, `state.advance`) have **no
proposer-emittable form**."* `03` §2.3: *"Forging readiness requires being the gate, which requires
being `keel`."* Therefore the STAMP is produced by a keel binary acting as the gate, and **the orb's
Python never writes `stamped_by`.**

---

## 2. C1 verdict (registry-first, C1/L2)

**Registry check, run first:** `Light/registry/features/` and `Light/registry/services/` are **empty
directories** — F06's lead recorded this and it still holds. There is no registry candidate to
install. So the C1 decision is made against the *code* estate:

| piece | verdict | what is reused, verbatim |
|---|---|---|
| the 14-check readiness gate | **install** | `lld_ready::check_and_evaluate(&brief, &refs)` — zero edits to `lld_ready.rs` |
| the shape validators | **install** | `lld::validate_module_brief`, `lld::validate_lld_v1` — zero edits to `lld.rs` |
| the content hasher | **install** | `lld::content_hash(&module_brief)` — the real F02 hasher, never re-derived |
| gate reference data loading | **install** | `load_gate_refs()` (`main.rs:3763`) — reads `owners.v1.json` + `gate-refs.v1.json` |
| gate refusal printing | **install** | `print_gate_verdict()` (`main.rs:3834`) — so refusals are byte-identical to `fleet gate lld-ready` |
| SOW intake | **install** | `fleet sow --lld` — **zero edits to `sow.rs` or `create_sow_from_lld`** |
| the subprocess-to-keel discipline | **extract-in-spirit** | F05's `readiness.py` pattern (fail-closed, exit-code-only, hard timeout) is *followed*, not modified |
| **the stamped-`Freeze` producer** | **build-new** | `fleet freeze stamp` — the artifact F05 flagged as absent |
| **the orb-side handoff step** | **build-new** | `build/handoff.py` + one block in `app.py` |

**Verdict: install for the entire checking path; build-new for exactly two seam pieces.** Nothing on
the decision path is re-implemented. **Zero new contract schemas** — the output is F02's existing
`lld.v1`, and the input is the same bare `ModuleBrief` file shape `fleet gate lld-ready` already
takes, which is what lets `handoff.py` reuse `readiness.py`'s serialisation call unchanged.

---

## 3. Findings that change this lane (each proved against the tree, not recalled)

### 3.1 `fleet sow --lld` already re-runs the readiness gate — so the stamp is not the security boundary

`main.rs:456-487` (F07's step 5-6) calls `lld_ready::check_and_evaluate(&module_brief, &refs)` and
refuses on `NotReady`/`MeasuredNothing`. Combined with F07's other four guards, intake independently
re-derives **shape** (`validate_lld_v1`), **hash** (`lld::content_hash`, recomputed — `main.rs:394-420`),
**owner agreement across all three copies**, **registry-verdict agreement**, and **the full 14-check
gate**. Consequence for this lane: `stamped_by` is a **provenance label**, not a signature, and the
real authority is intake-side re-derivation. The stamp still must be keel-authored, because the
blueprint's structural claim depends on it and because `freeze_id`/`version`/`content_hash`/
`depth_evidence` genuinely cannot be computed by a proposer — but a contract that sold the *string*
as the security property would be selling a proxy.

### 3.2 The blueprint's central forgery claim is **false in the tree today**, and one line from being exploited

`03` §4.1 and `freeze.v1.json`'s own description both assert *"there is no proposer path that writes
this string."* Measured:

- `orb/backend/relay-py/src/orb_relay/proxy/lld_schemas.py:230` defines a complete Python `Freeze`
  model whose last field is literally `stamped_by: Literal["keel:lld-ready"]`.
- The same file defines `SowSeed` (`:252`) and `LldV1` (`:262`) — the whole wrapper.
- The same file defines `canonical_json` (`:651`) and **`content_hash` (`:679`)** — a byte-exact
  mirror of Rust's, kept honest by `lld-crosslang.sh`.

So the orb can today construct a fully schema-valid, **correctly-hashed**, gate-passing `lld.v1` with
`stamped_by: "keel:lld-ready"` from Python, and every one of F07's five guards would pass it — because
each guard checks that the file is *internally consistent*, and a proposer with a correct hasher
produces an internally consistent file. `forged_freeze.json` catches only the string `"orb:lld-ready"`
— the estate again tested the one variant a *careless* forger produces (F07's own lesson, recurring
one layer up).

`Literal["keel:lld-ready"]` is a type annotation the proposer itself satisfies, not a guard against
the proposer. **This is not fixable inside F09** — see §11.2 for why, and §7.2/§8 M4 for the strongest
guarantee this lane *can* deliver (a census ratchet plus a red-under-mutation test that the orb's own
modules never write the literal).

### 3.3 `freeze.decision` and `freeze.why` have **no source field anywhere in the tree**

`check_freeze` enforces both as non-empty strings (`lld.rs:560-565`). Measured against every candidate
source:

- `module-brief.v1.json` is `additionalProperties: false` and its 16 required fields contain **no
  `decision` and no `why`**. They cannot be added by a consumer.
- F05's `FreezeProposal` carries `{proposed_by, node_id, content_hash, brief, readiness,
  clarify_turns_used, residual_ambiguity, coverage}` — **neither field**.
- `DecomposeOutcome` (`schemas.py:121`) carries `{kind, brief, missing, rejections}` — **neither field**.

I ran the projection test mechanically rather than eyeballing it: for `complete_freeze.json`, a
recursive search of `module_brief` for each freeze field's exact value returns:

```
freeze.node_id             -> brief source: ['node_id']
freeze.owner               -> brief source: ['owner']
freeze.decision            -> *** NO MECHANICAL SOURCE ***
freeze.why                 -> *** NO MECHANICAL SOURCE ***
freeze.killed_alternatives -> *** NO MECHANICAL SOURCE ***
freeze.accepts_when        -> *** NO MECHANICAL SOURCE ***
freeze.depth_evidence      -> *** NO MECHANICAL SOURCE ***
sow_seed.restatement       -> *** NO MECHANICAL SOURCE ***
sow_seed.blind_suite_seed  -> *** NO MECHANICAL SOURCE ***
sow_seed.blast_radius      -> *** NO MECHANICAL SOURCE ***
sow_seed.owner             -> brief source: ['owner']
sow_seed.registry_verdict  -> brief source: ['registry']
```

Blueprint `03` §6 says *"the mapping is mechanical."* **Against the shipped schemas it is not** — the
third instance of F07's lesson (*a blueprint mapping table can cite field paths the shipped schema
does not have*). Two of those ten are shape transforms that *are* mechanical once written down
(`accepts_when`/`blind_suite_seed` from the flat `brief.acceptance`; `depth_evidence` from the gate's
own `Verdict`), one is a straight copy the table simply mis-named (`blast_radius` ←
`failure_story.blast_radius`, which F07's entry already corrected), and `killed_alternatives` ←
`alternatives` is a straight copy of an identical `$defs/killedAlt` with an identical `minItems: 2`
floor. That leaves **three genuine prose gaps: `decision`, `why`, `restatement`.** §6 resolves them
and §4.3 records the alternative that was killed.

**Critical corollary the builder must not get wrong:** `complete_freeze.json`'s `freeze` was
hand-authored before any gate existed (`AUTHORSHIP.md`), and its `killed_alternatives` are a
**different list** from its brief's `alternatives` (verified — they share no entry). The fixture is
therefore **not** a specification of the projection. **Do not write a test asserting that stamping
`complete_freeze.json`'s brief reproduces `complete_freeze.json`'s freeze.** It will not, and a
builder who assumes it will either fail the lane or — far worse — edit a read-only fixture.

### 3.4 `compile_task_text` concatenates `restatement + decision + why + purpose`

`sow.rs:264-274`:

```rust
let restatement = sanitize(
    &format!(
        "{} Decision: {} Why: {} Purpose: {}",
        str_at(sow_seed, "restatement"),
        str_at(freeze, "decision"),
        str_at(freeze, "why"),
        str_at(module_brief, "purpose"),
    ),
    "restatement",
    false,
)?;
```

This is why §6's formulas must produce *distinct* strings for the three prose fields: set all three to
`purpose` and the compiled SOW restatement is `purpose` four times over. It also means the three
derived strings must survive `sanitize` (`sow.rs:181-196`), which refuses: empty-after-collapse
(`EMPTY_AFTER_NORMALIZE`), any `|` (`SOW_TEXT_PIPE_COLLISION`), and the words `predicate` or
`acceptance` followed by `:` or `=` (`SOW_TEXT_MARKER_COLLISION`, via `contains_marker`,
`sow.rs:204-218`). §6's literals contain none of those; brief *content* that does is already F07's
named refusal surface and stays so.

### 3.5 crew.sow has **no enforced "not an echo" bar**

`03` §6 and F07's contract both lean on the SOW's *"not an echo"* bar. `validate_sow`
(`fleet/crew/crew/sow.py:367-400`) refuses on exactly: empty `restatement`, not-exactly-one acceptance
predicate per leaf, an out-of-range clarification line, a generic clarification, and
`len(alternatives) < 2`. **There is no echo check.** Recorded because it cuts both ways: it means
§6's projection cannot be refused for redundancy (so the lane is buildable), and it means the
redundancy is a *quality* cost this contract must disclose rather than a *correctness* risk a gate
would catch. Fourth instance of "a schema/blueprint description is prose, not an enforced invariant."

### 3.6 There is no freeze ledger, and `freeze.version`/`supersedes` cannot be computed without one

`grep -rn "freezes" fleet/keel/fleet/src/*.rs` → **zero hits.** `state_dir()` (`main.rs:2217`) creates
`runs/ artifacts/ attestations/ ledger/ oracles/ sows/` — no `freezes/`. But `freeze.v1` **requires**
`version` (*"STAMPED. Monotonic per node_id"*) and **requires** `supersedes` (*"null for v1, the prior
`freeze_id` otherwise"* — required-and-nullable, a deliberate F02 tightening so *"you cannot forget to
consider it"*). Both are unsatisfiable without per-`node_id` state.

So the freeze ledger is **forced by the schema**, not scope creep: without it, re-freezing a node
after a spec change emits a second `version: 1` with a different `content_hash` and `supersedes: null`,
silently violating blueprint §4.2 rule 3 (*"supersede is a link, not a delete"*) — a defect designed
in at contract time. Blueprint §4.3 already specifies the location: *"appended to `$STATE/freezes/` —
outside every worktree, so a build cannot edit the spec it is judged against."*

### 3.7 The gate never reads `decision` or `why`

`grep -n 'decision\|"why"' lld_ready.rs` → 2 hits, **both inside comments** (`:61`, `:188`). So the two
prose fields carry zero authority and are checked by nothing. This is what makes §6's
derive-from-enforced-content choice defensible: deriving them from fields the 14 checks *do* enforce
makes them more trustworthy than free prose, not less.

### 3.8 `readiness.py` dumps with `model_dump(mode="json")` and **no** `exclude_none`

`readiness.py:95`. F05's contract flagged the null-emission question as *unknown and not to be
assumed*; F05's verified evidence settled it empirically — real briefs dumped this way pass keel's
shape layer and freeze for real. **`handoff.py` must use the byte-identical call.** Any divergence
(adding `exclude_none=True`, dumping the wrapper instead of the brief) risks the failure mode F05
named: the stamp shape-rejects where the gate accepted, and every unit test stays green.

### 3.9 A new keel module needs a real dispatch caller

`graph.rs:1254`'s `every_shipped_module_is_reachable_from_dispatch` fails on a module with no caller —
this is why `contract_lld_validate` and `gate_command` exist at all (their own comments say so). The
`freeze_command` arm in `main.rs` satisfies it. `lld`/`lld_ready` are declared in **`lib.rs`** (`:3`,
`:4`), not in `main.rs`'s `mod` block — so `pub mod freeze;` goes in `lib.rs`. `CMDS` (`main.rs:4944`)
is the help/completion table and must gain `freeze`.

---

## 4. Killed alternatives

### 4.1 The orb stamps its own freeze in Python — **killed (structural)**

`lld_schemas.py` already has `Freeze`, `SowSeed`, `LldV1`, `canonical_json` and `content_hash`
(§3.2), so this is ~30 lines and needs no keel change at all. It is the cheapest path and it is the
one this lane exists to refuse.

*Why killed:* it makes the proposer the author of authority, which blueprint `03` §3.4 kills by name
(*killed alternative (a): the Orb owns the graph*) and which `forged_freeze.json` exists as a negative
fixture against. §3.1 shows intake would still re-derive every property — so the *file* would be
sound — but the estate's whole defence against a model fabricating status is that the authority field
has no proposer-emittable form. Writing one voluntarily is the `fleet-rs` I1 forgery the blueprint
cites (*"seven adapters each wrote the authority field they were supposed to earn"*), volunteered.
*Failure story:* the next lane that needs "was this freeze really gated?" has no answer, because the
only evidence was a string the proposer typed. *Revive trigger:* none. If keel is ever removed from
the loop entirely, `freeze.v1`'s `stamped_by` const must be renamed first, in a human-merged schema
change — the rename is the decision, not a side effect.

### 4.2 Extend `fleet gate lld-ready` with a `--stamp` flag — **killed (design)**

Additive, and safe for F05's `readiness.py` (which reads only the exit code, so a new flag cannot
disturb it). Genuinely tempting: one command, one gate run.

*Why killed:* it makes an evaluation command a state-mutating command. F06's gate is specified as a
**pure function** whose CLI *"is the I/O layer; the gate itself never reads its own reference data"*
(F06 contract §3.5) — and `freeze stamp` must write the ledger, allocate a monotonic `version`, and
link `supersedes`. Folding writes into `gate lld-ready` means every readiness consultation becomes a
potential write; F05's `readiness.py` calls it on **every BUILD turn**, so a `--stamp`-adjacent bug
would append ledger entries mid-dialogue. Blueprint §3.4 also names the authority op `freeze.stamp`
directly, so a separate verb matches the spec's own noun. *Revive trigger:* if a later lane proves
two separate gate runs (one to evaluate, one to stamp) is a measured latency problem on the freeze
turn — at which point the fix is a `--stamp` flag *plus* moving the purity guarantee somewhere it is
still tested, not the flag alone.

### 4.3 `decision`/`why`/`restatement` carried as proposer content on `FreezeProposal` — **killed (hashing)**

The honest case for it is strong: `freeze.decision`/`why` are the only two fields in `freeze.v1` with
**no `"STAMPED."` marker** in their schema description while all eleven neighbours have one, and the
gate reads neither (§3.7) — so they read as *content*, and the orb, having just held a real design
dialogue, is the one party that actually knows what was decided and why.

*Why killed:* `content_hash` is `sha256(canonical_json(module_brief))` — *"the brief and nothing
else — ever"* (F02 §6.2, quoted in `lld.rs:758-759`). Prose riding on the proposal rather than the
brief would be **un-hashed content inside the one object whose entire purpose is to be hash-bound**:
alterable at will with every F07 guard still passing. That is F07's own finding reproduced
deliberately. It also forces a second change — the orb has no source for that prose either
(`DecomposeOutcome` carries only `brief`, §3.3), so it would mean extending F03's LLM decomposer, i.e.
putting an *unmeasurable* quality claim on the critical path for two fields **no check reads**.
*Revive trigger:* a lane that first extends `content_hash` to cover a `freeze_content` sub-object — an
F02 schema change, human-merge — after which the prose is hash-bound and this becomes the better
design. Named forward in §11.

### 4.4 Asynchronous handoff — a queued job the freeze turn returns immediately from — **killed (no runtime, and it makes the proof racy)**

*Why killed:* no job runner, queue, or poll endpoint exists in `relay-py`; building one is a lane of
its own. Worse, it would make F09's own acceptance test racy — the property to prove is *"the freeze
turn produced an observable SOW"*, and with an async job the test must poll and pick a timeout, which
is exactly the "sleep until it probably worked" shape this estate has been burned by. Synchronous
also gives the operator the SOW id on the same turn they froze.
*Disclosed cost:* two subprocess round-trips (stamp + intake) land on the freeze turn's request path.
This is a text turn, not the audio path (same boundary F05 drew), but it is real latency. §5.3
therefore requires that the handoff **can never delay or swallow the freeze proposal**: the proposal
is returned regardless, and a slow or failed handoff surfaces as a named outcome. *Revive trigger:* a
measured freeze-turn p95 over budget on real briefs — then move to a background task with a poll
endpoint, and keep §7.3's observability assertions by polling the same on-disk record.

### 4.5 Have F09 make `fleet sow --lld` refuse a freeze whose `freeze_id` does not resolve in the ledger — **killed (out of lane, and it breaks a frozen fixture)**

This is the one change that would close §3.2 **structurally**: a proposer-authored freeze would be
refused at intake because its `freeze_id` resolves to no keel-written ledger entry.

*Why killed:* (a) `create_sow_from_lld` is F07's file and F07 is merged and independently verified —
editing it puts this lane's diff inside another lane's contract surface; (b) it would break F07's own
green suite, because `complete_freeze.json`'s `freeze_id` is the hand-written literal
`fz-0123456789abcdef`, which resolves in no ledger and must keep passing (`fleet contract lld
validate` and `f07_sow_intake.rs` T1 both depend on it), and the fixtures are read-only; (c) it needs a
migration story for freezes stamped before the ledger existed. *Revive trigger:* a lane that owns
`sow.rs` and can add the check behind a `--require-ledger-freeze` flag defaulted off, then flipped on
once the fixtures carry ledger-resolvable ids. **Named forward in §11.2 with the honest label:** the
guarantee F09 ships here is `mitigates`, not `kills_structural`.

---

## 5. The interface (exact)

### 5.1 keel — `fleet freeze stamp`

```
fleet freeze stamp <brief.json> --out <lld.json>
```

Both arguments **required**; no default output path, no stdout JSON. The rationale for `--out` being
mandatory rather than "JSON on stdout": `print_gate_verdict` is reused verbatim for refusals and it
writes its verdict line to **stdout**, so a success-path JSON document on stdout would be one code
change away from being corrupted by a diagnostic. Keeping the artifact in a file also means the orb
passes a *path* to `fleet sow --lld` and **never holds the stamped bytes** — which is what makes
§7.2's "the orb never authored a stamp" assertion a statement about data flow, not just about strings.

**Input:** a bare `ModuleBrief` JSON file — deliberately the *same* shape `fleet gate lld-ready <file>`
takes (`main.rs:3752`, `gate_lld_ready_file`), so `handoff.py` reuses `readiness.py`'s serialisation
call unchanged (§3.8) and this lane adds **no new input contract**.

**Steps, in this order.** Order is load-bearing: **no `--out` file and no ledger entry may exist on
any refusal path.**

| # | step | reuses |
|---|---|---|
| 1 | read + parse `<brief.json>` | — |
| 2 | recursive **key** scan for a forbidden authority key | new (§5.2) |
| 3 | shape-validate the brief | `lld::validate_module_brief` (via `check_and_evaluate`) |
| 4 | run the readiness gate | `lld_ready::check_and_evaluate(&brief, &refs)`, `load_gate_refs()` |
| 5 | compute `content_hash` | `lld::content_hash(&brief)` |
| 6 | resolve `version` / `supersedes` / idempotency from the ledger | new (§5.4) |
| 7 | project brief + verdict → `freeze` + `sow_seed` → `lld.v1` | new (§6) |
| 8 | self-check: `lld::validate_lld_v1(&wrapper)` must return **zero** violations | `lld::validate_lld_v1` |
| 9 | append the ledger entry + receipt, then write `--out` atomically (temp + rename) | `receipt.v1.json` convention |

Step 8 is a producer self-check, not decoration: the producer must never emit an artifact its own
consumer would reject. If it ever fires, that is a projection bug and the command must exit
`EXIT_INVARIANT` with the violation list, not write the file.

**Exit codes.** Every code that can arise from a condition `fleet gate lld-ready` also reports **must
match it** — F07 §8's rule, quoted in `main.rs:456-461`: *"the same condition must not report two
different codes depending on which command the operator typed."*

| exit | constant | condition | stdout / stderr |
|---|---|---|---|
| `0` | `EXIT_OK` | stamped, or an idempotent re-stamp | stdout: `FREEZE_STAMPED freeze_id=<fz-…> node_id=<…> version=<N> content_hash=<sha256:…> out=<path>` |
| `3` | `EXIT_ENV` | unreadable file · not valid JSON · `FLEET_STATE` unset/empty · ledger or `--out` I/O failure · `owners.v1.json`/`gate-refs.v1.json` unreadable | stderr: `fleet: freeze stamp: <reason>` (mirrors `gate_lld_ready_file`'s wording) |
| `6` | `EXIT_INVARIANT` | gate `NotReady` **or** `MeasuredNothing`; or step 8 self-check fails | `print_gate_verdict`'s exact output, unmodified |
| `7` | `EXIT_REFUSAL` | usage error; `STAMP_KEY_PRESENT`; `FREEZE_UNHASHABLE` | usage text, or `freeze_stamp_refused: reason=<TOKEN> …` (F07's `sow_lld_refuse` shape) |
| `8` | `EXIT_MISMATCH` | brief shape-invalid (`EntryOutcome::ShapeInvalid`) | `print_gate_verdict`-adjacent: per-violation lines on stderr, `SHAPE INVALID (…)` on stdout |

Constants confirmed at `main.rs:31-36`: `EXIT_OK=0`, `EXIT_ENV=3`, `EXIT_INVARIANT=6`,
`EXIT_REFUSAL=7`, `EXIT_MISMATCH=8`.

### 5.2 `STAMP_KEY_PRESENT` — a recursive key scan, justified by evidence

Refuse (exit 7) if **any** of `stamped_by`, `freeze_id`, `content_hash`, `depth_evidence`, `state`,
`version` appears as a **JSON object key at any depth** in the input brief.

Why this is not redundant with existing checks: `_FORBIDDEN_MODULE_BRIEF_FIELDS`
(`lld_schemas.py:330`) and its Rust twin reject those names **only at the brief's top level**. F07
proved the nested hole exists and named the exact graft point: *"`guarantees[].derivation` is never
type/key-checked by `validate_module_brief` (only `kind`/`calc`/`enforced_by` are ever read), so
grafting the numeric leaf there passes shape."* The same slot takes a nested `"stamped_by"` just as
well. A proposer sneaking authority-shaped keys into a brief must be refused before anything is
stamped over it.

Two implementation requirements, both from recorded scars:

- It is a **key** scan (walk object keys), **not** a substring grep. F05 recorded that
  `grep -c stamped_by` over a real response returns `1` from a `guarantees[].claim` *string* that
  merely describes the rule — a false positive on real data. The contract's own suggested check was
  itself a proxy.
- The scan must report the **path** of the offending key, not just that one exists.

### 5.3 orb — `build/handoff.py` (new) and one block in `app.py`

`handoff.py` defines:

```python
class HandoffOutcome(str, Enum):
    SOW_READY   = "sow_ready"    # stamp exit 0 AND sow --lld exit 9
    NOT_STAMPED = "not_stamped"  # stamp refused (any non-zero)
    NOT_ACCEPTED= "not_accepted" # stamp ok, sow --lld refused (any exit != 9)
    UNAVAILABLE = "unavailable"  # FLEET_BIN unset / not executable / FileNotFoundError
    TIMEOUT     = "timeout"      # either subprocess exceeded its budget

class FleetHandoff:
    def __init__(self, fleet_bin, *, timeout_s: float = ...) -> None: ...
    def hand_off(self, brief: ModuleBrief) -> HandoffResult: ...

class UnavailableHandoff:
    """FLEET_BIN unset or not executable. ALWAYS returns UNAVAILABLE — never SOW_READY."""
```

**It must not modify `readiness.py` and must not add a method to F05's `ReadinessGate` Protocol** —
`UnavailableReadinessGate` and every test fake implement that Protocol, so widening it breaks F05's
verified T5 suite. The ~15 lines of tempfile+subprocess shape are duplicated deliberately; the reason
goes in a docstring, because the two calls have different semantics (one is a pure consultation, the
other mutates keel's ledger) and F05's module is specified as the former.

It inherits F05's §4.2 discipline verbatim, and the contract restates it because it is the property
that matters most:

1. **Only the documented success code is a pass** — stamp exit `0`, intake exit `9`. Nothing else.
2. **stdout is never parsed for authority.** `freeze_id` and `sow_id` are read from stdout *as
   identifiers to echo and to locate files with*; the *decision* that the handoff succeeded comes from
   the exit codes alone.
3. **`UNAVAILABLE` refuses.** Never "assume stamped", never skipped, never silently downgraded.

**`app.py` wiring** — one block, immediately after `freeze_proposal` is constructed (the
`app.py:1087-1097` region), inside the same `if` that already guards a non-`None` proposal:

- `ConversationResponse` gains **one** optional additive field: `handoff: SowHandoff | None = None`,
  non-null only on the turn a freeze proposal is non-null.
- `SowHandoff` (new model in `proxy/schemas.py`, additive): `{outcome: HandoffOutcomeWire, sow_id: str
  | None, freeze_id: str | None, node_id: str, freeze_version: int | None, detail: str}`. Same
  wire-mirror-enum idiom `AskKindWire` already established (`schemas.py:136-146`) — a separate enum,
  not a re-export.
- **The handoff can never delay or swallow the freeze.** The `freeze` field is populated first and
  returned regardless of handoff outcome; every failure path yields HTTP 200 with `freeze` non-null
  and `handoff.outcome` naming the failure. An exception inside the handoff is caught, mapped to an
  outcome, and logged — never propagated.
- `_FLEET_BIN` (`app.py:112`) is reused; the handoff is resolved once at import, same fail-closed
  shape as `_readiness_gate` (`app.py:113-114`).

### 5.4 The freeze ledger

```
$FLEET_STATE/freezes/<node_id>/<version>.json     # the complete lld.v1, verbatim
```

Plus one receipt appended to the existing `$FLEET_STATE/ledger/` per the `receipt.v1.json` convention
already in the tree. No new schema: the ledger entry **is** the `lld.v1` document.

- `version` = `1 + max(existing versions for node_id)`, or `1` if none.
- `supersedes` = the `freeze_id` of `version - 1`, or `null` when `version == 1`.
- **Idempotency (required):** if the latest entry for `node_id` has the **same `content_hash`**,
  re-stamping returns that entry unchanged — exit `0`, same `freeze_id`, same `version`, **no new
  ledger entry**. Only a *different* `content_hash` allocates a new version. This is blueprint §4.2
  rule 2 exactly (*"any change to a sourced field changes `content_hash`, which the gate detects as a
  new proposed spec"*), and without it an ordinary retry inflates versions and fabricates a supersede
  chain.
- `freeze_id` = `"fz-" + blake3(content_hash + "@" + version).to_hex()[..16]`, satisfying
  `^fz-[0-9a-f]{16}$`. Deterministic, which is what makes idempotency observable. `blake3` is already
  a workspace dependency (`sow::id_for_task`).

---

## 6. The projection (pinned exactly — a builder who "improves" a formula must fail a test)

F07's recorded lesson: *"emit an exact literal and pin it with an exact-match test, so a builder who
'improves' it to `1 engineer-day` fails."* Same discipline here. `B` = the input `module_brief`,
`V` = the gate's `Verdict`.

### 6.1 `freeze`

| field | value | kind |
|---|---|---|
| `schema_version` | `"1.0"` | literal |
| `freeze_id` | `"fz-" + blake3(content_hash + "@" + version).to_hex()[..16]` | **stamped** |
| `version` | ledger-allocated (§5.4) | **stamped** |
| `node_id` | `B.node_id` verbatim | copied |
| `content_hash` | `lld::content_hash(B)` | **stamped** |
| `decision` | `B.purpose` verbatim | derived |
| `why` | `"Chosen over " + N + " killed alternatives: " + join("; ", B.alternatives.map(a → a.option + " — " + a.why_killed))`, where `N = B.alternatives.len()` | derived |
| `killed_alternatives` | `B.alternatives` verbatim | copied (identical `$defs/killedAlt`, identical `minItems: 2`) |
| `accepts_when` | `{ predicate: { given: B.acceptance.given, when: B.acceptance.when, then: B.acceptance.then, oracle_kind: B.acceptance.oracle_kind }, artifact: B.acceptance.artifact }` | shape transform (flat → nested) |
| `owner` | `B.owner` verbatim | copied |
| `depth_evidence` | `{ score: V.score, checked_check_ids: lld_ready::GATE_CHECK_IDS }` | **stamped** (gate output) |
| `supersedes` | prior `freeze_id`, or `null` at `version == 1` | **stamped** |
| `stamped_by` | `"keel:lld-ready"` | **stamped** |

The separator in `why` is an em dash `—` (U+2014), **not** a pipe: `sanitize` refuses any `|` in every
field (§3.4).

### 6.2 `sow_seed`

| field | value |
|---|---|
| `restatement` | `"Build " + B.node_id + " — registry verdict " + B.registry.kind + ". Interface: " + join(", ", B.interface.map(i → i.name)) + ". Fails if: " + B.failure_story.trigger` |
| `blind_suite_seed` | **byte-identical to `freeze.accepts_when`** — blueprint §6 maps `accepts_when.{predicate,artifact}` into the blind-suite seed by name, and `lld.v1.json:19` `$ref`s the same `acceptsWhen` def |
| `blast_radius` | `B.failure_story.blast_radius` verbatim (F07's entry already corrected `03` §6's wrong path here) |
| `owner` | `B.owner` verbatim — must equal `freeze.owner`, or F07's step-4 `OWNER_SPLIT` guard refuses at intake |
| `registry_verdict` | `B.registry` verbatim — must equal `module_brief.registry`, or F07's step-5 `REGISTRY_VERDICT_SPLIT` guard refuses |

`restatement` deliberately contains **no `B.purpose`**, because `compile_task_text` appends
`"Purpose: " + B.purpose` itself (§3.4). `decision` is `B.purpose`, so `purpose` appears twice in the
compiled task text and not four times.

### 6.3 Disclosed imprecision (say it, don't hide it)

`decision`, `why` and `restatement` are **mechanical derivations, not the hand-written mini-ADR prose
a human would author.** Blueprint §4.3 says the accumulating freeze ledger *is* the low-level design,
so shallow prose there is a genuine cost and this contract will not pretend otherwise.

What makes the trade acceptable, stated precisely rather than waved at:

- The freeze's **substance** — `killed_alternatives`, `accepts_when`, `depth_evidence`,
  `failure_story`, `guarantees`, `registry` — is carried **verbatim** from a brief that just passed
  all 14 checks. Nothing substantive is synthesised.
- Every derived string is a **pure function of hash-covered brief content**, so it is transitively
  bound by `content_hash`. There is **no un-hashed content and no proposer-settable authority
  anywhere in the emitted freeze** — the strongest structural position available without an F02
  schema change (§4.3).
- No check anywhere reads `decision` or `why` (§3.7), so the derivation cannot make any gate weaker.

**Revive trigger:** the §4.3 path — extend `content_hash` to cover a `freeze_content` sub-object
(F02 schema change, human-merge), then carry real dialogue-authored prose hash-bound. Until then this
stays a derivation and stays labelled one.

---

## 7. The acceptance suite — this IS the spec (T1/T2)

**Written by the lead, before implementation. It is supposed to be red until the builder makes it
green.** The builder may not edit `tests/contract` or `tests/large/acceptance` equivalents; in this
tree that means **`fleet/keel/fleet/tests/f09_freeze_stamp.rs`,
`orb/backend/relay-py/tests/test_f09_handoff.py`, and `fleet/tests/acceptance/orb-freeze-to-sow.sh`
are lead-owned and read-only to the builder once written.** Any diff touching them must be flagged.
`fleet/contracts/fixtures/lld/**` is **read-only to this lane, entirely** — not even an
`AUTHORSHIP.md` row; the builder proves non-modification with
`git diff <merge-base> -- fleet/contracts/fixtures/lld/` in the evidence bundle.

Per F06's and F07's established precedent, the table below **is** T1: it fixes exact exit codes, exact
strings, and exact pass/fail behaviour in prose before any code exists, and the builder mechanically
translates it into the test files. Fixtures needed beyond the read-only corpus are built **in-memory
or in a tempdir**, never by editing the corpus.

### 7.1 keel producer — `fleet/keel/fleet/tests/f09_freeze_stamp.rs`

Run: `cargo test --manifest-path fleet/keel/Cargo.toml --test f09_freeze_stamp`

| id | property | expected |
|---|---|---|
| **F09-T1** | happy path on the READY control fixture `complete_module.json` (read-only) | exit `0`; stdout matches `FREEZE_STAMPED freeze_id=fz-[0-9a-f]{16} node_id=… version=1 content_hash=sha256:[0-9a-f]{64} out=…`; the `--out` file exists |
| **F09-T2** | **the producer/consumer contract** — `lld::validate_lld_v1(<out>)` returns **zero** violations, **and** the real compiled binary run as `fleet sow --lld <out>` exits **`9`** and writes exactly one record under `$FLEET_STATE/sows/` | zero violations; exit `9`; 1 record. *The single most important test in this lane: it proves the producer's output is what the merged consumer accepts, end to end, with no fixture in between.* |
| **F09-T3** | keel authored the stamp, not the input | `<out>.freeze.stamped_by == "keel:lld-ready"`, **and** the input brief file's own bytes contain **zero** occurrences of `keel:lld-ready` (assert on the fixture text directly) |
| **F09-T4** | **exit-code parity with `gate lld-ready`** — census over all four shared conditions: a `NotReady` brief, a shape-invalid brief, a nonexistent path, a malformed-JSON file | for each: `freeze stamp` and `gate lld-ready` return the **same** exit code, **and** no `--out` file is created. Denominator asserted `== 4` (a census, not a sample — `gate-census-not-sample`) |
| **F09-T5** | `MeasuredNothing` never stamps | via F06's `pub` seam `lld_ready::evaluate_with(&brief, &refs, &[])` → the projection function must refuse; assert the freeze is not produced and the outcome is `MeasuredNothing`. (Not reachable through the CLI — `check_and_evaluate` always passes 14 checks — which is exactly the unreachable-branch trap F06 fixed for the TS gate and must not be reintroduced here) |
| **F09-T6** | version + supersedes | stamp `B` → `version=1`, `supersedes=null`; mutate a hashed field of `B` in memory (→ different `content_hash`), stamp again → `version=2`, `supersedes == v1.freeze_id`; **both** ledger entries exist at `$FLEET_STATE/freezes/<node_id>/{1,2}.json` |
| **F09-T7** | idempotency | re-stamp the byte-identical brief → exit `0`, **same** `freeze_id`, **same** `version`, ledger still holds exactly **1** entry for that `node_id` |
| **F09-T8** | nested authority-key graft is refused | a brief with `guarantees[0].derivation.stamped_by = "keel:lld-ready"` (the exact slot F07 proved is never type-checked) → exit `7`, reason token `STAMP_KEY_PRESENT`, the offending **path** named in the message, no `--out` file, no ledger entry |
| **F09-T9** | numeric leaf | a brief with a JSON number grafted at `guarantees[0].derivation` (F07's own finding: a top-level graft is caught earlier by shape, so it would test the wrong layer) → exit `7`, reason `FREEZE_UNHASHABLE`, no `--out` file, no ledger entry |
| **F09-T10** | **the projection is exact** | for `complete_module.json`, assert **every one of the 13 freeze fields and 5 sow_seed fields** byte-for-byte against §6's formulas, including the full `why` and `restatement` strings as literals. *This is the test that fails when a builder "improves" a formula.* Explicitly **not** an assertion that `complete_freeze.json`'s freeze is reproduced (§3.3) |
| **F09-T11** | `FLEET_STATE` unset | exit `3`, the actionable message printed (never silent — `state_dir`'s own D21/B8 lesson), no `--out` file |
| **F09-T12** | **census ratchet on the forgery literal** | scan every file under `fleet/keel/fleet/src/**` and `orb/backend/relay-py/src/**`; the literal `keel:lld-ready` may appear **only** in `freeze.rs` (the stamp constant), `lld.rs` / `lld_schemas.py` (validator constants), and `lld_ready.rs`. Assert the count of files scanned is **> 0** (measuring nothing is a failure) and that the set of files containing the literal **equals** the allowed set — equality, not `contains` |
| **F09-T13** | no partial artifacts | for **every** refusal path in T4/T8/T9/T11: `<out>` does not exist afterwards, and the ledger is unchanged; on success, `<out>` was written via temp+rename (assert no `*.tmp` residue) |

### 7.2 orb handoff — `orb/backend/relay-py/tests/test_f09_handoff.py`

Run: `cd orb/backend/relay-py && .venv/bin/pytest -q` · requires
`export FLEET_BIN=<built keel binary>` and `FLEET_STATE=<tempdir>`

| id | property | expected |
|---|---|---|
| **F09-T14** | a real freeze turn hands off | driven over **real HTTP** (uvicorn subprocess + a disposable stub gateway — never `TestClient`, per F05's own manual-drive discipline): on the turn `freeze` is non-null, `handoff.outcome == "sow_ready"`, `handoff.sow_id` matches `^[0-9a-f]{64}$`, `handoff.freeze_id` matches `^fz-[0-9a-f]{16}$`, `handoff.freeze_version == 1` |
| **F09-T15** | **`UNAVAILABLE` refuses, one layer up** | with `FLEET_BIN` unset: the session **never freezes** (F05's existing property, unchanged), `handoff` is null, and **zero** files exist under `$FLEET_STATE/sows/` and `$FLEET_STATE/freezes/`. Then repeat with `FLEET_BIN` pointing at a **disposable copy** of the binary that has been `chmod -x`'d mid-session (never the real one — F05's 5e): the turn that would have frozen returns HTTP 200, `freeze: null`, `blocking: ["DEPTH"]`, no stack trace, and still zero sow records |
| **F09-T16** | the orb never transmits or holds a stamp | a **recursive key scan** (walk dict keys only — F05 proved a substring grep yields a false positive from a `guarantees[].claim` string) over the complete response JSON finds **zero** `stamped_by` keys; and `handoff` carries only ids/paths — assert it has no `freeze` object and no `lld.v1` sub-document |
| **F09-T17** | a `NOT_READY` brief never reaches intake | a schema-valid, keel-**invalid** brief (bad `owner`, F05's T5.2/T7.9 construction) driven for real turns → never freezes, `handoff` null, zero sow records, zero ledger freezes |
| **F09-T18** | the handoff never swallows the freeze | force a stamp timeout (`timeout_s` set very low, or a wrapper binary that sleeps) → HTTP 200, `freeze` **non-null**, `handoff.outcome == "timeout"`, no stack trace in the relay log |
| **F09-T19** | **AST proof the orb cannot stamp** | parse `build/handoff.py` and `app.py` with `ast` (F05's T6.5 technique, which asserts import absence by parsing the module's own AST): no call constructing `lld_schemas.Freeze`, `LldV1` or `SowSeed`, and the literal `keel:lld-ready` absent from both files |

### 7.3 the real drive — `fleet/tests/acceptance/orb-freeze-to-sow.sh`

This is the lane's headline claim and the only test that proves it. **A log line is not the property.**
One appended stage in `fleet/verify.sh`.

1. Build the real keel binary; `export FLEET_BIN`, `FLEET_STATE=<tempdir>`.
2. Start a **real** `uvicorn` relay subprocess and a **real** disposable stub gateway (two genuine OS
   processes; `curl` drives the turns — F05's verified pattern, reproduced independently by F05's
   verifier).
3. Drive real `/v1/respond` BUILD turns until the freeze turn.
4. **Observe the SOW, four independent ways** — noting that `fleet sow list` / `fleet sow show`
   **do not exist** in this tree, so these are the real observables and none of them is a log line:
   - **(a)** exactly **one** new record under `$FLEET_STATE/sows/`, and its `lld_provenance.node_id` /
     `.freeze_id` / `.content_hash` equal the values in `$FLEET_STATE/freezes/<node_id>/1.json` and
     the `freeze_id` the response reported;
   - **(b)** `fleet sow accept --id <id>` exits `0` and prints `SOW_ACCEPTED id=… by=… at=…` —
     **keel's own reader loading and mutating the record**, the strongest available "this is a real
     SOW, not a file I wrote";
   - **(c)** the accept receipt is appended to `$FLEET_STATE/ledger/`;
   - **(d)** **round-trip identity:** `fleet sow --task "<the exact task text the stamp path
     compiled>"` yields the **same 64-hex id** (`sow::id_for_task` is `blake3(task)`, deterministic) —
     proving the record's identity derives from the real compiled text rather than from anything the
     test invented. This is F07's own acceptance-script pattern, reused.
5. **Negative control in the same script** (a run that proves the measurement can fail): repeat step 3
   against a `chmod -x`'d **disposable copy** of the binary → the session does not freeze, and
   `$FLEET_STATE/sows/` gains **zero** records. Then restore the executable bit and confirm a fresh
   session freezes and hands off normally (the gate is not sticky).

---

## 8. Mandatory mutation battery

Green tests prove nothing about a test's power. Each mutation below must be applied to the **real
implementation** (never to a fixture), shown to turn the **named** test red with the predicted
failure, then reverted with `git diff` proven empty. F05's lane found that **three of eleven**
mutations initially passed for the wrong reason — a green mutation run on the first attempt is a
finding, not a pass.

| # | mutation | must turn red |
|---|---|---|
| **M1** | delete the recursive `STAMP_KEY_PRESENT` scan | F09-T8 |
| **M2** | hardcode `version = 1` | F09-T6 |
| **M3** | make `supersedes` always `null` | F09-T6 |
| **M4** | **in `handoff.py`, build the `lld.v1` in Python (`lld_schemas.LldV1(...)` with `stamped_by="keel:lld-ready"`) and skip the keel stamp entirely** | F09-T19 **and** F09-T12 **and** F09-T3. *The mutation that proves the authority model is enforced rather than merely intended — this is the §4.1 alternative, applied as an attack* |
| **M5** | change the `why` formula (drop the `"; "` join separator, or the `" — "`) | F09-T10 |
| **M6** | catch-and-null the freeze when the handoff fails | F09-T18 |
| **M7** | remove the idempotency `content_hash` comparison | F09-T7 |
| **M8** | treat any non-zero stamp exit as success | F09-T15 **and** F09-T17 |
| **M9** | replace the recursive key scan with a substring `grep` | F09-T8 must still pass, **and** a new sub-case asserting a brief whose `guarantees[].claim` merely *mentions* `stamped_by` is **accepted** must go red — F05's exact false positive, pinned so it cannot be reintroduced |

---

## 9. Files owned (no overlap with any other lane)

| path | action |
|---|---|
| `fleet/keel/fleet/src/freeze.rs` | **new** — projection, ledger, freeze_id, idempotency, key scan |
| `fleet/keel/fleet/src/lib.rs` | edit — add `pub mod freeze;` (`lld`/`lld_ready` live here, not in `main.rs`) |
| `fleet/keel/fleet/src/main.rs` | edit, **additive only** — `freeze_command` dispatch arm, usage text, `CMDS` entry (`:4944`) |
| `fleet/keel/fleet/tests/f09_freeze_stamp.rs` | **new** — lead-owned, read-only to builder once written |
| `orb/backend/relay-py/src/orb_relay/build/handoff.py` | **new** |
| `orb/backend/relay-py/src/orb_relay/proxy/schemas.py` | edit, **additive only** — `SowHandoff` + `HandoffOutcomeWire` + one optional field on `ConversationResponse` |
| `orb/backend/relay-py/src/orb_relay/app.py` | edit — **one block** inside the existing BUILD/freeze branch |
| `orb/backend/relay-py/tests/test_f09_handoff.py` | **new** — lead-owned, read-only to builder once written |
| `fleet/tests/acceptance/orb-freeze-to-sow.sh` | **new** — lead-owned, read-only to builder once written |
| `fleet/verify.sh` | edit — **one** appended stage |
| `docs/lane-contracts/F09-orb-fleet-handoff.md` | this file |
| `status/F09-Orb--Fleet-handoff-wiring.status` | this lane's own status file only |

**Must not be touched — if you are editing one of these, you have left the lane:**
`fleet/keel/fleet/src/lld.rs` · `lld_ready.rs` · `sow.rs` · `create_sow_from_lld` and every other
existing `main.rs` function · `fleet/contracts/*.json` (every schema) ·
**`fleet/contracts/fixtures/lld/**` (entirely, including `AUTHORSHIP.md`)** ·
`orb/.../build/freeze_protocol.py` · `build/readiness.py` · `proxy/lld_schemas.py` ·
`proxy/lld_decomposer.py` · any prior lane's tests · any other lane's status file.

---

## 10. Done-definition

1. All **13** keel tests, all **6** orb tests green; the exact commands and counts pasted into the
   evidence file.
2. `bash fleet/tests/acceptance/orb-freeze-to-sow.sh` exits `0`, with the **real** transcript pasted:
   the freeze turn's response, the `FREEZE_STAMPED` line, the sow record's `lld_provenance`, the
   `SOW_ACCEPTED` line, the round-trip id equality, and the negative control's zero-record result.
3. **Red-on-arrival proven**, not asserted — copy the three test files into a disposable
   `git worktree add --detach` at this lane's **merge-base** (`git merge-base HEAD master`, *not*
   `master`, which moves; F05's verifier hit exactly this) and record the real failure mode. Skips
   are not passes: F05's convergence file honestly reported `7 skipped` at base because no keel binary
   existed there, and that was recorded as a skip, never counted as green.
4. **All 9 mutations** in §8 proven red-then-reverted, each with the observed failure message and a
   clean `git diff` after revert. M4 and M9 are not optional.
5. `git diff <merge-base> -- fleet/contracts/fixtures/lld/` is **empty**.
6. Full `bash fleet/verify.sh` run in the **foreground**, `$?` read **directly and never through a
   pipe** (`verify.sh:20` warns about this and F06's verifier still made the mistake once). Every
   failing stage traced to a **file:line disjoint from this lane's files**, cross-checked against the
   already-recorded pre-existing failure set: `fmt`/`clippy` in `lifecycle.rs`+`main.rs`, the
   intermittent `f08_pr_emit.rs` git-push race (now independently reproduced by four sessions), a
   `repl.rs` coverage-only flake, 2 `gitleaks` findings on commit `8f86440`, 1 `semgrep` finding at
   `registry-reference/registry/features/memory/memory_store.py:184`, and `corpus.sh`'s
   `registry-reference/` scoring. **Trace by content, not by citation** — after this lane's insertions
   shift line numbers, `diff <(grep -n '<snippet>' original) <(grep -n '<snippet>' current)` on the
   exact flagged text is what separates "pre-existing, just shifted" from "introduced by me" (F07's
   method).
7. `S0` environment established by the **documented** steps only: `cd orb && npm install` (expect 662
   packages) and `orb/scripts/setup.sh:145-149`'s venv lines. A fresh worktree has no `node_modules`
   and no `.venv`; `lld-crosslang.sh` failing there is environment, not regression — three prior lanes
   recorded this identical symptom.
8. `status/F09-Orb--Fleet-handoff-wiring.status` → `STATUS=verifying`, then a dated FLEET-LEARNINGS
   entry at the **root absolute path** `/Users/rachitsrivastava/youtube/Principal Engineering/FLEET-LEARNINGS.md`
   (never `Light/FLEET-LEARNINGS.md`), then `bash Light/status/render.sh`.
9. Commit to `lane/F09-handoff`. **Do not merge.** Human-merge always (A15/D4).

---

## 11. Out of scope

### 11.1 Not this lane

- **The design-graph / console rendering — F21 (`design-graph.v1` + `lane-status.v1` push) and F22
  (the `WKWebView` pane). Explicitly not F09's.** F09 writes the freeze ledger; rendering it is F21/F22.
- **`design-graph.v1` itself**, node upserts, edges, `state.advance`, the lifecycle FSM (`03` §3.2).
  F09 stamps a freeze; it does not advance a node's `state`.
- **F10, the capstone.** F09 delivers freeze → SOW. F10 drives SOW → built → attested → PR and
  measures the wall clock. Do not start it here.
- **F12** (freeze ledger as a session object / reconnect survival) — F09's ledger is keel-side, under
  `$FLEET_STATE`; the orb-session mirror is F12's.
- **Real ADR prose from the dialogue** — §4.3/§6.3, gated behind an F02 `content_hash` change.
- **F17's depth-bar tightening**, the planner role (F27), registry extraction (F28/F29).
- **Voice, latency, audio.** Nothing here is on the audio path (F05's boundary, unchanged).
- **`blake3` parity with `03` §4.1's `content_hash`.** F02 shipped `sha256:` and that divergence is
  F02's, already merged, not F09's to re-litigate.

### 11.2 Known-open, deliberately not closed here — say it plainly

**A hand-authored `lld.v1` with `stamped_by: "keel:lld-ready"` and a correctly-computed
`content_hash` is still accepted by `fleet sow --lld`.** §3.2 proves the proposer path exists today
(`lld_schemas.py` ships `Freeze`, `LldV1`, `SowSeed` **and** a byte-exact `content_hash`), and §4.5
explains why F09 cannot close it: the fix is a ledger-resolution check inside F07's
`create_sow_from_lld`, and it would break `complete_freeze.json`'s frozen `fz-0123456789abcdef`.

So the guarantee this lane ships is labelled honestly:

| guarantee | label |
|---|---|
| every field of an F09-stamped freeze is either keel-computed or a pure function of hash-covered brief content | **kills_structural** |
| the orb's own modules never construct a `Freeze`/`LldV1` and never write `keel:lld-ready` | **kills_mechanical** (F09-T12 census ratchet + F09-T19 AST check + M4) |
| *no* `lld.v1` reaching `fleet sow --lld` was hand-authored | **mitigates** — not enforced. Named revive trigger: a lane owning `sow.rs` adds `--require-ledger-freeze`, defaulted off, flipped on once the fixtures carry ledger-resolvable ids |

Blueprint `03` §2.3's *"Forging readiness requires being the gate"* and `freeze.v1.json`'s *"there is
no proposer path that writes this string"* are **aspirational, not true in this tree.** Recorded here
so the next reader inherits the fact rather than the claim.

---

## 12. Landmines (each one already cost someone a session)

1. **`/usr/bin/cat` does not exist on this box**, and `>` truncates its target *before* the command on
   its left resolves — F07 destroyed a tracked file this way. Use the `Write` tool.
2. **`grep` is `ugrep` under the Bash tool** but `/usr/bin/grep` inside a script. Probe the way the
   script will run.
3. **Never `git stash`** on this tree (prior dispatches' standing landmine). Back up to plain files
   and diff byte-for-byte on restore.
4. **`git merge-base HEAD master`, not `master`** — `master` moves mid-lane when a sibling merges, and
   a naive `git diff master` then pulls in another lane's files. F05's verifier hit this live.
5. **`--is-ancestor`, not `git log`** — `git log --oneline -12`'s topological window can omit a merged
   commit entirely. F07 raised a false alarm this way.
6. **Do not background-and-wait.** A subagent cannot receive its own backgrounded child's completion
   notification; `verify.sh` runs foreground or under `timeout N`, never `&`.
7. **`$?` after a pipe is the pipe's status.** `verify.sh:20` warns about it; F06's verifier still got
   it wrong once and caught it before reporting.
8. **A fresh worktree has no `orb/node_modules` and no `relay-py/.venv`.** `lld-crosslang.sh` will fail
   there for environment reasons three lanes have already recorded.
9. **`f08_pr_emit.rs` has a real, pre-existing intermittent git-push race.** Different sub-tests fail
   on different runs. Four independent sessions have now logged it on four commits. It is not yours.
10. **`canonical_json`/`content_hash` raise on *any* numeric leaf at any depth** (F02 §6.3). Hash the
    `ModuleBrief` and **nothing else, ever**. The freeze itself is never hashed — `depth_evidence`
    contains the only JSON numbers in the whole contract.
11. **`guarantees[].derivation` is never type/key-checked** by `validate_module_brief` (only
    `kind`/`calc`/`enforced_by` are read). It is the graft point for both T8 and T9 — and a top-level
    graft would be caught by shape first, testing the wrong layer.
12. **A recursive key scan, never a substring grep**, for `stamped_by` (F05's false positive; pinned by
    M9).
13. **`complete_freeze.json`'s freeze is not the projection's expected output** (§3.3). Do not assert
    reproduction, and do not "fix" the fixture.
14. **`handoff.py` must not widen F05's `ReadinessGate` Protocol** — `UnavailableReadinessGate` and
    every test fake implement it.
15. **`load_gate_refs()` resolves `contracts/` from `env!("CARGO_MANIFEST_DIR")`** — a compile-time
    path. The acceptance script must use the binary built in this tree, not a copied-elsewhere one.

---

## 13. Routing and merge policy

| role | model | scope |
|---|---|---|
| build | **mid-engineer (Sonnet)** | logic + integration across two languages, a new keel command, subprocess wiring, a real HTTP drive — not mechanical |
| verify | **verifier (Sonnet, never Haiku)** | independent PASS/FAIL, re-derived from this contract and the evidence file only, never the builder's transcript |

Split into **two briefs** if the builder's session runs long; the seam is clean and the file sets do
not overlap:

- **F09a (keel producer):** `freeze.rs`, `lib.rs`, `main.rs`, `f09_freeze_stamp.rs`. Done when T1–T13
  and M1–M3, M5, M7 are green/proven. Self-contained — needs no orb change.
- **F09b (orb handoff):** `handoff.py`, `schemas.py`, `app.py`, `test_f09_handoff.py`,
  `orb-freeze-to-sow.sh`, `verify.sh`. Depends on F09a. Done when T14–T19, the §7.3 drive, and M4, M6,
  M8, M9 are green/proven.

**Human-merge always (A15/D4).** This lane appends a `verify.sh` stage, adds a keel authority command,
and edits the orb wire contract (`proxy/schemas.py`). Ship to a PR and stop.
