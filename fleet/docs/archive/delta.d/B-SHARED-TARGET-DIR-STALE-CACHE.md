# B-SHARED-TARGET-DIR-STALE-CACHE — worktree removal can leave a stale binary

Found live, 2026-08-30 ~02:50 IST. `AGENTS.md`'s own worktree section instructs setting
`export CARGO_TARGET_DIR="$PWD/target-shared"` so parallel worktree builds share one cache
instead of each rebuilding the crate from scratch. This session built and tested from two
short-lived worktrees (`.worktrees/test-fanout`, `.worktrees/test-claude-fanout`) using that same
shared target dir, then removed both worktrees (`git worktree remove --force`) once done.

Immediately after, `cargo test` from the **main repo** started failing 5 real tests
(`intent::tests::every_intent_route_selects_declared_resolved_skills`,
`swarm::tests::credit_requires_verifying_evidence`,
`swarm::tests::lead_code_gate_refuses_code_and_accepts_no_code`,
`swarm::tests::pristine_store_seeds_persistent_per_agent_records`,
`swarm::tests::verified_dispatch_advances_distinct_agent_states_and_scorecards`), all with the
same symptom: `Invariant("cannot load agent registry (exit 3)")`.

Root cause: `agent.rs:97` `Registry::load_default()` (and `skills.rs:117`, same pattern) resolve
`agents.toml`/`skills.toml` via `env!("CARGO_MANIFEST_DIR")` — a path baked in **at compile time**,
not read at runtime. With a shared `target-shared`, Cargo's fingerprinting did not force a
recompile when the crate was later invoked from a *different* manifest path (the main repo, after
having last been compiled from within a worktree) — the cached test binary kept the worktree's
now-deleted `CARGO_MANIFEST_DIR` baked in, so `agents.toml` resolved to a path under a directory
that `git worktree remove` had already deleted. `fs::read_to_string` on that dead path failed with
`ENOENT` → `EXIT_ENV` (3) → `Registry::load_default()`'s `Err` → the panic.

Confirmed and fixed by forcing a rebuild (`touch fleet/src/agent.rs && cargo test ...`) — the test
passed immediately after. Not a logic bug in either file; a real gap in `AGENTS.md`'s own
worktree-sharing advice for any code using `env!("CARGO_MANIFEST_DIR")`.

**Not fixed at the root** (would need either: stop using `env!(CARGO_MANIFEST_DIR)` for a path
that can legitimately change across builds sharing one target dir, e.g. resolve relative to
`std::env::current_exe()` at runtime instead; or document "after removing a worktree that shared
your `CARGO_TARGET_DIR`, force a rebuild before trusting `cargo test`" in `AGENTS.md`'s own
worktree section). Left as a real, scoped backlog item — this fragment is the write-up, not the
fix.
