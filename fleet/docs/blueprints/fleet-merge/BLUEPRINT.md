# BLUEPRINT — `fleet-merge`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-merge`
- **One-line purpose:** Own worktree creation/removal for isolated role lanes and the merge-back
  gate that gives lane work a home in the real branch — refusing an empty merge, verifying HEAD
  actually advanced, and verifying files actually changed, so `git merge` exiting `0` is never
  mistaken for "the lane's work landed."
- **Build branch:** `extract` (MIGRATION-PLAN §3 row 9) — both source files are already the working,
  tested primitive; nothing here is greenfield. **Caveat (read §5/divergence note): `worktree.rs` is
  cited as reuse evidence by BOTH this row and row 13 (`fleet-worker`), and one of its functions
  (`lane_cap`) is a scheduling concern, not a merge concern — the extraction excludes it. See the
  divergence note at the end of this file.**
- **Imports:** `fleet-types` (`ExitCode` — every typed error here maps to fleet's existing
  `EXIT_ENV`/`EXIT_INVARIANT` exit-code contract via `fleet_types::ExitCode`, so this crate's callers
  keep the same process exit-code taxonomy the monolith already commits to, instead of inventing a
  second one).
- **Imported by:** `fleet-worker` (spawns the process that does work *inside* a worktree this crate
  created, then calls this crate's `merge_lane` to land it and `remove` to clean up — see divergence
  note for why the edge runs this direction, not the reverse), `src/` (composition root — CLI
  dispatch, receipt-writing on refusal via `fleet-store`).

## 2. Responsibility & non-goals

**Owns:** the full lifecycle of one isolated worktree (naming, creation, removal) and the three hard
invariants that make a merge-back trustworthy: **(1)** a lane that staged zero files is refused, not
silently merged as a no-op success; **(2)** a merge that did not move `HEAD` is refused, even if
`git merge` itself exited `0`; **(3)** a merge that moved `HEAD` but changed zero files is refused.
These three invariants exist because all three have already failed silently in production once each
(see `fleet/bin/merge-lane.sh`'s own header comment, D31) — this crate is the typed, testable form of
that lesson, not a new idea.

**Non-goals (the seam):**
- Does **not** decide *how many* lanes run concurrently, or size the concurrency cap to available
  cores — that is a scheduling/workforce-sizing decision (today's `worktree.rs::lane_cap`, a
  scheduling concern for whichever crate dispatches lanes) and is excluded from this crate; see the
  divergence note.
- Does **not** spawn the CLI/adapter process that does work inside a worktree, and does not decide
  which model/adapter runs it — that is `fleet-worker`'s job (today's fd-3 `dup2` plumbing and crew
  adapters in `main.rs`/`agent.rs`). This crate only creates the directory the worker runs inside and
  merges/removes it afterward.
- Does **not** write ledger receipts on refusal or success — that is `fleet-store`'s job (today's
  implicit assumption in `merge-lane.sh`'s caller, which currently only echoes to stdout).
- Does **not** parse CLI args or print human-formatted output — that is the composition root's job
  (`src/`); this crate returns typed values/errors only.
- Does **not** decide routing/role/model policy — unrelated to `fleet-router`; this crate never reads
  a `Role` or a `Decision`.
- Does **not** invalidate build caches as a hard requirement of merging — `invalidate_build_cache`
  (§3) exists because `merge-lane.sh` does it (D34/M3), but it is deliberately best-effort and
  non-fatal, exactly as the script treats it (`|| true`); a cache-invalidation failure must never
  turn a successful merge into a refusal.
- Does **not** invent a second git abstraction. See §4's IO-injection note for why git is called as
  a direct subprocess rather than through an injected trait.

## 3. Public API contract

```rust
//! Worktree lifecycle + merge-back invariants for fleet's parallel role lanes.
//!
//! This crate answers two questions, both IO-boundary-adjacent but each with a hard, previously-
//! violated-in-production invariant behind it: "did creating an isolated worktree actually succeed
//! and exist on disk" (`worktree` module) and "did merging a lane's branch back actually land real
//! work, not a silent no-op" (`merge` module, `invariant` submodule holds the three pure checks so
//! they are unit-testable without a real git repo). Git itself is invoked as a direct subprocess
//! (`std::process::Command`), matching `fleet/keel/fleet/src/worktree.rs`'s and
//! `fleet/bin/merge-lane.sh`'s existing pattern exactly -- see the module doc on `git_exec` for the
//! justification (no `git2`/libgit2 dependency exists anywhere in the workspace's `Cargo.lock`
//! today, and a trait-injected git boundary would buy no unit-test speed here: the invariants this
//! crate exists to protect are properties of *real* git output, and every integration test already
//! must run against a real temp repo -- a mock would let a bug in the actual git invocation slip
//! through untested, which is exactly the kind of proxy this codebase's law rejects).

use std::path::{Path, PathBuf};
use fleet_types::ExitCode;

// =====================================================================================
// A. Worktree lifecycle -- fleet/keel/fleet/src/worktree.rs:27-164
// =====================================================================================

/// A live, isolated worktree. Removal is the caller's responsibility (call `remove`) -- this type
/// does not implement `Drop`-based cleanup, preserved verbatim from `worktree.rs:27-32`'s
/// deliberate rejection: a swallowed `Drop` failure is exactly the kind of proxy this codebase's
/// standing law rejects. Callers MUST pair every `create` with a `remove` on both the success and
/// the error path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub name: String,
}

/// Why `create` or `remove` failed. Every variant maps to fleet's existing two-code exit
/// contract via `exit_code()` -- this crate does not invent a third exit meaning.
#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    /// `name` was empty or all-whitespace. Mirrors `worktree.rs:53-55`'s `EXIT_INVARIANT` guard.
    #[error("worktree name must not be empty")]
    EmptyName,
    /// `git worktree add` failed on every one of the 8 jittered-retry attempts. Mirrors
    /// `worktree.rs:65-106`'s retry loop exhausting without success.
    #[error("git worktree add failed after {attempts} retries: {stderr}")]
    CreateFailed { attempts: u32, stderr: String },
    /// `git worktree add` reported success but the target directory does not exist on disk.
    /// Mirrors `worktree.rs:107-110`'s post-create existence check -- a real observed-vs-claimed
    /// gap, not a hypothetical.
    #[error("git reported success but {path} does not exist")]
    MissingAfterCreate { path: PathBuf },
    /// `git worktree remove --force` failed AND the filesystem fallback (`remove_dir_all`) left
    /// the directory still present. Mirrors `worktree.rs:136-141`'s leaked-directory check.
    #[error("worktree at {path} could not be removed -- it still exists on disk")]
    RemoveLeaked { path: PathBuf },
    /// `remove` was asked to remove a worktree whose directory does not exist on disk (built
    /// implementation only — pins D2 from fleet-cli E2E findings: a path that was never a
    /// worktree, or was already removed, used to fall through to a silent `Ok(())`; a caller
    /// asking for a removal that has nothing to remove must not be told it succeeded).
    #[error("no worktree exists at {path} -- nothing to remove")]
    NotFound { path: PathBuf },
    /// The `git` process itself could not be spawned (binary missing, permissions). Mirrors the
    /// `.map_err(|_| EXIT_ENV)` sites at `worktree.rs:84,133`.
    #[error("could not spawn git: {0}")]
    Spawn(String),
}

impl WorktreeError {
    /// Maps to fleet's existing process exit-code contract (`fleet_types::ExitCode`), preserving
    /// `worktree.rs:21-23`'s `EXIT_ENV = 3` / `EXIT_INVARIANT = 6` split by meaning, not by name.
    pub fn exit_code(&self) -> ExitCode {
        match self {
            WorktreeError::EmptyName
            | WorktreeError::MissingAfterCreate { .. }
            | WorktreeError::RemoveLeaked { .. } => ExitCode::Invariant,
            WorktreeError::CreateFailed { .. }
            | WorktreeError::Spawn(_)
            | WorktreeError::NotFound { .. } => ExitCode::Env,
        }
    }
}

/// Build a worktree name unique per call even under concurrent callers racing the identical task:
/// pid + a monotonic in-process counter + the caller's label. Never a content digest of the task
/// alone. Verbatim port of `worktree.rs:39-46`.
pub fn unique_name(label: &str) -> String { unimplemented!() }

/// `git -C <repo> worktree add -b fleet/<name> .worktrees/<name> HEAD`, with up to 8 jittered
/// retries against git's transient `.git/index.lock` contention (observed directly: 3 of 4
/// concurrent lanes succeed, the 4th fails fast mid-checkout -- `worktree.rs:58-64`'s own evidence
/// comment). Named branch, never `--detach`: a human must be able to inspect the lane's branch
/// afterward. Verbatim port of `worktree.rs:52-116`, with `Result<_, i32>` replaced by
/// `Result<_, WorktreeError>` per this crate's typed-error rule.
pub fn create(repo: &Path, name: &str) -> Result<Worktree, WorktreeError> { unimplemented!() }

/// `git -C <repo> worktree remove --force .worktrees/<name>`, with a filesystem fallback
/// (`remove_dir_all`) and a best-effort branch delete (`git branch -D`) so a failed lane never
/// leaks a worktree directory OR a dangling branch. The branch-delete and `worktree prune` calls
/// are deliberately best-effort (not surfaced as `WorktreeError`) -- only a *surviving worktree
/// directory* is treated as a leak, matching `worktree.rs:122-164`'s exact behavior verbatim.
pub fn remove(repo: &Path, worktree: &Worktree) -> Result<(), WorktreeError> { unimplemented!() }

// =====================================================================================
// B. Merge-back gate -- fleet/bin/merge-lane.sh:1-33
// =====================================================================================

/// What a successful, verified merge actually moved. Every count here is measured after the
/// fact from real git output, never assumed from a `git merge` exit code alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergeOutcome {
    pub branch: String,
    /// Files staged inside the worktree before the lane's own commit (`merge-lane.sh:11`'s
    /// `STAGED`).
    pub staged_files: usize,
    /// Files whose content differs between the pre-merge and post-merge `HEAD` in the target repo
    /// (`merge-lane.sh:21`'s `CHANGED`) -- distinct from `staged_files`, which counts inside the
    /// worktree, not the target repo.
    pub changed_files: usize,
    /// Target repo's `HEAD` sha before the merge (`merge-lane.sh:17`'s `BEFORE`).
    pub before: String,
    /// Target repo's `HEAD` sha after the merge (`merge-lane.sh:19`'s `AFTER`).
    pub after: String,
}

/// Why `merge_lane` refused. Every variant is one of `merge-lane.sh`'s named, already-shipped
/// refusal lines (D31's own fix) -- this crate does not add new refusal conditions beyond what the
/// script already enforces; it only makes them typed instead of a printed string + exit code.
#[derive(Debug, thiserror::Error)]
pub enum MergeRefusal {
    /// `merge-lane.sh:9`. No directory at the given worktree path.
    #[error("no worktree at {0}")]
    NoWorktree(PathBuf),
    /// `merge-lane.sh:10`. `git add -A` itself failed (not "staged nothing" -- the command errored).
    #[error("git add failed in {0}")]
    StageFailed(PathBuf),
    /// `merge-lane.sh:12-14`, the D31 fix itself: staged exactly 0 files.
    #[error("lane {branch} staged 0 files -- it produced nothing")]
    EmptyStage { branch: String },
    /// `merge-lane.sh:15-16`. The lane's own commit failed.
    #[error("commit failed for lane {branch}")]
    CommitFailed { branch: String },
    /// `merge-lane.sh:18`. `git merge` reported a conflict.
    #[error("conflict merging lane {branch}")]
    Conflict { branch: String },
    /// `merge-lane.sh:20`. `git merge` exited 0 but `HEAD` is byte-identical before and after --
    /// the exact silent-success case D31's header comment describes.
    #[error("merging lane {branch} moved HEAD nowhere -- nothing was integrated")]
    HeadUnmoved { branch: String },
    /// `merge-lane.sh:22`. `HEAD` moved but the diff between before/after touches 0 files.
    #[error("merging lane {branch} changed 0 files")]
    NoFilesChanged { branch: String },
    /// The `git` process itself could not be spawned.
    #[error("could not spawn git: {0}")]
    Spawn(String),
}

impl MergeRefusal {
    /// Mirrors `merge-lane.sh`'s own exit codes exactly: usage/missing-directory is `exit 3`
    /// (`ExitCode::Env`); every other refusal in the script is `exit 6` (`ExitCode::Invariant`).
    pub fn exit_code(&self) -> ExitCode {
        match self {
            MergeRefusal::NoWorktree(_) => ExitCode::Env,
            _ => ExitCode::Invariant,
        }
    }
}

/// Stage, commit, and merge one lane's worktree branch into the target repo's current `HEAD`,
/// enforcing all three invariants in order (empty-stage, head-unmoved, no-files-changed) exactly
/// as `merge-lane.sh:10-23` does today. `repo` is the target repo whose `HEAD` must advance;
/// `worktree_dir` is the lane's worktree (staging/commit happen there); `branch` is the lane's
/// branch name (e.g. `fleet/<name>` from `Worktree.branch`). Verbatim port of the script's control
/// flow, with each refusal line replaced by a typed `MergeRefusal` variant instead of an `echo` +
/// `exit`.
pub fn merge_lane(repo: &Path, worktree_dir: &Path, branch: &str) -> Result<MergeOutcome, MergeRefusal> {
    unimplemented!("port merge-lane.sh:9-23 verbatim, calling the three pure checks below at the \
                    point each corresponding shell guard runs today")
}

// =====================================================================================
// C. Pure invariant checks -- the D31 gate, factored out so it is unit-testable without git
// =====================================================================================

/// `merge-lane.sh:12-14`'s guard, as a pure fn: `staged_files == 0` is a refusal, any positive
/// count passes. Exposed as its own `pub fn` (not inlined into `merge_lane`) specifically so this
/// invariant has a unit test that never spawns a git process.
pub fn check_stage_nonempty(staged_files: usize, branch: &str) -> Result<(), MergeRefusal> {
    unimplemented!()
}

/// `merge-lane.sh:20`'s guard: `before == after` (byte/string equality of the two sha hex strings)
/// is a refusal. Never treats a shorter/abbreviated sha as equal to a full one -- both `before`
/// and `after` must come from the same `git rev-parse HEAD` invocation shape (full 40-hex sha),
/// which `merge_lane` guarantees by construction.
pub fn check_head_moved(before: &str, after: &str, branch: &str) -> Result<(), MergeRefusal> {
    unimplemented!()
}

/// `merge-lane.sh:22`'s guard: `changed_files == 0` is a refusal even though `HEAD` moved (e.g. an
/// empty merge commit with no diff) -- this is a distinct failure mode from `HeadUnmoved` and must
/// name itself distinctly so an operator isn't told "nothing moved" when something did move, just
/// with no content.
pub fn check_files_changed(changed_files: usize, branch: &str) -> Result<(), MergeRefusal> {
    unimplemented!()
}

// =====================================================================================
// D. Best-effort build-cache invalidation -- fleet/bin/merge-lane.sh:25-33 (D34/M3)
// =====================================================================================

/// After a successful merge, invalidate the cached `fleet` build artifact so a stale
/// `CARGO_MANIFEST_DIR`/`CARGO_TARGET_DIR` baked in at compile time from the now-removed worktree
/// never silently serves a green build pointing at a path that no longer exists (D34/M3, caught
/// twice). Deliberately returns `bool` (ran vs. did-not-run), never `Result` -- a failure inside
/// `cargo clean` is swallowed exactly as `merge-lane.sh:31`'s `|| true` swallows it, because this
/// step is advisory cache hygiene, not a merge invariant; it must never turn a real, verified merge
/// into a refusal. Only called if `cargo` is on `PATH` and `manifest_path` exists, matching
/// `merge-lane.sh:30`'s guard.
pub fn invalidate_build_cache(manifest_path: &Path, package: &str) -> bool { unimplemented!() }
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `Worktree` | `path`/`branch`/`name` are only ever produced by `create` — no public constructor lets a caller hand-build a `Worktree` pointing at a directory git never created. | A `remove` call operating on a directory that was never actually a git worktree (and so isn't in git's worktree admin list), silently no-op-ing instead of surfacing a real leak. |
| `WorktreeError` / `MergeRefusal` | Every fallible path returns one of these two enums — never a bare `i32` exit code (today's `worktree.rs` shape) or a printed string + shell exit (today's `merge-lane.sh` shape). `exit_code()` is the *only* place either enum collapses back to a raw code, and it is a pure, total, non-panicking match. | A caller silently pattern-matching on a raw `i32` and getting the wrong branch because `EXIT_ENV`/`EXIT_INVARIANT`'s numeric values (3/6) are easy to transpose — the enum variant name is what a reviewer reads, not a memorized magic number. |
| `MergeOutcome` | Only constructed by `merge_lane` after all three invariants (`check_stage_nonempty`, `check_head_moved`, `check_files_changed`) have already passed. | A `MergeOutcome` existing at all while `changed_files == 0` or `before == after` — those states are refusals, not `MergeOutcome` variants, so a caller can never accidentally read a "successful" outcome that actually changed nothing. |
| `check_head_moved`'s `before`/`after` | Compared by exact string equality on the full 40-character hex sha — never truncated/abbreviated shas, which could collide. | Two distinct commits whose abbreviated shas happen to share a prefix being wrongly treated as "HEAD didn't move." |

**Money/precision:** no money type in this crate. `staged_files`/`changed_files` are `usize` counts
of file paths, never float — a fractional file count is meaningless.

**Clock/RNG/IO injection points:** `create`'s retry-jitter reads `SystemTime::now()` directly
(ported verbatim from `worktree.rs:71-76`) — this is a **known, accepted exception**, not silently
introduced: the jitter's only job is to desynchronize sibling lanes' retry timing, it is never
compared against a caller-visible value, and it produces no test-observable output (the retry count
and final success/failure are what tests assert on, never the sleep duration). Every other function
in this crate is IO via direct `std::process::Command` subprocess calls to `git` — the boundary is
named, not injected as a trait (see §3's module doc for the justification): every git invocation
site is `Command::new("git")` inside `worktree.rs`/`merge.rs`, mirroring today's `worktree.rs`/
`merge-lane.sh` exactly, and integration tests exercise these against real `tempfile::TempDir` repos
rather than a mock, per the brief's requirement. `check_stage_nonempty`/`check_head_moved`/
`check_files_changed`/`invalidate_build_cache`'s `bool` decision, and `WorktreeError::exit_code`/
`MergeRefusal::exit_code`, are pure and IO-free, and are unit-tested without any git process at all.

## 5. Reuse map

Source read in full: `fleet/keel/fleet/src/worktree.rs` (243 lines, 2026-09-08) and
`fleet/bin/merge-lane.sh` (33 lines, 2026-09-08).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `worktree.rs:21-23` (`EXIT_ENV`, `EXIT_INVARIANT`) | Two raw `i32` exit constants. | no, replaced | Collapsed into `fleet_types::ExitCode` (already owns this taxonomy per its blueprint); `WorktreeError`/`MergeRefusal::exit_code()` map to it instead of redeclaring the constants a 15th time. |
| `worktree.rs:27-37` (`Worktree`) | Public struct, no `Drop`. | yes, verbatim | Fields already `pub`; no change. |
| `worktree.rs:39-46` (`unique_name`) | pid + atomic counter + label. | yes, verbatim | None — logic and signature unchanged. |
| `worktree.rs:52-116` (`create`) | `git worktree add` with 8x jittered retry + prune-on-failure + post-create existence check. | yes, logic verbatim | `Result<Worktree, i32>` → `Result<Worktree, WorktreeError>`; the three distinct failure modes (retries exhausted, spawn failure, missing-after-create) that today all collapse to `EXIT_ENV`/`EXIT_INVARIANT` become distinct enum variants carrying the real diagnostic (`stderr`, `attempts`, `path`) instead of being discarded (today's `let _ = last_stderr;` at `worktree.rs:104` explicitly throws away the exact string a `CreateFailed{stderr}` variant should carry). |
| `worktree.rs:118-164` (`remove`) | `git worktree remove --force` + filesystem fallback + best-effort branch/prune cleanup. | yes, logic verbatim | `Result<(), i32>` → `Result<(), WorktreeError>`; same enum-variant upgrade as `create`. |
| `worktree.rs:166-174` (`lane_cap`) | `min(16, available_parallelism()-2)`, floored at 1. | **no — belongs in `fleet-worker`, not here** | This is a concurrency-sizing/scheduling decision (how many lanes to *dispatch*), not a worktree-lifecycle or merge-back concern. It has no dependency on `Worktree`/`create`/`remove` at all — it is a pure function of the host's core count. See divergence note: MIGRATION-PLAN cites `worktree.rs` for both this crate (row 9) and `fleet-worker` (row 13) without saying which function goes where; this blueprint resolves it by excluding `lane_cap`. |
| `worktree.rs:176-243` (`#[cfg(test)] mod tests`) | 3 tests: create/remove round-trip, name-collision-free, lane-cap bounds. | as inspiration, not verbatim | Port `create_and_remove_round_trips_cleanly`/`two_concurrent_names_never_collide` into this crate's test plan (§9); `lane_cap_is_at_least_one_and_at_most_sixteen` moves with `lane_cap` itself, wherever that function re-homes. |
| `merge-lane.sh:1-9` (header, usage, dir-exists check) | D31 rationale comment + `[ -d "$D" ]` guard, `exit 3`. | yes, ported to types | `MergeRefusal::NoWorktree`, `exit_code() -> ExitCode::Env`. |
| `merge-lane.sh:10` (`git add -A ':!*.pyc' ':!*/target/*'`) | Stage everything except build artifacts/bytecode inside the worktree. | yes, verbatim pathspecs | Same exclusion pathspecs preserved exactly — dropping them would let a lane's own `target/` directory get staged and merged, which is exactly the kind of noise this exclusion exists to prevent. |
| `merge-lane.sh:11-14` (`STAGED` count + empty-stage refusal) | The D31 fix itself. | yes, logic verbatim | `check_stage_nonempty` (§3 C), unit-tested directly. |
| `merge-lane.sh:15-16` (commit with `core.hooksPath=/dev/null`) | Commit staged changes, bypassing local hooks (the lane's own commit is fleet-internal bookkeeping, not a human-authored commit subject to the repo's commit-msg hooks). | yes, verbatim | `MergeRefusal::CommitFailed` on failure. |
| `merge-lane.sh:17-20` (`BEFORE`/`AFTER` + head-unmoved refusal) | Capture `HEAD` before/after `git merge --no-edit -q`, refuse the silent-success case. | yes, logic verbatim | `check_head_moved` (§3 C), unit-tested directly; conflict (`merge` exits non-zero) is `MergeRefusal::Conflict`, checked separately before the head-moved comparison, matching the script's own line order (18 before 20). |
| `merge-lane.sh:21-23` (`CHANGED` count + no-files-changed refusal + success echo) | Final invariant + success message. | yes, logic verbatim | `check_files_changed` (§3 C), unit-tested directly; the success "echo" becomes the returned `MergeOutcome`, formatting deferred to the caller (`src/`) per §2's non-goals. |
| `merge-lane.sh:25-33` (D34/M3 cache invalidation) | Best-effort `cargo clean -q -p fleet` after a successful merge, only if `cargo`+manifest exist. | yes, verbatim, non-fatal | `invalidate_build_cache` (§3 D) — deliberately returns `bool`, not `Result`, matching the script's own `|| true`. |

## 6. Behavior spec

### `fn create(repo: &Path, name: &str) -> Result<Worktree, WorktreeError>`

| Input dimension | Behavior |
|---|---|
| empty | `name = ""` or all-whitespace → `Err(WorktreeError::EmptyName)` before any subprocess is spawned (mirrors `worktree.rs:53-55`'s early check). |
| null / `None` | n/a — `name: &str` is always present; an absent name is the caller's problem before calling `create`. |
| wrong-type | n/a — no type erasure at this boundary. |
| huge | `name` a 10,000-character string → passed through verbatim into the branch/path strings; git itself may reject an overlong ref name, surfacing as `CreateFailed{stderr}` carrying git's own error text — this crate does not pre-validate length, it reports git's real refusal. |
| negative | n/a — not numeric. |
| duplicate | Calling `create` twice with the same `name` against the same `repo`: the second call's `git worktree add -b fleet/<name> .worktrees/<name> HEAD` fails (branch/path already exist) on every retry attempt → `Err(CreateFailed{attempts: 8, stderr})`. Callers avoid this by always deriving `name` from `unique_name`, never a caller-chosen literal, for concurrent use. |
| concurrent | Two callers racing `create` against the *same* repo with *different* names (via `unique_name`) contend only on git's own transient `.git/index.lock` — the 8x jittered retry (ported verbatim) is exactly the mitigation; two callers racing with the *same* name collide as "duplicate" above, which is a caller error, not a race this crate must resolve. |
| unicode / non-ASCII | `name` containing non-ASCII (e.g. `"tâche"`) is passed through to git verbatim; git's own ref-name rules apply (git generally accepts UTF-8 branch names) — no additional validation or rejection here. |
| already-exists | Same as "duplicate" above. |
| partial-failure | `git worktree add` reports success (exit 0) but `path.is_dir()` is false → `Err(WorktreeError::MissingAfterCreate{path})` (mirrors `worktree.rs:107-110`) — the success claim is independently verified, never trusted from the exit code alone. |

### `fn remove(repo: &Path, worktree: &Worktree) -> Result<(), WorktreeError>`

| Input dimension | Behavior |
|---|---|
| empty | n/a — `worktree` is always a previously-`create`d value; there is no "empty worktree" input shape. |
| null / `None` | n/a — no `Option` at this boundary. |
| wrong-type | n/a. |
| huge | n/a — no size-dependent behavior; removal cost is git's own, not this crate's. |
| negative | n/a. |
| duplicate | Calling `remove` twice on the same already-removed `Worktree`: the built implementation checks `worktree.path.exists()` up front and returns `Err(WorktreeError::NotFound{path})` — this diverges from the spec text above (see the divergence note at the end of this file): `remove` is **not** idempotent as originally specified. The change pins D2 (fleet-cli E2E findings): a caller asking for a removal that has nothing to remove must not be told it succeeded. |
| concurrent | Two callers racing `remove` on the *same* `Worktree` value: whichever caller runs second sees `worktree.path.exists() == false` and gets `Err(NotFound)`, not a silently-successful no-op. |
| unicode / non-ASCII | Passed through verbatim; no validation performed on `worktree.name`/`.branch` since they were already validated (or not) at `create` time. |
| already-exists | See "duplicate" above. |
| partial-failure | `git worktree remove --force` fails AND the filesystem fallback (`remove_dir_all`) also fails to fully clear `worktree.path` → `Err(WorktreeError::RemoveLeaked{path})` — a real, observed leak, never silently reported as success. |

### `fn merge_lane(repo: &Path, worktree_dir: &Path, branch: &str) -> Result<MergeOutcome, MergeRefusal>`

| Input dimension | Behavior |
|---|---|
| empty | `worktree_dir` does not exist → `Err(MergeRefusal::NoWorktree(worktree_dir.to_path_buf()))` before any git command runs (mirrors `merge-lane.sh:9`). A worktree that exists but is completely empty (no tracked or untracked files) → `git add -A` stages 0 files → `Err(EmptyStage{branch})` (the core D31 case). |
| null / `None` | n/a — no `Option` parameters; `branch` is always a non-empty `&str` the caller already has from `Worktree.branch`. |
| wrong-type | n/a. |
| huge | A worktree with 100,000 changed files: `git add -A`/`git diff --name-only` both scale linearly with git's own cost, not this crate's — no additional buffering or in-memory limit is imposed beyond what `Command::output()` already buffers (git's own stdout, which for a name-only diff is line-per-file text, not file contents). |
| negative | n/a — no numeric input; `staged_files`/`changed_files` are always derived by counting lines of trusted git output, never caller-supplied. |
| duplicate | Calling `merge_lane` twice for the same already-merged `branch`: the second call's `git merge` on an already-merged branch typically reports "Already up to date" and `HEAD` does not move → `Err(MergeRefusal::HeadUnmoved{branch})` — a repeat merge is refused, not silently reported as a second success. |
| concurrent | Two callers racing `merge_lane` against the *same* target `repo`: git's own working-tree/index lock serializes the underlying `git add`/`commit`/`merge` calls at the OS level; a losing caller sees a `StageFailed`/`CommitFailed`/`Spawn` error from git's own lock contention, never a corrupted `MergeOutcome`. This crate does not add its own lock — it relies on git's, exactly as `merge-lane.sh` does today (no additional locking in the script either). |
| unicode / non-ASCII | `branch` names and file paths containing non-ASCII round-trip through git's UTF-8 output correctly; `staged_files`/`changed_files` counts are line counts of git's output, not character-set-dependent. |
| already-exists | See "duplicate" above. |
| partial-failure | `git add -A` fails outright (not "staged 0", the command itself errors, e.g. a corrupt index) → `Err(MergeRefusal::StageFailed(worktree_dir))`, distinct from `EmptyStage` and checked first, mirroring the script's `||` ordering at `merge-lane.sh:10`. A conflicting merge (git exits non-zero, working tree left mid-conflict) → `Err(MergeRefusal::Conflict{branch})`; this crate does **not** attempt `git merge --abort` on the caller's behalf — the conflicted state is left for a human/caller to resolve or abort explicitly, matching `merge-lane.sh:18`'s behavior exactly (the script does not abort either). |

### `fn check_stage_nonempty`, `fn check_head_moved`, `fn check_files_changed` (pure)

| Input dimension | Behavior |
|---|---|
| empty | `check_stage_nonempty(0, branch)` → `Err(EmptyStage)`; `check_head_moved("", "", branch)` → equal empty strings → `Err(HeadUnmoved)` (never reachable in practice since `merge_lane` always supplies real 40-hex shas, but the pure fn itself does not special-case empty strings as anything but "equal"). |
| null / `None` | n/a — all three take non-`Option` value types (`usize`/`&str`). |
| wrong-type | n/a — no type erasure. |
| huge | `check_stage_nonempty(usize::MAX, branch)` → `Ok(())` (any positive count passes; no upper bound is meaningful here). |
| negative | n/a — `usize` cannot be negative. |
| duplicate | n/a — no collection/identity concept in these three fns. |
| concurrent | All three are pure value fns with no shared state — trivially safe from any number of threads. |
| unicode / non-ASCII | `branch` containing non-ASCII is only ever interpolated into the returned error's `branch` field for display — byte-for-byte preserved, never validated or rejected by these fns. |
| already-exists | n/a. |
| partial-failure | n/a — no IO, cannot fail partially; each fn is a single synchronous comparison. |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `fleet-types` | workspace-path dependency (`{ path = "../fleet-types" }`) | Supplies `ExitCode`, so `WorktreeError::exit_code()`/`MergeRefusal::exit_code()` reuse fleet's existing process exit-code taxonomy instead of redeclaring it a 15th time (fleet-types' own §3 doc comment already lists 14 redeclaration sites; this crate must not become a 15th). |
| `thiserror` | `2.0.20` (matches `fleet/keel/Cargo.lock`'s already-resolved version) | Every fallible fn returns a `thiserror`-derived typed error (`WorktreeError`, `MergeRefusal`) instead of `String`/`anyhow`/a bare `i32` — this crate's hard requirement (template §3). |
| `tempfile` | `3.27.0` (dev-dependency; matches `fleet/keel/Cargo.lock`) | Integration tests construct real, disposable git repos under `tempfile::TempDir` — never the repo tree, per the brief and template §11's checklist item. |

No async runtime, no serialization framework: this crate's public types are plain structs/enums
with no wire-format contract of their own (unlike `fleet-types`' `Receipt`/`Attestation`), so no
`serde` dependency is needed here.

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.** Decomposed so the pure invariant checks (fast,
> git-free unit tests) are physically separate from the subprocess-calling code (slow, real-repo
> integration tests only) — a reviewer should be able to tell which test tier a file needs just from
> its name.

```
crates/fleet-merge/
  Cargo.toml
  src/
    lib.rs             # ~32 — module decls + re-exports only
    error.rs           # ~46 — WorktreeError (incl. NotFound, see §6 divergence note) + its exit_code(); re-exports MergeRefusal
    merge_refusal.rs   # ~44 — MergeRefusal + its exit_code(); split out of error.rs to keep WorktreeError's file under the line budget once NotFound was added
    git_exec.rs        # ~52 — run_git/run_git_quiet(repo, args), count_lines, jittered_sleep; shared subprocess helper used by worktree.rs and merge.rs, avoiding duplicated Command::new("git") boilerplate
    worktree.rs        # ~79 — Worktree, unique_name, create, remove (ports worktree.rs:27-164, minus lane_cap)
    invariant.rs       # ~35 — check_stage_nonempty, check_head_moved, check_files_changed (pure, no git_exec import)
    merge.rs           # ~78 — MergeOutcome, merge_lane (calls git_exec + invariant)
    cache.rs           # ~36 — invalidate_build_cache
  tests/
    common/mod.rs              # ~44 — shared init_repo/run test helpers used by every integration test file
    worktree_lifecycle.rs      # ~66 — create/remove round trip, invalid-name refusal, name-collision-free, leaked-directory reporting, all against a real tempdir repo
    worktree_remove_not_found.rs # ~33 — pins D2: remove reports NotFound on an already-removed worktree and on a path that never existed (split out of worktree_lifecycle.rs)
    merge_happy.rs              # ~30 — happy-path merge_lane end-to-end, real staged/changed counts
    merge_refusals.rs           # ~62 — empty-stage, head-unmoved (repeat merge), conflict refusal paths
    merge_zero_diff.rs          # ~30 — HEAD-moved-but-zero-files-changed refusal (empty merge commit)
    invariant_pure.rs           # ~61 — check_stage_nonempty/check_head_moved/check_files_changed + exit_code mapping unit tests with no git process at all
```
> The originally-sketched `merge_invariants.rs` was split into `merge_happy.rs` / `merge_refusals.rs`
> (and further into `merge_zero_diff.rs`) once bodies landed, as this section anticipated. The
> file-size gate (§10) is run before Opus review.

`Cargo.toml` sketch:
```toml
[package]
name = "fleet-merge"
version = "0.1.0"
edition = "2021"

[dependencies]
thiserror = "2.0.20"
fleet-types = { path = "../fleet-types" }

[dev-dependencies]
tempfile = "3.27.0"
```

## 9. Test plan

**Unit tests** (`invariant_pure.rs`, no git process):
- `stage_nonempty_rejects_zero_and_accepts_any_positive_count` — `check_stage_nonempty(0, _)` is
  `Err(EmptyStage)`; `check_stage_nonempty(1, _)` and `check_stage_nonempty(usize::MAX, _)` are both
  `Ok(())`.
- `head_moved_rejects_identical_shas_and_accepts_any_difference` — `check_head_moved("abc", "abc",
  _)` is `Err(HeadUnmoved)`; `check_head_moved("abc", "def", _)` is `Ok(())`.
- `files_changed_rejects_zero_and_accepts_any_positive_count` — mirrors the stage test shape.
- `exit_code_mapping_matches_merge_lane_sh` — every `MergeRefusal` variant except `NoWorktree` maps
  to `ExitCode::Invariant`; `NoWorktree` maps to `ExitCode::Env` — asserted exhaustively over all 7
  variants (a `match` with no wildcard arm in the test itself, so adding an 8th variant without
  updating this test fails to compile).

**Integration tests** (`worktree_lifecycle.rs`, `merge_invariants.rs`, real `tempfile::TempDir` repos,
calling only the public API):
- `create_and_remove_round_trips_cleanly` — ported from `worktree.rs:218-228`: `create` then
  `remove` leaves no directory behind.
- `create_rejects_empty_name_without_touching_git` — `create(repo, "")` is `Err(EmptyName)`; assert
  no `.worktrees/` directory was created at all (proves the check runs before any subprocess).
  Repeated for `create(repo, "   ")`.
- `create_twice_with_same_name_fails_on_retry_exhaustion` — the "duplicate" case from §6: second
  `create` call with an identical `name` against the same repo is `Err(CreateFailed{..})`.
  (marked `#[ignore]`-free but allowed to be slow: exercises the full 8-retry loop for real.)
  Publish real wall-clock time in the PR if this test is slow enough to need `--test-threads`
  tuning.
- `remove_reports_not_found_on_an_already_removed_worktree` /
  `remove_reports_not_found_for_a_path_that_never_existed` (`worktree_remove_not_found.rs`) — pin
  D2: `remove` is **not** idempotent as originally specified in this section; a second `remove` on
  an already-removed `Worktree`, or a `remove` on a path that never existed, both return
  `Err(WorktreeError::NotFound{..})`, never a silent `Ok(())`. See the divergence note at the end
  of this file.
- `two_concurrent_names_never_collide` — ported verbatim from `worktree.rs:230-235`.
- `merge_lane_happy_path_reports_real_staged_and_changed_counts` — create a worktree, write N new
  files, `merge_lane` succeeds, asserts `MergeOutcome.staged_files == N`,
  `.changed_files == N`, `.before != .after`, and the target repo's real `HEAD` now contains the
  files.
- `merge_lane_refuses_when_worktree_is_untouched` — create a worktree, change nothing, `merge_lane`
  returns `Err(EmptyStage{branch})`, and the target repo's `HEAD` is provably unchanged
  (`git rev-parse HEAD` before == after the call, checked from *outside* this crate's API as an
  independent oracle).
- `merge_lane_refuses_on_repeat_merge_of_an_already_merged_branch` — the D31 regression test: merge
  a branch once (succeeds), call `merge_lane` again for the same branch, assert
  `Err(HeadUnmoved{branch})` — this is the exact silent-success shape the invariant was written to
  catch, driven end-to-end through real git rather than only through the pure `check_head_moved`
  unit test.
- `merge_lane_reports_conflict_without_corrupting_the_target_repo` — construct a real conflicting
  change (same line edited on both the target's `HEAD` and the lane branch), assert
  `Err(Conflict{branch})`, and assert the target repo's `HEAD` sha is unchanged from before the call
  (a conflicted `git merge` leaves `MERGE_HEAD` set but does not advance `HEAD` — asserted directly,
  not assumed).
- `invalidate_build_cache_never_panics_without_a_manifest` — `invalidate_build_cache(<nonexistent
  path>, "fleet")` returns `false` and does not panic, mirroring `merge-lane.sh:30`'s guard.

**Mutation-testing targets** (`cargo mutants -p fleet-merge`):
- Flipping `staged_files == 0` to `staged_files <= 0` (a no-op for `usize` but a plausible mutant on
  the comparison operator, e.g. `>` vs `>=` at the boundary) in `check_stage_nonempty` must be
  killed by `stage_nonempty_rejects_zero_and_accepts_any_positive_count`'s `1` case.
- Deleting the `worktree.path.exists()` check inside `remove`'s partial-failure branch (so a leaked
  directory is silently reported as `Ok(())`) must be killed by a dedicated
  `remove_reports_leak_when_directory_survives_forced_remove` test that makes the removal
  genuinely fail (e.g. an open file handle or a read-only file inside the worktree on platforms
  where that blocks deletion) and asserts `Err(RemoveLeaked{..})`.
- Swapping `before != after` to `before == after` (or deleting the check entirely) in `merge_lane`'s
  call to `check_head_moved` must be killed by
  `merge_lane_refuses_on_repeat_merge_of_an_already_merged_branch`.
- Deleting the `check_files_changed` call after a successful `HEAD` move must be killed by a
  dedicated `merge_lane_refuses_on_head_move_with_zero_file_diff` test: construct an empty merge
  commit (`git merge --no-edit --allow-unrelated-histories` variant or an already-fast-forwarded
  case where `HEAD` moves but the diff is empty) and assert `Err(NoFilesChanged{branch})`.

**Property tests:** not applicable — this crate has no algebraic invariant beyond the three ordered
boolean gates already covered exhaustively by the unit + integration tests above; a property test
here would only re-state "any positive count passes, zero fails," which the unit tests already state
more directly.

## 10. Verification recipe

```bash
cd crates/fleet-merge
cargo test -p fleet-merge --all-targets
cargo clippy -p fleet-merge --all-targets -- -D warnings
cargo mutants -p fleet-merge
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
```
Expected: all unit + integration tests pass, 0 skipped — publish as `<passed>/<total>` (e.g. `14/14`,
never just "tests pass"). Clippy: 0 warnings. Mutants: every target named in §9 caught; publish
`<caught>/<total mutants>` — given the small, mostly-branch-heavy surface here, the floor is **100%
of viable mutants caught**; any survivor gets a new test, not a lowered floor. File-size gate: no
output from the `awk` line (a non-empty print means a file exceeds 80 lines and the crate fails
review regardless of test results).

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum (`WorktreeError`, `MergeRefusal`) — none
      swallowed into `bool`/`Option`/`String`/`.unwrap()` in non-test code. (`invalidate_build_cache`
      returning `bool` is a deliberate, documented exception for a non-fatal advisory step, not a
      swallowed error — see §3 D's doc comment.)
- [ ] Clock/RNG/IO documented: `create`'s retry jitter reads `SystemTime::now()` directly (accepted
      exception, §4); every other IO is a named, direct `git` subprocess call, never a trait
      boundary (justified in §3's module doc and §4).
- [ ] Thread-safety documented: every public fn takes owned/borrowed values and returns owned
      values with no interior mutability; concurrency safety for `create`/`remove`/`merge_lane`
      against the *same* repo relies on git's own lock, not a lock this crate adds (§6 "concurrent"
      rows) — say so explicitly in the crate's top-level doc comment when built.
- [ ] No float used for money or any precision-sensitive count — `staged_files`/`changed_files` are
      `usize`.
- [ ] No self-grading: the crate's own tests don't certify its own correctness as the sole gate —
      `cargo mutants` is run, not just unit tests; denominator published per §10 (mark done once
      actually run and recorded in the PR).
- [ ] The verify command's pass/fail denominator (`x/y`) is stated in this file and restated again
      in the PR — not just "green" (mark done with real numbers once built).
- [ ] Tests that touch the filesystem write only under `tempfile::TempDir` (both worktree-lifecycle
      and merge-invariant integration tests construct a fresh temp repo per test), never the repo
      tree or `$HOME`.
- [ ] Every non-goal in §2 is actually absent from the code: no lane-count/scheduling logic, no
      process-spawning-for-work (only `git` subprocess calls), no ledger/receipt writes, no CLI
      arg parsing or printing anywhere in `crates/fleet-merge/src/` — enforce with
      `grep -rn 'Command::new' crates/fleet-merge/src/` returning only `git` invocations, never any
      other binary.
- [ ] **No source file exceeds 80 lines** (verified: `find src tests -name '*.rs' | xargs wc -l` —
      every file ≤ 80). `lib.rs` is a thin hub, not a dumping ground.

## 12. Definition of Done

`fleet-merge` is DONE when: §10's four commands all pass with a published denominator (tests `N/N`,
clippy clean, mutants `M/M` caught, file-size gate silent) run from `crates/fleet-merge/`; every
unchecked box in §11 is checked with its real numbers; `registry/services/REGISTRY.md` (infrastructure
primitive, not a product feature — `services/`, per C1/L2) lists the crate; and Opus has
independently re-derived all three merge-back invariants and the worktree create/remove contract
from this blueprint alone (without re-reading `worktree.rs`/`merge-lane.sh`), reproduced the
"HEAD-unmoved-on-repeat-merge" mutation by hand, and driven one real `create` → write a file →
`merge_lane` → `remove` sequence end-to-end against a real temp repo, confirming the target repo's
`HEAD` and working tree reflect exactly what the lane produced.

---

## Divergence from MIGRATION-PLAN (for Opus)

1. **`worktree.rs` is cited as reuse evidence for two different crates without saying which function
   goes where.** MIGRATION-PLAN §3 row 9 (`fleet-merge`) cites `keel/fleet/src/worktree.rs`; row 13
   (`fleet-worker`) cites the same file again in its list (`worktree.rs · skills.rs · mcp.rs ·
   agent.rs · crew adapters · fd-3 in main.rs`). Having read the whole 243-line file, it splits
   cleanly: `Worktree`/`unique_name`/`create`/`remove` (worktree-lifecycle CRUD, no scheduling
   decision) belong here, in `fleet-merge`, because they are exactly what a merge-back needs
   (create the isolated space, and later remove it once merged) and exactly what this blueprint's
   §3/§5 extract. `lane_cap` (a pure function of `available_parallelism()`, unrelated to any
   `Worktree` value) is a scheduling/dispatch-sizing decision that belongs with whichever crate
   decides how many lanes to run concurrently — `fleet-worker`, matching row 13's framing ("highest-
   risk: no-ambient/hermetic probe lives here", i.e. the crate that actually spawns and bounds
   concurrent work). This blueprint resolves the citation collision by excluding `lane_cap` from
   `fleet-merge` entirely; row 9's evidence column should be narrowed to
   `worktree.rs:27-164` (excluding `lane_cap`) and row 13 should gain an explicit citation of
   `worktree.rs:166-174` so the split is documented, not implicit.
2. **The DAG edge direction: `fleet-worker` depends on `fleet-merge`, not the reverse.** MIGRATION-
   PLAN's stated DAG (`types → {lifecycle, store} → {siblings} → src/`) lists `fleet-merge` and
   `fleet-worker` as siblings with no edge between them stated either way. In practice, whichever
   crate spawns a lane's work *inside* a worktree must first call this crate's `create` to get that
   worktree, and later call `merge_lane`/`remove` to land and clean it up — so the real, necessary
   edge is `fleet-worker → fleet-merge` (a sibling→sibling edge, per the DAG note that such edges
   must be "minimal and named"). This blueprint assumes that edge is acceptable and names it
   explicitly here rather than silently adding it; MIGRATION-PLAN's per-blueprint "edges" section
   for `fleet-worker` should confirm or override this when its own blueprint is written.
3. **`worktree.rs`'s `create`/`remove` return `Result<_, i32>` today; this blueprint changes the
   signature to `Result<_, WorktreeError>`/`Result<_, WorktreeError>`.** This is a deliberate,
   template-mandated change (§3's "never a bare... error" rule), not a reuse gap — flagged here only
   so Opus's contract re-derivation knows the public signature is not byte-identical to today's
   fleet source, even though every code path and retry/verification behavior is ported verbatim.
4. **(post-build) `remove`'s idempotence contract changed from this spec's original text.** §3/§6 as
   originally written specified that calling `remove` on an already-removed (or never-existed)
   `Worktree` returns `Ok(())` — "idempotent by design," mirroring `worktree.rs:134-141`. The built
   crate (`crates/fleet-merge/src/worktree.rs`, `remove`) instead added a `WorktreeError::NotFound`
   variant and returns it in that case, citing "D2 (fleet-cli E2E findings)" in its doc comment and
   in `tests/worktree_remove_not_found.rs`'s header comment. This is a real, intentional contract
   change made during the build (not a doc-staleness artifact), so this pass updated §3/§6/§8/§9's
   prose to describe the shipped `NotFound` behavior rather than silently deleting the record of the
   original idempotent spec. Flagged for Opus/reviewer awareness: no D2 ticket or write-up was found
   under `docs/` to cross-check the claim against; the only evidence for the change is the two doc
   comments in the crate itself.
