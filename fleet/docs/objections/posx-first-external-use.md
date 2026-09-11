# Objections — first use of `fleet` against an external, non-Rust repo

**Author:** Rachit Shah's session, driving `fleet` on the POSX estate (Mokobara).
**Date:** 2026-09-10 · **Binary:** `fleet-cli 0.1.0`, commit `babce1c8edeb`, tree clean.
**Target repo:** `posx-mokobara-backend` — Node 20 / TypeScript / Medusa v2, ~1,200 source files.
**Use case:** validate a real feature SOW (Shopify customer order history) and run the pipeline
over the repo as a pre-merge security gate.

This is, as far as I can tell, the **first time fleet has been pointed at a repo that is not this
one**. Everything below was found in about forty minutes of ordinary use, not by fuzzing. The
register with severities, owners and fix status is `docs/TECH_DEBT.md`; this file is the raw
record, per the `AGENTS.md` rule that agents write here and never `DELTA.md`.

---

## What worked, first, because it is the load-bearing part

`fleet sow` earned its place. The content gates rejected my first SOW draft with **ten** specific
violations and one ambiguity finding, and every one of them was correct — the draft genuinely had
no measurable acceptance threshold and no explicit non-goals. Rewriting it to clear the gates
produced a materially better document than I would have written unprompted. The ambiguity check
in particular ("no stated success metric — work could ship and still miss the goal") is the kind
of finding a human reviewer skips.

`semgrep` (1529/1529) and `detectors` (111/111) both ran cleanly against a TypeScript tree.

---

## FD-1 · `fleet sow` panics on any non-ASCII character

```
thread '<unnamed>' panicked at crates/fleet-plan/src/intake/sow_checks.rs:25:17:
end byte index 8 is not a char boundary; it is inside '—' (bytes 6..9 of string)
```

`has_line_prefix_ci` does `l[..prefix.len()]`, a **byte** slice, against a `&str` that may hold
multi-byte UTF-8. My SOW had an em-dash in a heading. Reproduces with any of `— " " é 🙂` near
the start of a line.

Three things make this worse than a normal panic:

1. It is a **panic, not a typed exit**. `AGENTS.md` r7 defines 0/3/6/7/8; a panic is none of them,
   so a caller scripting `fleet sow` gets an unclassifiable failure.
2. It violates r8 — **"every refusal writes a receipt before exiting"**. A panic writes nothing.
3. An em-dash in prose is not an edge case. It is what anyone writing a real SOW in a real editor
   will produce. The gate that exists to improve written specs cannot read normally-typed English.

Worked around by forcing my SOW through `LC_ALL=C grep -n '[^ -~]'` and rewriting it ASCII-only.

---

## FD-2 · The `source_intent_hash` refusal points at a file that does not exist

**Corrected after investigation.** I originally wrote this up as *"the gate cannot pass."* That
was wrong, and how it was wrong is the actual defect.

```
REFUSED: source_intent_hash does not match intent.txt (expected <the value I passed>)
```

Two things, one line apart in `crates/fleet-plan/src/intake/sow.rs:30-38`:

1. The message interpolates the **caller's own `--intent-hash` flag** behind the word "expected".
   A reader assumes fleet computed that digest. It did not.
2. It names `intent.txt` — **a file nothing in this codebase ever opens.** The only occurrences of
   that string in the repo were this message and prose describing the *bash* original
   (`intake.sh:50`'s `hash_file` subprocess). `fleet-plan`'s `lib.rs` invariant is that the crate
   opens no file, no socket, no process; `BLUEPRINT.md:623` assigns the `shasum` to the caller.

The gate was always passable. The digest goes in a `source_intent_hash:` line **inside the
`--text` body**, exactly as `docs/USING-FLEET.md:140-142` says. My `intent.txt` experiment — write
the file, `shasum` it, pass the digest — could never have worked, and the error text is what sent
me there instead of to the one line I was missing.

**The generalisable part:** a string that names a file is a claim about behaviour, and nothing
type-checks it. This filename outlived the implementation it described, straight through a
port from bash to Rust, because an error message has no compiler. Error text naming a path
should be generated from the path the code actually opens.

Fixed: the refusal now names both sides and states that nothing is read from disk; when neither
side carries a hash the check is **skipped rather than failed**, so a caller with no intent
provenance still gets a content verdict. Still exit 7, never exit 3 — there is no tool or file
whose absence this check can observe, so an environment fault is not representable here.

## FD-3 · A REFUSED SOW is written to memory, and poisons the next attempt

Sequence, exactly as it happened:

1. `fleet sow` with draft A → REFUSED, structure violations.
2. Fixed the draft → ran again → REFUSED again, now with an **extra** finding:
   `ambiguity (Memory): This looks like a prior decision -- which one governs? --
   Similar to a remembered item: "<the full text of my own rejected draft A>"`

The memory gate matched my corrected draft against the rejected draft it had just stored. The
system's memory is being populated by submissions it explicitly refused. Every rejected attempt
raises the noise floor for the next one, and a user iterating toward a passing SOW — which is the
intended workflow — makes it monotonically worse.

Only accepted SOWs are decisions. A refusal is the absence of a decision.

---

## FD-4 · `FLEET_STREAM_DIR` silently writes nothing

`fleet run --help` documents it as mirroring the run's ledger receipts as NDJSON. I set it to an
absolute path under an existing parent and ran a full pipeline through to the verify stage.
**The directory was never created and not one byte was written.**

Correction on re-investigation: it was not *quite* silent. `maybe_flush` did surface
`note: fleet-stream: No such file or directory (os error 2)` — with no path and no variable name
in it, which is silence for practical purposes. `sinks/file.rs:29-33` opens with
`OpenOptions::create(true)`, which creates the file but never its parent directory.

Silence is the defect. Either create the path or fail naming it. Both, now.

---

## FD-5 · `--json` is strictly less informative than the human path

```json
{"task":"…","final_stage":"Verify","classification":null}
```

That is the entire machine-readable output of a run whose human summary carried six stage results,
eight gate verdicts with scores, and a refusal reason. `result` is `#[serde(skip)]`, so the
refusal never reached JSON at all.

**Correction on the `classification: null` half.** That was not a serialization bug — it is a
**resume artifact**. `stage_loop.rs` `continue`s past any stage the step log already marked done,
and my two invocations shared a `--task` string, so `Classify` never re-ran. My report conflated
two different causes in one line. A resumed stage now says `outcome: resumed` with an absent
`elapsed_ms`, rather than a fabricated `0`.

This inverts the intent of `--json`. Anything automating fleet — CI, the central server in the
SOW's phase 4, another agent — gets less than a human reading stderr.

And the fix immediately earned its keep: with the classification actually serialized, a normal
`fleet run` turns out to report `role: "invalid"`, refusal *"role has no eligible tier set"*, all
six routing stages at `checked: 0, total: 5`, and no model ever resolved — while the classify
stage reports **pass**. That is FD-9 below, and it was invisible for exactly as long as the field
was omitted.

---

## FD-6 · Five of eight verify gates cannot run on a non-Rust repo, and skip instead of failing

Real output against the Node repo:

```
SKIP gate unit tests -- unit tests unavailable
SKIP gate mutants -- mutants skipped: set FLEET_MUTANTS=1 to run it
PASS gate semgrep 1529/1529
FAIL gate trivy 0/10 -- NonZeroExit(1)
SKIP gate recur -- recur: no applicable input in this repo
PASS gate detectors 111/111
SKIP gate policy -- policy unavailable
FAIL gate corpus 25/27 -- NonZeroExit(1)
```

The unit-tests gate shells out to `cargo test --workspace`, hardcoded in the gate registry's
`static` array. A TypeScript repo has no such command, so the gate reports **SKIP** — that is,
`checked == 0`.

This is the objection that matters most, because it contradicts two of this repo's own hard rules
at once:

- `AGENTS.md` r6: *"A gate/check that examined zero inputs FAILS. Never passes."*
- `AGENTS.md` r10: *"Publish the denominator. A verdict carries `{checked,total}`; `checked==0`
  is a failure."*

And it contradicts the crate built specifically to enforce them. The SOW's §04 cites
`fleet-verify`'s `zero_exit_with_zero_total_is_fail_not_pass` as evidence that fleet is *"built to
catch the 'measured nothing' bug on purpose."* On this run, four gates measured nothing and the
pipeline treated all four as non-events. The invariant is enforced *inside* a gate's own result
and not *across* the gate set.

The practical consequence for the POSX estate: **fleet cannot currently serve as the pre-merge
gate for the 24 non-Rust repos it is intended for.** Not because the gates are wrong, but because
the commands they run are baked in at compile time. The registry needs a per-repo override so a
Node repo can declare `npm run test:unit` — an override that lets a repo say *what its unit-test
command is*, never one that lets a repo opt out of having one.

---

## FD-7 · `cargo` is reported missing when it is installed

`fleet doctor` prints `cargo: missing` on a machine with cargo 1.98.1 at `~/.cargo/bin/cargo`.
Fleet resolves it from `PATH` only, and `~/.cargo/bin` is not on the default non-login `PATH` on
macOS.

This compounds FD-6: the unit-tests gate degrades to SKIP for what is actually an **environment
fault (r7 exit 3)**, and reports it as a benign skip. Per `AGENTS.md` r7, *"an environment fault
must never be reported as an agent failure"* — the inverse also holds: it must not be laundered
into a non-failure. Falling back to `$CARGO_HOME/bin` then `~/.cargo/bin`, and naming the paths
searched when it still cannot be found, fixes both.

---

## Proposed `DELTA.md` rows — for the lead to write, not me

`DELTA.md` is lead-only per this file's own header, so these are handed over rather than appended.
Both are BLUEPRINT-SILENT in my reading; the verdict is the lead's call.

| topic | blueprint says | implementation found | suggested verdict |
|---|---|---|---|
| `source_intent_hash` never reads `intent.txt` | `BLUEPRINT.md:623` assigns the `shasum` to the caller; `USING-FLEET.md:140-142` documents the in-body `source_intent_hash:` line | The refusal message named `intent.txt`, a filename inherited from `intake.sh:50`'s `hash_file` and surviving only in that string. Blueprint and implementation already agreed; the *message* was the only thing that disagreed with both. | **BLUEPRINT-RIGHT, message wrong** — no blueprint change. Record that the skip-when-neither-side-carries-a-hash case is new, and that `USING-FLEET.md` §4 should gain the skip rule. |
| `checked==0` across a gate **set**, not within one gate | `05` §3.1 and `AGENTS.md` r6/r10 state the rule for a gate's own verdict | Four gates each reported SKIP with `checked==0` and the pipeline aggregated them as non-events. The rule is enforced one level below where it binds. | **BLUEPRINT-SILENT** — the rule needs a fleet-level statement: a *pipeline* whose gate set measured nothing is not a pass, and the aggregate verdict must publish how many gates actually examined input. |
| gate commands are compile-time constants | nothing in the blueprint says the gate set is portable across repos | `GATES` is a `static` array with `'static` lifetimes and fn pointers; the unit-tests gate is literally `cargo test --workspace`. Fleet's stated purpose includes driving whatever CLI is installed in *any* repo, but its own verifier only understands one language. | **BLUEPRINT-SILENT** — portability is claimed for the *work* plane (drives any coding CLI) and never stated for the *control* plane (the verify gates). The two must match or fleet is a Rust-only tool wearing a language-agnostic description. |

---

## One note on the SOW document itself

`agentic-sdlc-proposal.html` §04 reports *"351 passed · 0 failed"* for `cargo test --workspace` and
lists three confirmed breaks. That test count is real and I have no reason to doubt it. But every
defect above sits **outside** the test suite's reach, in the same class as the SOW's own Finding 0
(`fleet swarm` passing 351 tests while the real child process dies on argument parsing). Seven
more instances of *a proxy is not the property*, found by the first person to run the binary
against someone else's repo. The number worth carrying into §04 is not 351 — it is that
**first external use produced seven defects in forty minutes**, and that is the honest measure of
how much of fleet has been exercised outside its own tree.


---

## FD-8 · The gate *command* is now per-repo. The gate *parser* is not.

Found by verifying the FD-6 fix, which is the honest way to find out a fix is half a fix.

With `.fleet/gates.toml` pointing the unit-tests gate at the Node repo's real suite, the command
runs and the suite genuinely passes:

```
$ npm run --silent test:unit
Test Suites: 26 passed, 26 total
Tests:       671 passed, 671 total
```

Fleet's verdict on that same run:

```
FAIL gate unit tests -- Unparseable
checks 0 performed  (no gate examined any input -- treat this run as a failure, not a pass)
exit=6
```

`crates/fleet-verify/src/parsers.rs:73-78` — `unit_tests` looks for libtest's fixed
`"N passed; M failed;"`. Jest prints `"671 passed, 671 total"`: comma, not semicolon, and *total*
rather than *failed*. Every parser in that file is welded to one tool's output shape.

So FD-6 moved the command out to config and left the denominator behind. Those two have to travel
together, or a repo can be told to run its own tests and still never produce a verdict — which is
the whole point of the exercise for the 24 non-Rust repos.

**The behaviour is right even though the outcome is wrong.** It fails closed at exit 6 rather
than passing something it could not measure, and the new aggregate line says so in plain words.
r6 and r10 are working exactly as designed; they are now blocking the thing FD-6 set out to
enable. That is a good failure to have.

One caution for the fix: if the parser becomes a repo-supplied regex, a repo can manufacture its
own denominator, and r6 exists precisely to stop that. A named set of built-in parsers
(`libtest` | `jest` | `pytest` | `go-test`) keeps the denominator's definition on fleet's side of
the boundary. Whatever the shape, "no denominator" must not be expressible.

---

## FD-9 · `fleet run` never passes a role, so classification always refuses — while reporting pass

```json
"classification": {
  "role": "invalid", "selected_adapter": null, "resolved_model": null,
  "stages": [{"number": 1, "name": "explicit role", "checked": 0, "total": 5, "candidates": []}, …],
  "refusal": {"stage": 1, "stage_name": "explicit role",
              "reason": "role has no eligible tier set",
              "fix": "pass a role of lead, builder, verifier, designer, or meter"}
}
```

The stage's own payload is a refusal with `checked: 0` on all six routing stages, and the stage
reports **pass**. This is the same shape as FD-6 one level up: a component that measured nothing
is treated as a non-event.

`fleet run` has no `--role`. Either it should default to `builder`, or the classify stage's
refusal must fail the stage. Not my call, and not a one-line change, so it is logged rather than
fixed.

---

## Third proposed `DELTA.md` row

| topic | blueprint says | implementation found | suggested verdict |
|---|---|---|---|
| a required gate that SKIPs does not fail `fleet run` | `AGENTS.md` r6/r10; `05` §3.1 | `pipeline/verify_stage.rs:29-33` pushes only `Verdict::Fail` into `failed`, so a required gate that skips leaves the verify stage passing on that gate. `fleet gate` and `fleet oracle` both handle it correctly (`Report::exit_code()` → `env_faults() > 0` → exit 3) — **the pipeline is the odd one out.** Left alone deliberately: changing it would newly refuse Rust repos missing `conftest` or `uv`. Made machine-visible instead via `GateRecord.env_fault`. | **BLUEPRINT-SILENT** — the rule is stated for a gate and for the two single-gate commands, and never for the pipeline. Needs a decision, not just a patch: which is right, and what happens to a repo legitimately missing an optional tool. |
