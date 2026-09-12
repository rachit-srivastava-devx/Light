# Fleet blueprint migration — effects and operations research

**Research group:** D  
**Snapshot:** 2026-09-11 (Asia/Kolkata)  
**Local checkout:** `main` at `82454636847ddd2bc8b2e8aa6dc463a822f14bf6`; pre-existing dirty files were preserved.  
**Scope:** `integrate`, `approval`, `notify`, `broker`, `rollback`, multi-day persistence, resource limits, grants/capabilities, and reconciliation.

## Decision in one page

1. **Integrate and rollback with the real `git` CLI.** Use `git worktree` and `git merge-tree`/`git merge` as the authority for worktree and merge semantics. Do not add `git2` to the migration kernel.
2. **Persist effects in SQLite, not an external broker.** One transaction writes the domain transition and its outbox row; a bounded Tokio channel only wakes in-process consumers. Every effect has an idempotency key, lease, attempt count, and explicit `unknown` outcome.
3. **Make approval a durable, human-minted capability.** Approval must bind task id, scope/diff hash, base commit, actor, reason, and expiry. A restart may resume a still-valid approval; it may not infer approval from a completed agent step.
4. **Treat notifications as best-effort projections.** Terminal/JSONL is the deterministic baseline. Desktop notification transports are optional capability probes and never determine workflow success.
5. **Use OS-specific controls behind a capability probe.** Existing `nix` process-group/RLIMIT controls remain the baseline. Add Linux Landlock only when the running kernel proves support; do not make macOS `sandbox-exec` or Linux `bubblewrap` a silent mandatory dependency.
6. **Reconcile before retrying unknown side effects.** Startup, periodic sweeps, and operator commands compare durable intent, receipts, leases, and provider readback. A zero-input reconciliation is failure.

## Evidence labels and adoption rules

- **D (documented):** upstream release, source, API, or operating-system documentation. It proves what the upstream project claims, not that Fleet executed it.
- **L (locally verified):** a command was executed in this checkout/host during this research run and its output is recorded below.
- **U (unverified here):** proposed smoke or platform-specific behavior not executed in this macOS checkout.
- Exact release metadata below uses the upstream release/tag commit where available. A tag commit is not a proof that the dependency is present in this checkout.

## Candidate inventory

| Area | Exact upstream snapshot | License | Adoption decision | Smoke command | Evidence |
|---|---|---|---|---|---|
| Git CLI | Git `v2.51.0`, commit [`c44beea485f0`](https://github.com/git/git/commit/c44beea485f0f2feaf460e2ac87fdd5608d63cf0), tagged 2025-08-18 | GPL-2.0-only | **Adopt.** Parent-owned subprocesses are the mutation authority. | `git --version`; `git worktree list --porcelain`; `git merge-tree --write-tree HEAD HEAD` | **D:** [worktree docs](https://git-scm.com/docs/git-worktree), [merge docs](https://git-scm.com/docs/git-merge). **L:** `git version 2.51.0`; worktree listing and a same-tree merge preflight both returned successfully. |
| `git2` / libgit2 | `git2` `0.21.0`, tag commit [`dffaf272eb0e`](https://github.com/rust-lang/git2-rs/commit/dffaf272eb0e62ac15b74283c4e488252db9afc3), 2026-05-18; upstream currently requires libgit2 `1.9.6+` | MIT OR Apache-2.0; libgit2 GPLv2 with linking exception | **Do not adopt for integrate/rollback.** It is useful for read-only object/diff inspection, but adds a second implementation of worktree/merge behavior and a native library surface. | `cargo test -p fleet-merge`; a dedicated `git2::Repository::open` smoke only if a later read-only adapter is proposed | **D:** [git2-rs source](https://github.com/rust-lang/git2-rs), [libgit2 release line](https://github.com/libgit2/libgit2/releases). **L:** `cargo tree -p fleet-merge --depth 1` showed no `git2`; no git2 API was run. |
| SQLite/Rust binding | `rusqlite` `0.40.2`, tag commit [`e88f112bef78`](https://github.com/rusqlite/rusqlite/commit/e88f112bef7899234a497baed5cc3c3d553deeb8), released 2026-08-08; MIT | MIT | **Adopt the existing SQLite authority; upgrade separately.** Current checkout resolves `rusqlite 0.32.1` with `bundled` and `load_extension`; do not mix a version upgrade into the blueprint migration. Enable the upstream `backup` feature only with a real backup smoke. | `sqlite3 :memory: 'select sqlite_version();'`; future adapter gate: `cargo test -p fleet-store --features backup` | **D:** [rusqlite source/features](https://github.com/rusqlite/rusqlite), [SQLite backup API](https://sqlite.org/backup.html). **L:** system SQLite reported `3.51.0`; `cargo tree -p fleet-store --depth 1` showed `rusqlite v0.32.1`. Backup API was not locally exercised. |
| Tokio channels | Tokio `1.53.1`, commit [`75fef53d0a85`](https://github.com/tokio-rs/tokio/commit/75fef53d0a8590c2d1dbb63672aa7b7d1ef51155), released 2026-07-20 | MIT | **Adopt only as an ephemeral broker.** Use bounded `mpsc` for work and `broadcast`/`watch` for live projections. It is not the durable queue, receipt ledger, or idempotency authority. | `cargo test -p fleet-stream`; assert bounded send backpressure and receiver lag behavior | **D:** [Tokio release](https://github.com/tokio-rs/tokio/releases/tag/tokio-1.53.1), [sync module](https://docs.rs/tokio/1.53.1/tokio/sync/index.html). **L:** local manifests/lock resolve `tokio v1.53.1`; the focused stream test command was started but its final result was not observed because another cargo build was already active. |
| `redb` | `redb` `4.2.0`, tag commit [`23b6ba05473b`](https://github.com/cberner/redb/commit/23b6ba05473b13e69ed4db82f4b5bc07f0c33be9), released 2026-08-17 | MIT OR Apache-2.0 | **Keep only for the existing embedded KV use.** Do not create a second durable effects/outbox authority beside SQLite; that would split backup, retention, and reconciliation. | Existing `fleet-store` KV tests; no new broker smoke | **D:** [redb design/durability](https://github.com/cberner/redb/blob/master/docs/design.md). **L:** local `fleet-store` resolves `redb v2.6.3`; no new redb code was added. |
| Desktop notification library | `notify-rust` `4.18.0`, tag commit [`975d65116fba`](https://github.com/hoodie/notify-rust/commit/975d65116fbab8e772f2b5c9bd5eedb73fc02a9e), released 2026-06-16 | MIT OR Apache-2.0 | **Optional adapter, not core.** Prefer explicit transport capability probing and durable outbox rows. On macOS its feature set is smaller than Linux/XDG; do not claim delivery without an OS acknowledgement. | Linux: `notify-send --version`; Rust adapter smoke with a test bus. macOS: an app-bundle `UNUserNotificationCenter` test, not a CLI-only claim | **D:** [notify-rust source/limitations](https://github.com/hoodie/notify-rust), [Freedesktop notification spec 1.3](https://specifications.freedesktop.org/notification/latest-single/). **U:** not a local dependency; no notification side effect was sent. |
| Linux filesystem sandbox | Landlock crate `0.4.7`, peeled tag commit [`62fec0f1521e`](https://github.com/landlock-lsm/rust-landlock/commit/62fec0f1521e4ab7f697752c1f324b725fe643d5), released 2026-07-27; kernel feature since Linux 5.13 | MIT OR Apache-2.0; kernel interface documented under GPLv2 project terms | **Adopt as optional Linux hardening with a runtime ABI probe.** A required policy must fail closed if requested rights cannot be enforced; best-effort mode must be explicit and recorded. | Linux-only: `landlock` example or a small `Ruleset::default().create().restrict_self()` probe; record kernel ABI and handled rights | **D:** [kernel Landlock API](https://docs.kernel.org/userspace-api/landlock.html), [crate changelog](https://github.com/landlock-lsm/rust-landlock/blob/main/CHANGELOG.md). **U:** this macOS host cannot verify kernel enforcement. |
| Linux namespace sandbox | bubblewrap `0.12.0`, peeled tag commit [`2a76602a8c71`](https://github.com/containers/bubblewrap/commit/2a76602a8c71f36c1527cf9fc3417d9149822e0c), released 2026-08-26 | LGPL-2.1-or-later | **Optional external adapter only; never a hidden requirement.** Pin at least `0.12.0`: upstream documents a symlink-setup vulnerability fixed there. The caller owns the filesystem/network policy; bubblewrap is not a complete policy. Do not use setuid mode. | Linux-only: `bwrap --version`; then a read-only bind plus denied-write test | **D:** [bubblewrap model/limitations](https://github.com/containers/bubblewrap), [0.12.0 security fix](https://github.com/containers/bubblewrap/security/advisories/GHSA-pxhw-h44j-8pfx). **U:** not installed on this macOS host. |
| POSIX process limits | `nix` `0.31.3`, tag commit [`b5933ca17880`](https://github.com/nix-rust/nix/commit/b5933ca178802b558a667514f717a86b3a1cedcc), changelog date 2026-05-11 | MIT | **Keep the existing local `nix 0.29.0` lane for this migration; evaluate upgrade independently.** Use `RLIMIT_AS`, process groups, signals, and one monotonic deadline. Limits are parent-owned and stamped in receipts. | `cargo test -p fleet-worker timeout_kill`; Linux/macOS process-group smoke with a child that exceeds the deadline | **D:** [nix 0.31.3 changelog](https://github.com/nix-rust/nix/blob/master/CHANGELOG.md), [nix source](https://github.com/nix-rust/nix). **L:** local manifest resolves `nix v0.29.0`; source/blueprint documents `setrlimit` and group termination. Focused test result remained unobserved during concurrent cargo builds. |
| macOS sandbox/permission surface | macOS `26.6.1`, build `25G76`, host date 2026-09-11; `/usr/bin/sandbox-exec` is an Apple-provided tool, not a vendored repo | Apple platform terms; no reusable project license | **Do not make `sandbox-exec` the product contract.** For a future signed GUI, use App Sandbox entitlements and explicit user grants. For the current local CLI, use path containment, no-ambient-credential rules, process limits, and explicit capability probes. | `sandbox-exec -p '(version 1) (allow process-exec)' /usr/bin/true` | **D:** [Apple App Sandbox](https://developer.apple.com/documentation/security/app_sandbox), [Apple notification authorization](https://developer.apple.com/documentation/usernotifications/asking-permission-to-use-notifications). **L:** `sw_vers` reported `26.6.1`; the harmless sandbox profile command exited 0. User notification authorization was not requested. |

## Node contracts

### `integrate` and `rollback`

**Authority:** the parent Fleet process. A worker may write only inside its owned worktree and may
return a proposal/receipt over fd 3; it must not merge, rewrite the parent branch, or receive ledger,
state-directory, or socket paths in its environment.

**Safe sequence:**

1. Resolve and record the real Git worktree root and base commit with `git rev-parse`; do not infer
   the worktree root from the Fleet project subdirectory.
2. Create a uniquely named worktree under `<git-root>/.worktrees/<run>` using `git worktree add --lock`.
3. Before integration, re-read parent `HEAD`, worktree `HEAD`, `git status --porcelain=v2`, and the
   expected diff hash. Refuse if the parent moved, the worktree is dirty outside the declared write
   set, or the artifact hash changed.
4. Preflight with `git merge-tree --write-tree <parent> <lane>`; only then perform the parent-owned
   `git merge --no-ff` under the Fleet repository lock. A conflict is a typed refusal, not a retry loop.
5. Record merge commit, parent-before, lane commit, diff hash, actor, and receipt sequence. Remove
   the worktree only after the receipt is durable; cleanup is containment-checked and idempotent.

`git2` may be considered later for read-only diff/object inspection, but it must not become a second
merge/worktree implementation. The upstream worktree docs explicitly describe shared refs, private
per-worktree `HEAD`/index state, branch-already-checked-out refusal, locking, and repair; those are
the semantics Fleet needs to preserve.

**Rollback:**

- Before merge: remove only an owned worktree under the repository’s `.worktrees` directory and mark
  the artifact `rolled_back` once. A second rollback is a refusal, not success.
- After merge: use a new, human-approved `git revert --no-edit <merge-commit>` proposal. Never use
  `reset --hard` against a shared/user branch as an automated rollback.
- Every refusal writes a receipt. Never report rollback success solely because a path disappeared.

**Documented vs local:** the safety sequence is a migration decision based on Git’s upstream
worktree/merge contracts (**D**). Local evidence is only version/list/preflight (`git version`,
`git worktree list --porcelain`, and `git merge-tree --write-tree HEAD HEAD`, all **L**). This
checkout’s `git worktree list` reports the Git worktree root as `/Users/rachitsrivastava/youtube/Principal
Engineering/Light`, while the assigned Fleet project is its `fleet/` subdirectory; that topology must
be resolved at runtime. A clean end-to-end lane merge against a fresh temporary repository is **U**.

### `approval`

Do not add a provider or UI to the kernel. Use the existing typed `HumanApproval` seam and persist an
approval record with:

```text
approval_id, task_id, scope_hash, base_commit, artifact_id, actor, reason,
issued_at, expires_at, decision, approval_receipt_seq
```

Invariants:

- only the parent/operator boundary can mint the token;
- `scope_hash` and `base_commit` must equal the proposal being accepted;
- expired, revoked, duplicate, or scope-mismatched approvals refuse;
- a restart can reload `pending`/`approved` records, but may not infer approval from a worker result;
- approval is a capability with a narrow action and expiry, not a boolean on the task row.

**Smoke:** create an approval for a fixed task/scope hash, restart, reload, accept once, then replay
the same approval and assert a typed duplicate/refusal. This is **U** as an end-to-end CLI smoke;
the lifecycle contract and compile-fail approval seam are **D/local repository evidence**, not proof
that the future persistence bridge is complete.

### `notify`

Use a durable outbox effect such as:

```text
effect_id, task_id, transport, idempotency_key, payload_hash, status,
attempts, not_before, lease_owner, lease_until, delivered_at, last_error
```

Transport policy:

- `terminal-jsonl`: always available, deterministic, and included in the receipt stream;
- macOS: an app-bundle `UNUserNotificationCenter` adapter must request/check authorization and treat
  denial as a recorded capability result; `osascript` can be a development convenience only;
- Linux: `org.freedesktop.Notifications` through `notify-rust` or a direct D-Bus adapter, after
  probing `GetCapabilities`; lack of a session bus is `unsupported`, not workflow failure;
- webhook/email/Slack are out of the base migration: they add credentials, remote unknown outcomes,
  and provider reconciliation work without being required by the local product.

Notifications are at-least-once projections. A provider timeout yields `unknown`; retry only after
readback or a transport-specific idempotency guarantee. No desktop popup is evidence that a task
integrated successfully.

### `broker` and outbox/idempotency

Use two deliberately separate layers:

```text
SQLite transaction: domain state + outbox row  ->  bounded Tokio channel ->  adapter lease
       durable authority                               wake-up only       ->  ack/reconcile
```

Minimum invariants:

- `idempotency_key` is unique and deterministic from `(task_id, effect_kind, scope_hash)`;
- the domain mutation and outbox insert commit atomically;
- a worker claims with a lease, never deletes the row before acknowledgement;
- `sent` is written only after provider acknowledgement; timeout is `unknown`, not `failed`;
- an expired lease is reclaimable; a live lease is not stolen without a recorded override;
- retries use bounded exponential backoff and an integer attempt count;
- bounded in-process queues apply backpressure; no unbounded `mpsc` or `Vec` is an admission policy;
- `checked == total` is required for reconciliation/gate verdicts, and `total == 0` fails.

SQLite WAL is suitable for this single-host product, but upstream documents that WAL requires all
processes on the same host and introduces `-wal`/`-shm` files. The state directory must therefore
stay local and the backup path must understand WAL; do not copy only the main `.sqlite` file while a
writer is live.

### Multi-day persistence, backup, and retention

Persist the following before a process exits successfully: lifecycle state, transition receipt,
outbox effects, leases, approval records, resource reservations, per-sink cursors, and reconciliation
watermarks. A restarted process reconstructs from these records; in-memory channels, locks, and timers
are disposable.

Backup protocol:

1. Use SQLite Online Backup API (or `VACUUM INTO` when a compact vacuumed snapshot is specifically
   wanted), never a raw copy of a live WAL database.
2. Write to `<backup>.partial`, finish the snapshot, run `PRAGMA integrity_check`, verify the
   ledger/hash chain and a manifest checksum, fsync the file and containing directory, then atomically
   rename to the dated snapshot name.
3. Record `{source_db_hash, backup_hash, checked, total, sqlite_version, created_at}` in a receipt.
4. Retain **7 daily + 4 weekly + 3 last-known-good** verified snapshots by default, subject to a
   configured byte quota and minimum-free-space floor. Never delete the only verified snapshot.
5. Do not prune the append-only hash-chained receipt ledger in place. Rotate immutable segments and
   retain a manifest linking the segments; verify the whole retained chain before deleting an old
   segment.

The retention numbers are a Fleet policy proposal, not an upstream guarantee. They must be config,
visible in `fleet doctor`, and refused when absent/ambiguous rather than interpreted as unlimited.

**Local evidence:** `sqlite3 :memory: 'select sqlite_version();'` returned `3.51.0`; the same smoke
showed `journal_mode=wal` is not meaningful for an in-memory database. The online-backup adapter,
file-backed WAL backup, restore, and retention deletion were **U**. Existing repository documentation
and `fleet-store` tests provide ledger/process-restart evidence, but not backup/restore proof.

### Resource limits and grants/capabilities

Each worker lease must carry explicit integer limits; absence is refusal, never zero/unlimited:

```text
wall_ms, cpu_ms (if enforceable), memory_bytes, output_bytes, fd3_bytes,
open_files, concurrency_slot, network_mode, read_roots, write_roots, expires_at
```

Parent enforcement:

- one absolute monotonic deadline for the whole child operation;
- process group/session isolation and group kill on timeout or cancellation;
- `RLIMIT_AS`/memory and file-descriptor caps where the host supports them;
- bounded fd-3 packet/output size; the current worker contract documents a 65,536-byte receive
  ceiling, so larger responses must become an explicit `packet-too-large` result rather than a JSON
  parse/environment failure;
- `available_parallelism` is an upper bound, not a reason to admit work beyond the persisted budget;
- all grants are logged as capability snapshots with tool path, version, resolved model (when relevant),
  and expiry.

Application capabilities remain a typed allowlist: role/agent × skill × operation. Unknown or missing
capabilities refuse before spawning. OS hardening is additive: Landlock/bubblewrap may restrict
filesystem/network visibility, but neither replaces the parent’s write-set containment or the no-
ambient-credential rule.

## Reconciliation algorithm

Run the reconciler at startup, after a crash/timeout, before retrying an `unknown` effect, periodically
while a run is active, and on explicit operator request:

1. Load every non-terminal effect and every live/expired lease from SQLite.
2. Compare each effect with Fleet receipts, provider readback (when the transport supports it), and
   the target repository/state hash.
3. Classify exactly one result: `acked`, `not_seen`, `failed`, `unknown`, `conflict`, or `expired`.
4. `acked` advances the outbox and receipt once; `not_seen` may retry with the same idempotency key;
   `unknown` blocks automatic retry until the adapter’s readback rule resolves it; `conflict` and
   `expired` require an operator decision.
5. Persist the classification and publish `{checked,total}`. `checked=0` is a failed reconciliation,
   even when there are no rows, because a zero-input check proves nothing.

The existing stream design’s per-sink cursor and retry/resume shape is the correct local precedent:
each sink advances its cursor only after durable delivery, and a restart resumes from the cursor rather
than replaying or skipping blindly. The new outbox should reuse that invariant without making each
transport invent its own ledger.

## Gaps to carry into the next blueprint

- **U:** end-to-end parent-owned merge, post-merge revert, approval restart/replay, SQLite online
  backup/restore, retention deletion, Linux Landlock/bubblewrap, and real desktop notification acks.
- **Known local skew:** upstream current candidates are newer than the checkout’s `rusqlite 0.32.1`,
  `redb 2.6.3`, `nix 0.29.0`; migration should not silently upgrade them.
- **No provider proof:** no remote forge, email, webhook, notification daemon, or external broker was
  used in this research run.
- **Test command status:** focused cargo tests were invoked, but another cargo workspace/build process
  was already active and the final exit code was not observed. Do not call those tests green.

## Primary sources

- [Git worktree](https://git-scm.com/docs/git-worktree), [Git merge](https://git-scm.com/docs/git-merge), [Git v2.51.0 source](https://github.com/git/git/tree/v2.51.0)
- [git2-rs](https://github.com/rust-lang/git2-rs), [libgit2](https://github.com/libgit2/libgit2)
- [SQLite transactions](https://sqlite.org/lang_transaction.html), [WAL](https://sqlite.org/wal.html), [Online Backup API](https://sqlite.org/backup.html), [VACUUM](https://sqlite.org/lang_vacuum.html)
- [rusqlite](https://github.com/rusqlite/rusqlite), [Tokio](https://github.com/tokio-rs/tokio)
- [Freedesktop Desktop Notifications 1.3](https://specifications.freedesktop.org/notification/latest-single/), [notify-rust](https://github.com/hoodie/notify-rust)
- [Apple App Sandbox](https://developer.apple.com/documentation/security/app_sandbox), [Apple notification authorization](https://developer.apple.com/documentation/usernotifications/asking-permission-to-use-notifications)
- [Linux Landlock API](https://docs.kernel.org/userspace-api/landlock.html), [Rust Landlock](https://github.com/landlock-lsm/rust-landlock)
- [bubblewrap](https://github.com/containers/bubblewrap), [bubblewrap 0.12.0 advisory](https://github.com/containers/bubblewrap/security/advisories/GHSA-pxhw-h44j-8pfx)
- [nix](https://github.com/nix-rust/nix), [redb durability design](https://github.com/cberner/redb/blob/master/docs/design.md)
