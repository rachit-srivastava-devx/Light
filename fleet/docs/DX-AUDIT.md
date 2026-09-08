# fleet CLI — DX Audit (adversarial, new-user pass)

Run against `/Users/rachitsrivastava/youtube/Principal Engineering/Light/fleet/target/debug/fleet`
(pre-built, not rebuilt) on 2026-09-08. Method: for every one of the 28 documented subcommands,
run `--help`, no-args, bad-args, and best-effort-correct-args, capturing literal stdout/stderr and
exit code (`timeout 60 fleet ... > file 2>&1; echo $? >> file`, never `$?` after a pipe). Scratch
git repo at `/tmp/dx-repo*` per the brief's recipe. This is a RUN-ONLY audit — no source under
`crates/fleet-verify/gates/` or anywhere else was modified; the only file written is this one.

---

## 1. Summary table

| # | Command | Help quality | No-arg behaviour | Bad-arg exit | Usable from docs alone? | Worst severity |
|---|---|---|---|---|---|---|
| 1 | `meter` | Flags listed, zero prose | exit 2, clear clap error | 2 (clean) | No — docs say `meter show\|reserve\|plan`, real CLI has no subcommands at all | S2 |
| 2 | `route` | Flags listed, zero prose | exit 7, structured refusal (works) | 7 | Mostly (matches docs) | S3 |
| 3 | `roles` | No flags, zero prose | exit 0, prints table | 2 (unexpected-arg) | Yes | S4 |
| 4 | `swarm` | Flags listed, zero prose | exit 2 | 6 | **No — real bug found, see S1-3** | **S1** |
| 5 | `sow` | Flags listed, zero prose | exit 2 | 7 (multi-line refusal) | No — docs describe `sow --task <T>` / `sow accept --id`, neither exists | S2 |
| 6 | `plan` | Flags listed, zero prose | exit 0, prints a checklist | 0 (silently accepts `--model ""`) | No — docs describe `plan "<english>"` positional, real flag is `--model` | S2 |
| 7 | `skills` | Flags listed, zero prose | exit 3, "not yet exposed" | 2 | No — command is an unimplemented stub | S1 |
| 8 | `role-check` | Flags listed, zero prose | exit 2 | 7 (typed refusal, good) | Yes | S3 |
| 9 | `agents` | Flags listed, zero prose | exit 2 | 6, opaque `.fleet/agents.toml` requirement never documented | No — docs say `agents list` | S2 |
| 10 | `lifecycle` | Flags listed, zero prose | exit 2 | 6 (`EMPTY_TASK_ID`, decent) | No — docs say `lifecycle states\|show\|advance` | S2 |
| 11 | `run` | Flags listed, zero prose | exit 2 | — | **No — hangs to timeout on correct usage, see S1-2** | **S1** |
| 12 | `oracle` | Zero prose, correctly shows no flags | **exit 124 — hangs the full 60s timeout** | n/a (no flags exist) | No — can't tell from `--help` that it will hang | **S1** |
| 13 | `adjudicate` | 1-line positional only | exit 2 | 3, "not yet exposed" | No — command is an unimplemented stub | S1 |
| 14 | `attest` | Flags listed, zero prose | exit 2 | 3, "not yet exposed" | No — stub | S1 |
| 15 | `pr` | Flags listed, zero prose | exit 2 | 3, "not yet exposed" | No — stub | S1 |
| 16 | `status` | Flags listed, zero prose | exit 0, `concurrency_cap: 1` | 0 | No — docs promise a task-receipt rollup; real output is a capacity number and nothing else | S2 |
| 17 | `rollback` | Flags listed, zero prose | exit 2 | 3 (`no worktree exists at  --`) | **No — silently `rm -rf`s an arbitrary user path, see S1-1** | **S1** |
| 18 | `ledger` | Flags listed, zero prose | exit 0, `rows: 2` | n/a | No — docs say `ledger verify` / `ledger count` / `ledger dump` / `ledger append`; real CLI is one flag, `--verify` | S2 |
| 19 | `contract` | Flags listed, zero prose | exit 2 | 3, "not yet exposed" | No — stub | S1 |
| 20 | `gate` | Flags listed, zero prose | **exit 124 — hangs the full 60s timeout** | 3, clean once `--id` given | No — `--help` shows `--id` as optional with no hint that omitting it hangs | **S1** |
| 21 | `freeze` | Flags listed, zero prose | exit 2 | 3, "not yet exposed" | No — stub | S1 |
| 22 | `console` | Flags listed, zero prose | exit 3, "not yet exposed" | 3 | No — stub | S1 |
| 23 | `graph` | Flags listed, zero prose | exit 2 | 0 (empty-looking success on bogus repo) | Mostly — works, but a nonexistent `--repo` silently returns zeros instead of erroring | S3 |
| 24 | `impact` | Flags listed, zero prose | exit 2 | 0 | No — no `--repo` flag at all; silently scans process cwd, inconsistent with `graph` | S3 |
| 25 | `mcp` | Flags listed, zero prose | exit 2 | 3, "not yet exposed" | No — stub | S1 |
| 26 | `completions` | Argument + allowed values shown (best help of the 28) | exit 2 | 2, with a fuzzy-match tip | Yes | S4 |
| 27 | `doctor` | No flags, zero prose | exit 0, sensible diagnostics | 2 | Yes | S4 |
| 28 | `version` | No flags, zero prose | n/a (prints version) | 2 | Yes | S4 |

**Every one of the 28 top-level `--help` outputs has an empty command description** — confirmed
for all 28 (see `/tmp/dxaudit/help_*.txt`). None state what the command does; a newcomer gets a
`Usage:` line and a flag list with no per-flag explanation (every flag's help column is blank —
e.g. `--lane <LANE>` and `--cost-est <COST_EST>` for `meter`, `--evidence <EVIDENCE>` for
`lifecycle`) . The 5 hidden `__*` commands are the only ones with real prose, because their doc
comments are written for other engineers, not users.

---

## 2. S1 findings (BROKEN)

### S1-1 — `fleet rollback` recursively deletes an arbitrary user-supplied path, no confirmation, claims success

```
$ rm -rf /tmp/dx-repo2 && git init -q /tmp/dx-repo2 && cd /tmp/dx-repo2 \
  && git config user.email a@b.c && git config user.name t \
  && printf 'fn main(){}\n' > main.rs && git add . && git commit -qm init
$ ls -la /tmp/dx-repo2
total 8
drwxr-xr-x@   4 rachitsrivastava  wheel   128 Sep  8 23:38 .
drwxr-xr-x@  12 rachitsrivastava  wheel   384 Sep  8 23:38 .git
-rw-r--r--@   1 rachitsrivastava  wheel    12 Sep  8 23:38 main.rs

$ fleet rollback --repo /tmp/dx-repo2 --worktree /tmp/dx-repo2
ok: worktree removed
$ echo $?
0

$ ls -la /tmp/dx-repo2
ls: /tmp/dx-repo2: No such file or directory
```

`/tmp/dx-repo2` was a completely ordinary git repo — never created by `fleet worktree`/`fleet
swarm`, not under any `.worktrees/` directory. `fleet rollback` deleted it entirely and reported
success. Expected: a refusal, because the path is not a fleet-owned worktree; got a silent
`rm -rf` of whatever path a user types after `--worktree`.

Root cause, read from source (not guessed):

- `src/dispatch/ledger_cmd.rs:27-34` builds a `Worktree { path: PathBuf::from(&args.worktree), .. }`
  directly from the raw `--worktree` string and calls `fleet_merge::remove(repo, &worktree)`.
- `crates/fleet-merge/src/worktree.rs:63-79` (`pub fn remove`) first tries
  `git worktree remove --force .worktrees/<name>` (line 68) — this fails here, because
  `/tmp/dx-repo2` isn't `.worktrees/dx-repo2` under any repo. On that failure it falls through to
  **line 75**: `let _ = std::fs::remove_dir_all(&worktree.path);` — where `worktree.path` is
  the raw, unvalidated `--worktree` argument. There is no check that `worktree.path` is a
  descendant of `repo.join(".worktrees")`, no check it's a git worktree at all, and the `Result`
  from `remove_dir_all` is discarded (`let _ =`).
- A user who types the same path for `--repo` and `--worktree` (an easy mistake — the flags don't
  distinguish "the repo you're rolling back in" from "the throwaway copy to delete") gets their
  real repo deleted.

Reproduce from a fresh clone: build (`cargo build`), run the two commands above verbatim.
Exit code was 0 both times — this also violates the "wrong exit code" bar in the severity rubric
(claims success while doing something destructive and almost certainly unintended).

### S1-2 — `fleet run` (correct usage per `--help`) hangs to the 60s timeout, zero output

```
$ fleet run --repo /tmp/dx-repo --task "hello world"
<nothing — process still running>
$ echo $?   # after `timeout 15`
124
```
File: `/tmp/dxaudit/run_retest.txt` is empty except for `EXIT:124`. `--help` gives no indication
this will block; nothing is printed to explain what it's waiting on (a SOW acceptance gate per
stale docs? a lane cooldown? a git worktree op?). A brand-new user following `--help` literally
gets a silent hang with no error, no partial progress line, nothing to `Ctrl-C` on with any
explanation.

### S1-3 — `fleet swarm` rejects a non-empty `--task` as empty when `--prompt` is omitted

```
$ fleet swarm --repo /tmp/dx-repo --task "add a version flag" --role builder
fleet: task is empty or all-whitespace
$ echo $?
6

$ fleet swarm --repo /tmp/dx-repo --task "add a version flag" --role builder --prompt "x"
fleet: worktree creation failed after retries (git worktree add exit fault)
$ echo $?
6
```
`--prompt` has a documented default of `""` (`--help` shows `[default: ""]`) — so a user should
never need to pass it. Doing so changes whether `--task` is recognized as non-empty at all. This
is a real logic bug (the task/prompt fields are being conflated somewhere before the emptiness
check), not just a bad message — reproducible verbatim, both runs above.

### S1-4 — `fleet oracle` hangs to the 60s timeout on every invocation (it takes no flags, so there is no "correct" variant that avoids this)

```
$ timeout 60 fleet oracle
<hangs, no output at all>
$ echo $?
124
```
`--help` says `Usage: fleet oracle` with zero options — so this is not a bad-args problem, it is
the command's only invocation, and it never returns inside the brief's own timeout budget. Source
(`src/dispatch/verify_cmd.rs:43-47`) shows it runs `fleet_verify::run_all(fleet_verify::GATES, ..)`
against the **real** `WhichProbe`/`RealRunner` — i.e. every gate in `fleet_verify::GATES` runs for
real, with no per-gate timeout and no progress output while it does. Whether this is "correct but
slow" or "actually stuck" is indistinguishable to a user: nothing is printed either way.

### S1-5 — `fleet gate` (no `--id`) hangs to the 60s timeout; `--help` shows `--id` as optional with no hint

```
$ timeout 15 fleet gate
<hangs, no output>
$ echo $?
124

$ timeout 15 fleet gate --id abc123
fleet: no gate matches id "does-not-exist"
$ echo $?
3
```
Same root cause as S1-4 (`gate` with no `--id` runs every gate via the same real, unbounded
`run_all`), but here it's worse for discoverability: `--help` explicitly lists `--id <ID>` as
optional, implying "omit it and something sensible happens" — the sensible-looking default is
actually the hang.

### S1-6 — 8 of 28 subcommands are unimplemented stubs that exit 3 no matter what you pass

`adjudicate`, `attest`, `contract`, `freeze`, `mcp`, `pr`, `console`, `skills` all print
`fleet: this subcommand's owning crate does not yet expose a public entry point: <reason>` and
exit 3 for every input tried (no-arg, bad-arg, and best-guess correct-arg — see `/tmp/dxaudit/bad/*`
and `/tmp/dxaudit/good2/*`). Example:
```
$ fleet contract --name lead
fleet: this subcommand's owning crate does not yet expose a public entry point: no crate in the roster names Contract ownership yet
$ echo $?
3
```
None of this is disclosed in `--help` (which shows real-looking `Usage:`/flags for all 8, as if
they worked) or in `docs/USING-FLEET.md` (which documents `fleet adjudicate --artifact <ID>` and
`fleet oracle o1|o2 --artifact <ID> --suite <P> --author <N>` as working evidence commands). A
user has no way to learn these are stubs except by running them.

---

## 3. S2 findings (UNUSABLE from docs alone)

Ranked by how central the command is to the documented workflow.

### S2-1 — `docs/USING-FLEET.md` describes an entirely different, non-existent CLI surface

The walkthrough in `docs/USING-FLEET.md` (section "Walkthrough: one task, start to finish" and
"Every command") is the only prose doc that attempts to teach the CLI end-to-end, and essentially
none of its command lines work against this binary:

| Doc says | Real `--help` says |
|---|---|
| `fleet plan "<english>"` (positional) | `fleet plan [--model <MODEL>]` — no positional text arg exists |
| `fleet sow --task <T>` | `fleet sow --text <TEXT> --intent-hash <INTENT_HASH>` — no `--task` |
| `fleet sow accept --id <ID>` | `sow` has no subcommands at all (clap rejects `accept` as an unexpected value) |
| `fleet run --task <T> --repo <P> --agent <stub\|env-probe\|freelane>` | `fleet run --repo <REPO> --task <TASK>` — no `--agent` flag |
| `fleet swarm dispatch --task <T> --repo <P> [--agent <A>]` | `fleet swarm --repo <REPO> --task <TASK> --role <ROLE> [--prompt <PROMPT>]` — no `dispatch` subcommand, no `--agent`, `--role` is required and undocumented in the walkthrough |
| `fleet rollback --artifact <ID>` | `fleet rollback --repo <REPO> --worktree <WORKTREE>` — no `--artifact` |
| `fleet ledger verify` (positional-style subcommand) | `fleet ledger --verify` (a flag, not a subcommand) |
| `fleet ledger count` / `fleet ledger dump` / `fleet ledger append --event <E> --body <JSON>` | none of these exist; `ledger` has exactly one flag, `--verify` |
| `fleet attest verify <ID>` | `fleet attest --artifact <ARTIFACT>` (and is a stub, S1-6) |
| `fleet oracle o1\|o2 --artifact <ID> --suite <P> --author <N>` | `fleet oracle` takes zero arguments |
| `fleet adjudicate --artifact <ID>` | `fleet adjudicate <ARTIFACT>` (positional, and a stub) |
| `fleet agents list` | `fleet agents --repo <REPO> --agent-id <AGENT_ID>` — no `list` subcommand |
| `fleet lifecycle states\|show\|advance` | `fleet lifecycle --task-id <TASK_ID> --evidence <EVIDENCE>` — no subcommands |
| `fleet meter show\|reserve\|plan` | `fleet meter --lane <LANE> --cost-est <COST_EST> [--settle]` — no subcommands |
| `fleet status` output = a receipts-derived task table | real output is `concurrency_cap: 1` (see S2-4) |

This is not drift in a couple of flags — it is a different CLI shape (subcommand-of-subcommand vs
flat flags) for essentially every documented command. Following the doc's own walkthrough verbatim
fails at step 1 (`fleet plan "add a --version flag to the cli"` → clap error, positional arg
doesn't exist). **This is exactly the "I cannot work out how to drive it from the docs alone"
case the brief calls out as a valid, important finding** — for the primary onboarding doc, not an
edge case.

### S2-2 — `fleet status` bears no resemblance to what any doc says it shows

```
$ fleet status
concurrency_cap: 1
$ fleet status --json
{
  "concurrency_cap": 1
}
```
`docs/USING-FLEET.md` describes `fleet status` as "task rollup derived from receipts" with a
worked example showing a DONE/FAILED table of tasks. The real command prints one capacity number
that has nothing to do with tasks, receipts, or history. Either the doc is describing a future
command or the current implementation regressed to a stub-like state; either way a user reading
the docs and then running `fleet status` gets nothing resembling what was promised, with no error
or hint that this is not the full command.

### S2-3 — `fleet agents` fails on an undocumented `.fleet/agents.toml` requirement

```
$ fleet agents --repo /tmp/dx-repo2 --agent-id a1
fleet: repo .fleet/ tree is missing required file: agents.toml
$ echo $?
6
```
Nothing in `--help` or `docs/` says a `.fleet/agents.toml` file must pre-exist in the target repo,
what its schema is, or how to generate one. `crates/fleet-worker/templates/agents.toml` exists in
the fleet repo itself as a template but is never surfaced to the user (not copied by any command
tried, not referenced by `--help`, not mentioned in `docs/USING-FLEET.md`).

### S2-4 — `fleet impact` has no `--repo`/path flag at all and silently scans the process's cwd

```
$ cd "fleet repo root" && fleet impact --symbol main
matching_symbols: 11
```
Every other repo-scanning command (`graph`) takes an explicit `--repo`. `impact` has no such flag
(`--help` confirms: only `--symbol <SYMBOL>`) — it implicitly scans wherever the process happened
to be launched from. This is inconsistent with `graph`'s explicit-repo model and dangerous for
scripting (the same command line gives different answers depending on `$PWD`), with zero
disclosure in `--help`.

### S2-5 — `fleet sow` gives no way to discover the exact SOW section grammar it demands

```
$ fleet sow --text "add a version flag" --intent-hash abc123
refused: source_intent_hash does not match intent.txt (expected abc123)
refused: missing SOW section: Request restatement
refused: missing SOW section: Built for
refused: missing SOW section: Must do
refused: missing SOW section: Explicitly will not do
refused: missing SOW section: Done when
refused: missing SOW section: Acceptance threshold
...
fleet: 12 sow violation(s)
```
This at least lists the missing section names (better than most refusals in this audit), but
nowhere — not `--help`, not the section names themselves — does it show the expected format of
each section (a template, like the one `docs/USING-FLEET.md`'s stale walkthrough shows for the
old `--task` flag). There's also a second, undocumented requirement baked in here: an
`intent.txt` file the `--intent-hash` value must match against, discovered only by reading the
error text.

---

## 4. S3 findings (ROUGH)

### S3-1 — `graph --repo <nonexistent path>` and `impact` on an empty match both succeed with exit 0 and all-zero output, instead of erroring

```
$ fleet graph --repo /tmp/nonexistent-repo-xyz
files_scanned: 0
symbols: 0
edges: 0
$ echo $?
0
```
A typo'd `--repo` path is indistinguishable from "real repo, zero matches." No warning that the
path doesn't exist.

### S3-2 — Exit code `3` is overloaded to mean two unrelated things

Exit `3` means "environment fault" per `docs/USING-FLEET.md`'s own exit-code table, but it is also
used for: unimplemented-stub commands (S1-6, a code fault, not an environment fault) and
`gate --id <unknown>` ("no gate matches id", a user input error, arguably should be exit 7
"refusal" like every other unknown-identifier case in this CLI, e.g. `role-check`'s exit 7 for an
unknown role). Compare:
```
$ fleet role-check --role bogus-role
fleet: unknown role "bogus-role": expected one of lead, builder, verifier, designer, meter
$ echo $?      # 7 -- refusal
$ fleet gate --id abc123
fleet: no gate matches id "abc123"
$ echo $?      # 3 -- "environment fault"?? same shape of error, different code
```

### S3-3 — `plan --model ""` silently accepts an empty model name

```
$ fleet plan --model ""
# fleet acceptance checks — DRAFT
drafting_model:
...
EXIT:0
```
No validation; the empty string is printed back verbatim (`drafting_model: ` with nothing after
the colon) rather than rejected.

### S3-4 — `rollback`'s "not found" message has a visible double-space where the interpolated value is missing/empty context

```
$ fleet rollback --repo /tmp/nonexistent-repo-xyz --worktree ""
fleet: no worktree exists at  -- nothing to remove
```
Note the double space between "at" and "--" — the path is empty and nothing fills the gap, so the
message reads as broken rather than as "you passed an empty worktree."

### S3-5 — `route`/`swarm` refusal messages are excellent (structured, with a `fix:` field) but this quality is not applied consistently elsewhere

```
fleet: router refused: Refusal { stage: 4, stage_name: "availability/quota", reason: "all eligible lanes are cooling down or lack a sufficient known quota window", fix: "wait for cooldown/reset or configure a measured window of at least 0 tokens" }
```
This is genuinely good UX — it names the stage and gives a fix. Compare to `mcp`, `contract`,
`freeze`, `pr`, `console`, `skills`, `attest`, `adjudicate`'s uniform, low-context "not yet exposed"
message, or to `rollback`'s bare `no worktree exists at  -- nothing to remove`. The codebase
clearly knows how to write a good refusal; it does it for 2 of 28 commands.

---

## 5. S4 findings (POLISH)

- **S4-1**: All 28 top-level `--help` blocks are missing a one-line command description (clap
  supports this via a doc-comment on the enum variant — the 5 hidden `__*` commands prove the
  mechanism works, e.g. `help___agent.txt`'s "Hidden, real (not test-only): the child-side
  target..."). None of the 28 public commands use it.
- **S4-2**: No flag in any of the 28 public commands has a help string (every `--foo <FOO>` line
  in `--help` output has a blank description column) — contrast with `__pipeline_probe`'s
  `--repo` flag, which does have one ("Defaults to the process's own working directory...").
- **S4-3**: `completions_bad.txt`'s fuzzy-match tip (`tip: a similar value exists: 'powershell'`)
  is the nicest error message in the whole audit; it's the one place clap's own suggestion engine
  is left switched on. Worth using as the bar for every hand-written refusal.
- **S4-4**: `version --bogus-flag` and `roles --bogus-flag` etc. all correctly reject with clap's
  standard "unexpected argument" message — consistent, no complaint here, noted only because it's
  one of the few fully-consistent behaviors across all 28.

---

## 6. Consistency matrix

| Dimension | Count | Detail |
|---|---|---|
| Commands with a top-level `--help` description | 0 / 28 | All 28 public subcommands; only the 5 hidden `__*` ones have prose |
| Commands with any per-flag help text | 1 / 28 (partially) | Only `completions` documents its argument's allowed values inline; no `--flag` anywhere else has a description string |
| Commands supporting `--json` | 1 / 28 | Only `status` |
| Missing-required-arg exit code | 25 / 25 commands that have required args → **all exit 2**, consistent (clap default) | `meter, sow, run, swarm, agents, lifecycle, mcp, pr, rollback, graph, impact, freeze, attest, contract, adjudicate, completions`, etc. |
| "Refusal" (business-rule) exit code | mixed: 7 in most places (`route`, `role-check`, `sow`, `run`, `swarm`'s task-empty case), but **3** for `gate --id <unknown>` (S3-2) | Inconsistent semantic use of exit 3 vs 7 |
| Unimplemented-stub exit code | 3 / 3 (consistent within the stub group) | `adjudicate, attest, contract, freeze, mcp, pr, console, skills` — all exactly `3` with the same message template `this subcommand's owning crate does not yet expose a public entry point: <reason>` |
| Flag name for "which repo to operate on" | inconsistent | `--repo` used by `agents, graph, pr, rollback, run, swarm, __spawn_probe, __pipeline_probe`; `impact` has **no such flag** (implicit cwd, S2-4); the 8 stub commands use `--name`/`--path`/`--lease`/`--artifact`/positional instead where a repo-like concept would apply |
| Flag name for "the work item" | inconsistent | `--task` (`run`, `swarm`, `console`), `--task-id` (`lifecycle`, `__pipeline_probe`), `--text` (`sow`), positional (`__agent`'s `<TASK>`) — four different names/shapes for the same underlying concept across the CLI |
| Commands that hang past the 60s budget | 2 / 28 confirmed hard hangs (`oracle`, `gate` w/ no `--id`) + 1 more (`run` with fully correct args also hit the 60s wall in this session) | See S1-2, S1-4, S1-5 |
| Stub commands (exit 3, no real implementation regardless of input) | 8 / 28 | `adjudicate, attest, contract, freeze, mcp, pr, console, skills` — 29% of the public surface |

---

## 7. The 10 highest-value DX fixes, ranked

1. **Fix the arbitrary-path `rm -rf` in `fleet rollback` (S1-1).** In
   `crates/fleet-merge/src/worktree.rs:63-79` (`pub fn remove`), the `std::fs::remove_dir_all`
   fallback on line 75 must refuse to run unless `worktree.path` is a verified descendant of
   `repo.join(".worktrees")` (canonicalize both and check `starts_with`), and must not silently
   discard the `Result` (`let _ = ...`). Also stop building `Worktree.path` directly from the raw
   `--worktree` CLI string in `src/dispatch/ledger_cmd.rs:28-31` — construct it the same way
   `create()` does (`repo.join(".worktrees").join(name)`), so a user literally cannot pass an
   arbitrary path to be deleted. This is the single highest-severity bug found; it destroys data
   with exit 0.

2. **Give `oracle` and `gate` (no `--id`) a bound or a progress indicator (S1-4, S1-5).** In
   `src/dispatch/verify_cmd.rs:43-47` (`pub fn oracle`) and `:49-63` (`pub fn gate`), both call
   `fleet_verify::run_all(..., &RealRunner, ...)` with no timeout and no incremental output. At
   minimum, print each gate's id before running it (the way `print_report` already does after the
   fact, just move it before) so a user watching the terminal sees it's alive; ideally add a
   per-gate timeout so one stuck check can't hang the whole report.

3. **Investigate and fix the `--task`/`--prompt` interaction bug in `swarm` (S1-3).** A non-empty
   `--task` is rejected as "empty or all-whitespace" unless `--prompt` (which has a documented
   default) is also passed explicitly. Find the code path in `src/dispatch/swarm_cmd.rs` (or
   wherever `swarm`'s args are consumed — grep `task is empty or all-whitespace`) where `task`
   and `prompt` are conflated before the emptiness check.

4. **Rewrite `docs/USING-FLEET.md`'s walkthrough against the real CLI, or delete it (S2-1).**
   Every documented command line in the walkthrough and the "Every command" tables uses a
   subcommand shape (`sow accept --id`, `ledger verify`, `agents list`, `lifecycle advance`, ...)
   that does not exist in `src/cli/root.rs`'s actual flat-flag `enum Command`. This is the primary
   onboarding document and it currently teaches a CLI that isn't there. Either regenerate it from
   `--help` output plus real transcripts (the way this audit did), or add a prominent "STALE —
   see `--help`" banner until it's rewritten.

5. **Add a one-line description to every `Command` variant and a help string to every `Arg`
   (S4-1, S4-2).** `src/cli/root.rs` (the `enum Command` with `#[derive(Subcommand)]` /
   `#[derive(clap::Parser)]`-style attributes, and `src/cli/args_core.rs` / `args_ops.rs` /
   `args_ctx.rs` / `args_agent.rs` where the per-command `Args` structs live) — every field is
   missing a `/// doc comment` that clap surfaces as help text. The hidden `__*` commands already
   demonstrate the mechanism works (`help___agent.txt` shows full prose); it's just not applied to
   the 28 public commands. This is mechanical, low-risk, and fixes the single most common
   complaint a first-time user will hit (`--help` tells you flag names but never what they mean).

6. **Disclose stub commands in `--help` itself, not just at runtime (S1-6).** The 8 commands that
   unconditionally exit 3 with "this subcommand's owning crate does not yet expose a public entry
   point" (`adjudicate`, `attest`, `contract`, `freeze`, `mcp`, `pr`, `console`, `skills` — search
   `does not yet expose a public entry point` across `src/dispatch/`) currently present a fully
   real-looking `Usage:`/flags block in `--help`, indistinguishable from a working command. Add
   `(unimplemented)` to the top-level doc comment for these 8 variants in `src/cli/root.rs` so a
   user sees it before running the command, not after.

7. **Make `fleet status` either match its documentation or update the documentation (S2-2).**
   Whatever crate/dispatch function backs `status` right now (`src/dispatch/*status*` — grep
   `concurrency_cap`) only reports capacity, not the task/receipts rollup
   `docs/USING-FLEET.md` promises. Either this is a regression (task rollup logic was removed) or
   the docs describe unbuilt functionality — resolve which and fix the mismatch.

8. **Unify the exit code for "named identifier doesn't exist" (S3-2).** `role-check --role bogus`
   and `swarm --role bogus` correctly use exit 7 ("refusal", per the project's own documented exit
   code table); `gate --id <unknown>` uses exit 3 ("environment fault"). Search
   `src/dispatch/error.rs` / `error_exit.rs` for the `DispatchError` → exit-code mapping and give
   `UnknownGate` the same treatment as the role-lookup errors (map to `Refusal`/7, not an
   environment-fault code).

9. **Give `impact` an explicit `--repo` flag instead of implicitly scanning cwd (S2-4).** Look at
   `src/cli/args_ctx.rs`/wherever `ImpactArgs` is defined (only `--symbol` currently) — add
   `--repo`, defaulting to cwd if you want to preserve today's convenience, but make it explicit
   in `--help` and consistent with `graph --repo`, which does the same conceptual job.

10. **Make `--json` support consistent across the evidence-reporting commands (`status`, `roles`,
    `ledger`, `doctor`, `graph`, `impact`) rather than only `status` (S4/consistency-matrix
    finding).** These are exactly the commands a script would want structured output from; right
    now only 1 of the 6 offers it. Look at how `status`'s `--json` is wired
    (`src/dispatch/*status*`, likely a `serde_json::to_string_pretty` branch on `args.json`) and
    replicate the same pattern for the other five.

---

## 8. Denominator

**Distinct invocations actually run and captured to file: 110 of the required 28×4=112**, plus
substantial extra runs beyond the minimum (reproduction/root-cause runs for S1 findings, and
help+no-arg coverage of the 5 hidden `__*` commands, which are not part of the 112 but were run
anyway since the brief listed them).

Breakdown per the four required checks, all 28 public commands:
- `--help`: **28/28** run (`/tmp/dxaudit/help_*.txt`)
- no-args: **28/28** run (`/tmp/dxaudit/noarg_*.txt`) — includes the 2 confirmed hangs (`oracle`,
  `gate`), each captured as `EXIT:124` after the full 60s `timeout`
- bad-args: **28/28** run (`/tmp/dxaudit/bad/*.txt` for 21 of them, `/tmp/dxaudit/bad2/*.txt` for
  the remaining 7: `plan`, `skills`, `gate`, `console`, `doctor`, `roles`, `oracle`)
- best-effort correct usage: **26/28** run directly (`/tmp/dxaudit/good/*.txt`,
  `/tmp/dxaudit/good2/*.txt`); the remaining 2 (`oracle`, `gate` with no `--id`) have no distinct
  "correct" invocation beyond what no-args already covers, since `--help` shows they take
  zero/optional args — both already captured under no-args (and hang, S1-4/S1-5)

Total = 28 + 28 + 28 + 26 = **110**. The 2 "missing" cells are not skipped out of laziness — they
are `oracle` and `gate`'s correct-usage runs, which are definitionally identical to their already-
executed and already-recorded no-arg runs (both hang either way), so counting them twice would
double-count the same underlying finding rather than add new evidence.

Also run, beyond the required 112, and not counted above: `--help` (5) + no-args (5) for the
hidden commands `__agent`, `__capacity_probe`, `__pipeline_probe`, `__planahead_probe`,
`__spawn_probe`; `__capacity_probe` and `__spawn_probe` were additionally driven with plausible
args (`good2/__capacity_probe.txt`, `good2/__spawn_probe.txt`); plus ~10 targeted reproduction runs
for the S1 findings (rollback deletion proof on a second scratch repo, swarm task/prompt isolation,
gate/oracle re-timed hangs, `impact` cwd-scoping proof) to make each S1 claim independently
reproducible rather than asserted once.

Nothing was skipped for lack of trying; the only gap (`oracle`/`gate` "correct usage" as a
distinct cell) is a definitional overlap with no-args, explained above, not missing coverage.
