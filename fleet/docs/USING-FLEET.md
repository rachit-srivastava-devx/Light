# Using fleet

Everything here was run on 2026-08-24 against `keel/target/debug/fleet`. Exit codes are real,
captured with `rc=$?` on its own line — `$?` after a pipe is the pipe's status, which is a trap
this project fell into more than once.

---

## Careful: two different programs are called `fleet` on this machine

`~/.local/bin/fleet` is a symlink into a **separate, older `fleet` repository** (the bash one, at
`Principal Engineering/fleet/`). Typing `fleet` in a fresh shell runs **that**, not this.

This repo's `install.sh` installs to the same path. It now **refuses** rather than overwriting
something it did not put there:

```
WOULD REFUSE: ~/.local/bin/fleet is a symlink to .../Principal Engineering/fleet/fleet
              -- not installed by this repo.
  Installing would destroy it. Move it aside, or install elsewhere:
    FLEET_BIN_DIR=~/.local/bin/fleet-rs ./install.sh
```

Until you pick one, use an explicit path or prepend this repo's build directory.

## Setup — two lines

```bash
cd keel && cargo build --release && cd ..
export FLEET_STATE="$PWD/var/fleet"      # fleet will not guess where to keep evidence
export PATH="$PWD/keel/target/release:$PATH"
```

`fleet doctor` checks the rest. If `FLEET_STATE` is unset, every command that needs it stops and
tells you the exact line to run.

---

## The exit codes — the whole contract in six numbers

| Code | Means |
|---|---|
| `0` | ok |
| `3` | environment fault — something about *your machine* is wrong |
| `6` | invariant violated — fleet's own rules broke |
| `7` | refusal — fleet understood you and declined |
| `8` | mismatch — recomputed evidence disagrees with what was stored |
| `9` | SOW ready, awaiting human review |

**`7` is not an error.** It is the product working. Most of what follows is fleet saying no.

---

## Walkthrough: one task, start to finish

### 1. Say what you want in English

```bash
fleet plan "add a --version flag to the cli"
```

```
intent: implement a change
agent: builder
skills: rust
routed lane: UNAVAILABLE — route emptied at stage 4 (availability/quota): all eligible lanes
  are cooling down or lack a sufficient known quota window
  fix before running: wait for cooldown/reset or configure a measured window of at least 1 tokens
commands: 3 planned (denominator: 3)
  1. fleet sow
  2. fleet sow accept
  3. fleet swarm dispatch
note: plan only — nothing was executed.
```

It names the intent, the agent, the skills, the lane, and the commands — **and executes nothing.**
Note it reports the lane as `UNAVAILABLE` with the stage that emptied the route rather than
quietly picking something else. There is no fallback lane by design.

### 2. When it does not understand you

```bash
fleet plan "make me a sandwich"      # exit 7
```

```
fleet: no committed intent matches: "make me a sandwich"
  closest intents (3 candidates):
    fix the parser bug
    diagnose fleet
    is the ledger ok
```

It refuses and hands you the nearest real intents. It does not guess.

### 3. Try to skip the plan

```bash
fleet run --task "add a --version flag" --repo /path/to/repo --agent stub    # exit 7
```

```
fleet: refusing execution: task has no accepted SOW.
SOW id to accept: 9e97d99b220eade...
Create it with: fleet sow --task <exact task>
Then accept it with: fleet sow accept --id 9e97d99b220eade...
```

**This is the pushback that matters.** No work happens without a plan a human accepted. The SOW id
is bound to the exact task text — change one character and it is a different task needing its own
acceptance.

### 4. Ask for a SOW, get interrogated

```bash
fleet sow --task "add a --version flag"      # exit 7
```

```
SOW refused: challenge register has no evidence citation
CLARIFYING QUESTIONS
1. challenge register has no evidence citation? [SOW line 1]

Answer by re-running with the sections filled in:
  fleet sow --task "add a --version flag
  leaves:
  - print the version and exit 0 | acceptance: --version prints a semver, exits 0
  challenges:
  - may collide with an existing flag | citation: keel/fleet/src/main.rs:40
  alternatives:
  - json-output: emit JSON instead | tradeoff: harder for humans to read
  - build-info: add commit and date | tradeoff: leaks build-host details
  estimates:
  - 30 minutes
  edge cases:
  - --version combined with another flag"

Each challenge needs a real citation (file:line or corpus id), and at least two
named alternatives with tradeoffs -- one option is not a choice.
```

It refuses a lazy plan, asks the clarifying question, **and writes the template for you.** The two
demands worth noticing: a challenge needs a citation that resolves, and one alternative is not a
choice.

### 5. Give it a real SOW — it still stops

Re-run with the filled-in task. Now:

```
SOW_READY_AWAITING_REVIEW id=fcea10dfc63d8f37...
Accept with: fleet sow accept --id fcea10dfc63d8f37...
  ---> exit 9
```

**Exit 9.** It will not accept its own plan.

### 6. You accept

```bash
fleet sow accept --id fcea10dfc63d8f37...
```

```
SOW_ACCEPTED id=fcea10dfc63d8f37... by=rachitsrivastava at=2026-08-24T17:34:49Z
```

Who accepted it and when are now on the record.

### 7. Now it runs

```bash
fleet run --task "$T" --repo /path/to/repo --agent stub
```

```
registered artifact=0094d2237b98a12d... oracle=o1 author=lead
registered artifact=0094d2237b98a12d... oracle=o2 author=verifier
artifact=0094d2237b98a12d... quadrant=ACCEPT fault=NONE credit=NONE o1=pass o2=pass
artifact=0094d2237b98a12d...
```

Two independent oracles, a 2×2 adjudication, a frozen diff and an attestation.

### 8. Ask what happened — from receipts, not memory

```bash
fleet status
```

```
FLEET STATUS — 2 of 2 tasks from receipts
┌ DONE — 1 of 2 tasks ──────────────────────────────────────────────┐
│add a --version flag   stub    yes    completed; attestation verified│
┌ FAILED — 1 of 2 tasks ────────────────────────────────────────────┐
│add a --version flag   —       —      refused: SOW_NOT_ACCEPTED      │
```

The refused attempt from step 3 is still there. **Fleet does not forget the times it said no** —
and the denominator is printed, so you can see nothing was dropped.

### 9. Check the evidence

```bash
fleet ledger verify        # verified checked=12 total=12   exit 0
fleet attest verify 0094d2237b98a12d...
```

### 10. Try to cheat it

Edit one row in the middle of `$FLEET_STATE/ledger/chain.jsonl`, then:

```bash
fleet ledger verify        # exit 8
```

Restore it and it returns to `verified checked=12 total=12`, exit 0. Tested in both directions —
a check that only ever passes proves nothing.

---

## Every command

### Doing work
| Command | What it does |
|---|---|
| `fleet` | interactive REPL (when on a terminal) |
| `fleet --print <cmd>` | run one command, no REPL |
| `fleet plan "<english>"` | name intent, agent, skills, lane, commands. Executes nothing |
| `fleet sow --task <T>` | build a SOW. Exit 9 awaiting review |
| `fleet sow accept --id <ID>` | record who accepted it and when |
| `fleet run --task <T> --repo <P> --agent <stub\|env-probe\|freelane>` | run, freeze the diff, attest |
| `fleet swarm dispatch --task <T> --repo <P> [--agent <A>]` | allocate role work, verify, score |
| `fleet rollback --artifact <ID>` | restore the pre-run tree, keep all evidence |

### Evidence
| Command | What it does |
|---|---|
| `fleet status [--json]` | task rollup derived from receipts |
| `fleet ledger verify` | verify the hash chain. Exit 8 on tamper |
| `fleet ledger count` / `dump` | row count / raw JSONL |
| `fleet ledger append --event <E> --body <JSON>` | append a receipt |
| `fleet attest verify <ID>` | recompute an attestation. Exit 8 on mismatch |
| `fleet oracle o1\|o2 --artifact <ID> --suite <P> --author <N>` | register an independent oracle |
| `fleet adjudicate --artifact <ID>` | run both oracles, apply the 2×2 |

### The machinery
| Command | What it does |
|---|---|
| `fleet roles` | roles, owned gates, write permission |
| `fleet route --role <ROLE>` | pick a model through six auditable stages |
| `fleet role-check` | check role separation |
| `fleet agents list` | registry + scorecards |
| `fleet skills [--check]` | resolve skills; fail on unresolved declarations |
| `fleet lifecycle states\|show\|advance` | the 15-state typed SDLC |
| `fleet meter show\|reserve\|plan` | measured token usage. Unknown is `null`, never `0` |
| `fleet ratchet show\|check` | the quality bar; refuses a regression naming both values |
| `fleet graph` | module reachability over the dispatch surface |
| `fleet doctor` | environment diagnostics |
| `fleet completions <bash\|zsh\|fish>` | shell completions |

---

## Two defects this walkthrough found

Both were invisible to the test suite because the suite checks exit codes, and the exit codes are
**right**. What is missing is the sentence explaining them to a human.

1. **`fleet run --task ""` is completely silent.** Exit 7, zero bytes on stdout *and* stderr. The
   refusal is correct and the user is told nothing at all.
2. **`fleet ledger verify` prints nothing on tamper.** Exit 8, no output. On success it prints
   `verified checked=12 total=12`; on the failure path — the one that matters — silence.

Same species as `D58`/`D59`/`D60`: the code is right, the layer between it and the person using it
is not. Recorded in `docs/delta.d/opus-walkthrough.md`.

---

## Running the autonomous loop

```bash
bash bin/loop-status.sh        # what is running, what is next
```

Builders (`bin/codex-fanout.sh`) and an adversarial reviewer (`bin/codex-review.sh`) are scheduled
every 30 minutes via launchd. Stop them with:

```bash
launchctl unload ~/Library/LaunchAgents/ai.fleet.codex-loop.plist
launchctl unload ~/Library/LaunchAgents/ai.fleet.codex-review.plist
```
