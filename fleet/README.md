# fleet

Fleet is a local command-line tool that runs coding-agent work — Claude, Codex, whatever CLI
you've got installed — on your own Mac, checks the result the way a strict human reviewer would,
and only keeps what actually passes. It keeps its state in plain files under `$FLEET_STATE`
(default `~/.local/state/fleet`) and signs in with the CLI logins you already have. There is no
server, no daemon, no database to run, no API key to buy.

## The problem fleet exists to fix

Ask most coding agents to fix a bug and they will tell you it's fixed. Most harnesses stop there —
they run the agent, read its final message, and call that the result. Fleet doesn't work that way.
The agent's job is to *propose* a change. Deciding whether that change is real is a separate,
boring, deterministic program's job — not the model's, and not the harness's either. It re-runs
your actual tests, reads your actual diff, and never takes "I ran it and it passed" as evidence.

That split — the model proposes, a small non-model program decides — is the one idea everything
else here is built around.

## What makes fleet different

- **The model never grades its own work.** A separate, deterministic checker — not an LLM — runs
  your real test suite, scans the real diff, and is the only thing that can call a change "done."
  A model reviewer can flag risk or say what it thinks the code does; it cannot override a failing
  check.
- **Nothing passes by saying nothing.** Every check has to say how much it actually looked at
  (`3 of 3`, never a bare "passed"). A check that examined zero inputs is a recorded failure, not a
  free pass — that exact mistake ("a check ran, found nothing to check, and every downstream run
  treated silence as success") is what pushed this rule in.
- **Failures come with a reason, not a shrug.** Every run ends in one of five exit codes: it
  worked, your machine's environment is broken, an invariant was violated, the request was refused
  as unclear, or the evidence didn't match the claim. You always know which one, and a receipt on
  disk backs it up.
- **The agent can't fake authority.** The worker process is handed no ledger path, no state
  directory, no socket — its only way to talk back to fleet is one narrow, size-bounded channel
  that fleet itself reads and checks. It can hand back a patch; it cannot commit one, and it cannot
  touch your real branch.
- **It runs on your machine, under your login.** No multi-tenant service, no account to trust with
  your source, no metered API key — fleet drives the `claude`/`codex` CLI you already signed into.
- **Every action leaves a receipt.** Pass, fail, or refusal — fleet writes tamper-evident proof
  before it moves on, so "what actually happened" is answerable later, not just remembered.

## How a run actually moves

Every `fleet run` goes through the same eight stages, in order, and can pick back up exactly where
it left off if it's interrupted partway through:

```
Event -> Classify -> Scan -> Plan -> Dispatch -> Verify -> Merge -> Teach
```

1. **Event** — something asks for work: you, or a hooked-up trigger.
2. **Classify / Scan / Plan** — fleet works out what's actually being asked and whether a plan is
   even needed before touching code.
3. **Dispatch** — a worker agent runs in its own private git worktree, never your real branch.
4. **Verify** — the real gate commands run: tests, lint, a secret scan, whatever the repo defines.
   The model's own claim of success is never one of them.
5. **Merge** — only a verified result gets integrated, and only fleet's own git operations do it,
   never the worker's.
6. **Teach** — what happened gets written down as a lesson, scoped to when it actually applies
   again — not promoted to a universal rule just because it happened once.

Ask for several of these at once (`fleet swarm`, `fleet run-modules`) and fleet runs them across
separate worktrees rather than one at a time, and won't schedule two workers against files the
other is already touching.

## Install

macOS only. From this directory:

```sh
./install.sh
```

The installer checks for `cargo`, `rustc`, and `git`, builds a release binary, installs it to
`~/.local/bin`, and initializes `$FLEET_STATE` with mode `700`. Use `./install.sh --check` for a
no-change preflight, or `./install.sh --uninstall` to remove the binary while keeping your
receipts.

## Try it

```sh
fleet doctor          # is this machine set up correctly, in plain terms
fleet route --role builder --json   # which installed agent would take this job, and why
fleet run --repo .    # run the real pipeline above against a repo
fleet gate --id <id>  # run one gate by name and get its real exit code
```

Run `fleet help` for the full command list with examples, and `fleet completions zsh` (or `bash`,
`fish`) for shell completion. To gate a repo that isn't a Rust/Cargo project, give it a
`.fleet/gates.toml` naming its own commands (a Node repo can map the unit-test gate to
`npm run test:unit`); with no such file, the committed defaults run unchanged.

## Exit codes

| Code | Meaning |
|---:|---|
| 0 | Success |
| 3 | Environment fault — missing tool, state, or usable local resource |
| 6 | Invariant violation — the local workflow could not safely proceed |
| 7 | Refusal — ambiguous, empty, or unsupported input |
| 8 | Verification mismatch — the evidence does not match the claim |

An environment fault (3) is never reported as an agent failure — the check itself couldn't run,
which is a different claim from running it and finding a real problem.

## Where this stands today

[`docs/LLD/LLD.md`](docs/LLD/LLD.md) is fleet's full design: 36 crates, multi-repo coordination,
quota failover, a self-improving router — 47 requirements in total. It's a proposal under active
review, not a claim that all of it is built yet; its own §25 keeps an honest running list of where
the code and the design still disagree. What's real today, and what this README describes, is the
CLI in this repo: the eight-stage pipeline above, deterministic gates with typed exit codes,
worktree-isolated workers, and receipt-backed runs.

## Limitations

The current attestation is STRUCTURE, not TRUST, until an independent holdout oracle lands. A
passing repository check is not the same claim as a correct user outcome — the person running
fleet is still the root of trust. Receipts and artifacts are tamper-*evident*, not tamper-*proof*.
Fleet is local to one developer account on one Mac: it does not provide multi-user authorization,
remote durability, or independent third-party verification.

## Learn more

- [`AGENTS.md`](AGENTS.md) — the hard rules this codebase is built under, and the real incident
  behind each one.
- [`TARGET.md`](TARGET.md) — what fleet deliberately is *not* (no server, no multi-tenant, no
  cloud), read before proposing anything that sounds like one.
- [`docs/LLD/LLD.md`](docs/LLD/LLD.md) — the full design, including how it compares against other
  agent harnesses (§22) and exactly which functional requirements are and aren't built yet (§24–25).
- [`TECH_DEBT.md`](TECH_DEBT.md) — the open, evidence-backed defect list, each row with a
  reproducing command.
- `fleet doctor` — the honest answer to "why is my machine unhappy," from the CLI itself.
