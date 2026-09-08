# VERIFY-P0 — independent verification report

Verifier: independent agent, no access to builder transcripts. Fresh `treehouse` worktree at
commit `eb67664` (`lead: 9/9 gate wall, 26/26 acceptance — human surface routed, all modules
reachable`), on macOS/arm64, cargo 1.94.1 / rustc 1.94.1 (host toolchain reports
`x86_64-apple-darwin` — this machine's whole Rust toolchain runs under Rosetta; unrelated to
fleet, noted for the record only).

**Verdict: FAIL.** The three numeric claims (`verify.sh` 9/9, `p0.sh` 26/26, corpus 0 caught) are
each literally true once the environment is prepared correctly — I reproduced all three. The
"usable by a human" claim is false: the documented one-command install crashes on every fresh
machine, and outside the ~6 specific happy paths the acceptance suite checks, nearly every error
and refusal path in the CLI prints zero bytes. The corpus's "0 caught" also has a broken
denominator: two "clean" detectors cannot ever fire on the pattern they claim to catch, and a
third is dead code that never runs its own check.

---

## 1. README-only reproduction — FAIL ("undocumented setup" / broken documented setup)

Ran `./install.sh` exactly as README.md's "Install" section instructs, in the fresh worktree,
nothing pre-existing in `~/.local/bin` or `~/.local/state/fleet`:

```
$ ./install.sh
fleet install target: macOS (arm64)
OK prerequisite: cargo --version => cargo 1.94.1 (29ea6fb6a 2026-03-24)
OK prerequisite: rustc --version => rustc 1.94.1 (e408947bf 2026-03-25)
OK prerequisite: git --version => git version 2.51.0
CHANGE build: cargo build --release --manifest-path .../keel/Cargo.toml
   ... (normal cargo output) ...
    Finished `release` profile [optimized] target(s) in 1m 05s
CHANGE installed: /Users/rachitsrivastava/.local/bin/fleet (mode 755)
CHANGE ensured mode 700: /Users/rachitsrivastava/.local/state/fleet
CHANGE created: .../runs, .../artifacts, .../attestations, .../ledger  (mode 700 each)
PATH already contains /Users/rachitsrivastava/.local/bin
CHANGE created: /Users/rachitsrivastava/.zfunc
fleet: unknown command. Try `fleet --help`.
EXIT_CODE=7
```

The install **never prints its own "DONE fleet installed" line** and exits 7. Root cause:
`install.sh` line 127 unconditionally runs `"$BIN_PATH" completions zsh > "$ZFUNC_FILE"` as the
last step of the plain install path, and README.md line 39 documents `fleet completions
zsh|bash|fish` as a real command. Neither exists:

```
$ fleet completions zsh
fleet: unknown command. Try `fleet --help`.       # exit 7
$ grep -rn completion keel --include='*.rs'
(zero matches — no such subcommand anywhere in the source)
$ fleet --help | grep -i complet
(zero matches — not in the real command list either)
```

So the documented, one-command install (`./install.sh`, no flags) **cannot complete
successfully on a fresh checkout, ever** — it is not a flake, it is deterministic. A first-time
user following the README verbatim sees a crash and an "unknown command" error with no
indication the binary was in fact already installed underneath. (It was — I verified the rest
of the README's "60-second quickstart" works once you skip the broken completions step; see
§3(g)/§5.) `./install.sh --uninstall` and `--check` both work correctly. Cleaned up afterward:
removed `~/.local/bin/fleet`, `~/.local/state/fleet`, `~/.zfunc` from the real `$HOME` — machine
restored to its pre-test state.

## 2. Test ladder — real output

### 2a. `tests/acceptance/p0.sh`, three-plus runs

**Run 1** (fresh worktree, `cargo build` done, `tests/tools/b3oracle` **not yet built** —
this is the true from-scratch state):

```
== P0 acceptance ==
  ok   A1 fleet run exits 0
  ok   A2 run prints an artifact id
  ok   B1 artifact exists in the store
  ok   B2 artifact mode is 0444
  ok   B3 artifact is not writable
  FAIL B4 artifact id == its content hash   ORACLE MISSING - build tests/tools/b3oracle (this is a FAILURE, not a skip)
  ok   C1..C4, D1, E1..E4, F1..F2, G1..G3, H (all 6)
== 25 passed, 1 failed ==   (exit 1)
```

This is a real gap worth naming precisely: **run `tests/acceptance/p0.sh` in isolation on a
virgin checkout and you get 25/26, not 26/26**, because the independent oracle crate has its own
build step that nothing but `verify.sh`'s "acceptance builds" stage triggers. The failure mode
itself is correct and intentional (B4 fails loudly instead of vacuously skipping — see §3(e)) —
but the literal claim "`tests/acceptance/p0.sh` is 26/26" is only true after that separate build,
which is undocumented as a prerequisite for running the acceptance script standalone.

After `cargo build --manifest-path tests/tools/b3oracle/Cargo.toml`:

**Runs 2–5**, back to back, identical:
```
  ok   B4 artifact id == its content hash (independent oracle)
  ...
== 26 passed, 0 failed ==   (exit 0)
```
All four post-oracle runs: **26/26, exit 0.** E2/E3/E4 (20-way concurrent ledger append) held on
every run. One cosmetic wart, present on every run: `printf 'x' >> "$APATH" 2>/dev/null` at line
34 still leaks a shell-level `Permission denied` message to the terminal before the redirect
takes effect (bash resolves the `>>` before the `2>/dev/null`) — harmless noise, not a test
failure, not a product bug.

### 2b. `tests/corpus/run.sh`

```
DENOMINATOR checked=25 total=25 excluded=70 caught=0
```
Exit 0. Matches the claim exactly (95 scripts total = 25 checked + 70 excluded). See §3(e) for
why "0 caught" is not the same as "clean."

### 2c. `./verify.sh`

```
== fleet verify ==
  .... fmt                         ok   fmt
  .... clippy -D warn              ok   clippy -D warn
  .... unit tests                  ok   unit tests
  .... acceptance builds           ok   acceptance builds
  .... cargo-deny                  ok   cargo-deny
  .... cargo-audit                 ok   cargo-audit
  .... secrets                     ok   secrets
  .... acceptance                  ok   acceptance
  .... corpus                      ok   corpus
-- 9 passed, 0 failed, 0 skipped (denominator: 9 stages) --
```
Exit 0. **All 9 stages actually ran** — I confirmed `cargo fmt`, `cargo clippy`, `cargo-deny`,
`cargo-audit`, and `gitleaks` are all genuinely installed on this machine, so this 9/9 reflects
real execution, not `stage()`'s skip-on-missing-tool path silently inflating the count (see
§3(e) — that path exists and I tested it separately).

`cargo test --manifest-path keel/Cargo.toml` directly: 9 unit tests + 1 doc test, all pass, in
0.11s and 0.28s respectively. Test names are specific, not placeholders (e.g.
`bounded_count_rejects_invalid_denominator_and_overflow`,
`false_positive_control_has_zero_edges`, `traversal_is_rejected_and_in_lease_path_is_accepted`).

## 3. Attacks (a)–(h)

### (a) Contract tampering — PASS
`git log --all --oneline -- tests/acceptance/ contracts/` across every branch and the full
36-commit history: exactly **3 commits ever touched these paths, all authored by `lead`**
(`2ec202c` initial contract, `910e4e3` gate wall, `c1d41c4` the 6 human-surface assertions). No
`gates`/`corpus`/`adapters`/`oracle`/`evalh`/`agents`/`ci`/builder-slice commit touches either
path, on any of the 22 branches. Inspected the diff of `c1d41c4` directly: it is a pure addition
of 19 stricter lines (the H section) — nothing weakened or removed. `git diff HEAD~20 --stat --
tests/acceptance contracts` shows only that one addition. No tampering found.

### (b) Artifact immutability — PASS (matches documented threat model exactly)
Fresh run, then on the resulting artifact file (mode `444`):
```
$ echo x >> $ARTIFACT          # permission denied, exit 1
$ dd of=$ARTIFACT conv=notrunc # Permission denied, exit 1
$ chmod +w $ARTIFACT; echo x >> $ARTIFACT   # succeeds — file now has "corruption" appended
$ fleet attest verify $AID
                                # exit 8 (verification mismatch) — tamper IS detected
```
This is exactly what README.md's Limitations section promises ("tamper-evident, not
tamper-proof"). No OS-level immutable flag (`chflags uchg`) is used, only `chmod 444`, so any
process running as the same user can defeat it with `chmod +w` — but the re-verification step
catches the corruption every time. Behaves as documented, not as advertised-but-broken.

### (c) Independent hash verification — PASS, verified without trusting the binary or its own oracle
I did not trust `tests/tools/b3oracle` either, since it ships in the same repo. I wrote a
**separate, from-scratch Cargo project** (`my_own_b3_checker`, own `Cargo.toml`, own `main.rs`,
not copied from the repo) depending only on the standard upstream `blake3` crate, built fully
`--offline` from the already-cached registry:
```
AID (claimed by fleet)      = 0094d2237b98a12d0c45d811d904ae86d3b73f181c3f460e2f71cb297942bded
MY independent blake3 hash  = 0094d2237b98a12d0c45d811d904ae86d3b73f181c3f460e2f71cb297942bded
REPO's own oracle hash      = 0094d2237b98a12d0c45d811d904ae86d3b73f181c3f460e2f71cb297942bded
```
All three agree. `b3sum` CLI and Python's `blake3` module are not installed on this machine, so
those specific "unrelated tools" were not available — I built my own instead, which is the
protocol's documented fallback ("build your own checker against the real blake3 crate").

### (d) Ledger concurrency at higher load — PASS, exceeded the requested load
50 concurrent `fleet ledger append`, three independent runs, plus a 100-way run for headroom.
All three required assertions, every time:
```
run1: count=50  verify="verified checked=50 total=50" rc=0   distinct(prev_hash)=50 rows=50 EQUAL=True
run2: count=50  verify rc=0 checked=50 total=50               distinct=50 rows=50 equal=True
run3: count=50  verify rc=0 checked=50 total=50               distinct=50 rows=50 equal=True
100x: count=100 verify rc=0 checked=100 total=100              distinct=100 rows=100 equal=True
```
No lost rows, no `prev_hash` reuse, chain verifies clean at every scale tested.

### (e) Vacuous passes — PARTIAL FAIL (found real ones, inside a harness that itself resists the obvious attack)
`tests/corpus/run.sh`'s own anti-vacuity guard works: I copied the corpus directory, neutered
**all 95** detectors to unconditionally `exit 77`, and ran the unmodified harness against them:
```
DENOMINATOR checked=0 total=0 excluded=95 caught=0
```
real exit code (captured correctly, not through a pipe): **6**, not 0. Making every detector
"pass" does not make the suite pass — `[ "$checked" -gt 0 ] || exit 6` catches it. Same for
`p0.sh`'s B4: I confirmed on run 1 that a missing oracle is a **FAIL**, not a skip.

But static reading plus **positive-control testing** (inject the exact bad pattern each detector
claims to catch, in an isolated fake root, and check it actually fires) found real defects among
the 25 "mechanisable" detectors:

- **`T2.sh` is dead code.** Line 6 is an unconditional `exit 77`, before the python heuristic
  that actually checks for `shift 2`. I fed it a fixture containing `shift 2` directly — it
  still printed "NOT MECHANISABLE" and exited 77. Its own written detection logic can never run,
  under any tree contents.
- **`T20.sh` and `B10.sh` share one broken regex and can never fire on realistic input.** Both
  use `re.search(r'\$\?\b', s)` to catch "`$?` read after a pipe." In Python, `\b` needs a
  word/non-word transition; `$?` in real shell code is always followed by whitespace or
  punctuation (`echo $?`, `[ $? -eq 0 ]`, `; echo $?`), never a word character, so the boundary
  never fires:
  ```
  >>> re.search(r'\$\?\b', 'if [ $? -eq 0 ]; then echo ok; fi')   -> no match
  >>> re.search(r'\$\?\b', 'cmd1 | cmd2; echo $?')                -> no match
  >>> re.search(r'\$\?\b', 'x=$?y')                                -> MATCH (not realistic shell)
  ```
  I confirmed empirically: a fixture with `cmd1 | cmd2; echo $?` (exactly the failure both
  detectors describe: "pipeline status was read from the wrong command") gets exit 0 from both
  scripts, not the expected 1. Two separately-authored detectors (one filed under `T`, one under
  `B`) for the same historical failure, both non-functional the same way.
- **`C6.sh` has a narrower blind spot.** Its regex fires correctly on bare words
  (`generator == verifier`), Python (`if generator == verifier:`), and `assert` forms, but misses
  the far more common shell form `[ $gen == $ver ]` because the pattern requires `(ver|verifier)`
  immediately after the operator with no `$` in between. Partial coverage, not fully broken.
- 9 other mechanisable detectors I positive-control-tested (A1, A3, A4, A8-adjacent T6, C9, C22,
  C24, S9, T1) **did fire correctly** on injected bad fixtures and stayed quiet on clean
  negative-control fixtures.

Net effect: the printed line `caught=0` is arithmetically accurate, but at least 2 of the 25
"checked" detectors (T20, B10) are clean **only because they cannot detect their target pattern
at all**, not because the codebase doesn't contain it, and a third (T2) never executes its
mechanism. I did not exhaustively positive-control all 25; I tested 13 (52%) with paired
positive/negative fixtures.

### (f) Orphans — PASS
`pgrep -fl "keel/target"` and `pgrep -fl "\.local/bin/fleet"` return no matches at every
checkpoint (baseline, after the acceptance suite, after 50-/100-way concurrency, after the TUI
and MCP probes, after two kill-storms, at the end of the session). The unrelated
`.../fleet/.venv/.../phoenix serve` processes visible in a bare `pgrep fleet` are a different
project (Python, different path) and were excluded by the precise pattern.

### (g) The human surface — PARTIAL FAIL (the documented fix was narrow; the underlying defect is still pervasive)
`docs/DELTA.md` row D12, written by the lead, calls this "the single most important row" in the
build: a previous version had every user-facing command exit 7 with zero output, and the fix was
"6 human-surface assertions to the contract first." Those exact 6 checks now pass — `--help`,
`--version`, `doctor` (happy path), bare invocation, `ratchet show`, `console --help` all give
real, substantive output. I did not stop there. I broke `doctor` and swept the rest of the
surface:

```
$ FLEET_STATE=<unwritable-dir>/fleet fleet doctor     -> exit 3, 0 BYTES of output
$ FLEET_STATE=<a-regular-file>       fleet doctor     -> exit 3, 0 BYTES of output
$ fleet console </dev/null (no tty)                    -> exit 3, 0 BYTES of output
$ fleet mcp                                            -> exit 7, 0 BYTES of output
$ fleet mcp onlyonearg                                 -> exit 7, 0 BYTES of output
$ fleet impact                       (no --symbol)     -> exit 7, 0 BYTES of output
$ fleet impact --symbol ''           (empty value)     -> exit 7, 0 BYTES of output
$ fleet run                          (no args)         -> exit 7, 0 BYTES of output
$ fleet run --task '' --repo . --agent stub            -> exit 7, 0 BYTES of output
$ fleet attest verify                (no id)           -> exit 7, 0 BYTES of output
$ fleet attest verify garbage-not-a-hash                -> exit 8, 0 BYTES of output
$ fleet ledger append                (no event/body)   -> exit 7, 0 BYTES of output
$ fleet ledger count   (on an empty/absent ledger)      -> exit 6, 0 BYTES of output
$ fleet ledger verify  (on an empty/absent ledger)      -> exit 6, 0 BYTES of output
```
Re-verified two of these directly (not through command substitution) with `RUST_BACKTRACE=full`
to rule out output being swallowed by my harness: still exactly 0 bytes on stdout and stderr
combined. `doctor` and `ledger count/verify` also **disagree with each other** on the same
underlying condition: `doctor` explicitly labels an absent ledger "not an error" (prints
`ledger chain absent (no runs yet — not an error)`, exit 0), while `ledger count` and `ledger
verify` on that identical state exit **6** ("invariant violation") with nothing printed — the
suite itself works around this by hard-coding `"$FLEET" ledger count 2>/dev/null || echo 0`
rather than trusting the command's own output, which is a tell that the authors already knew
this path was unreliable.

By contrast, some refusal paths *are* good: `fleet totallybogus` prints `fleet: unknown command.
Try 'fleet --help'.`, and `fleet impact --symbol nonexistent_symbol_xyz` prints a genuinely
excellent actionable message (`project is not registered; run: fleet graph index --root ... --db
...`). So the CLI is capable of good error messages — it simply doesn't have them on most paths.

**Conclusion: `doctor` is capable of reporting UNHEALTHY (exit code correctly goes non-zero,
confirmed by deliberately pointing `FLEET_STATE` at an unwritable path and at a regular file) but
it does not *report* anything a human can read when it does.** The exact defect class the
project's own history names as its worst failure (green gates, silent CLI) is still present
throughout the refusal/fault paths of `run`, `attest`, `ledger`, `doctor`, `console`, `mcp`, and
`impact` — it was fixed only for the specific invocations the acceptance suite happens to check.

### (h) Reachability — PASS, with a minor documentation gap
`keel/fleet/src/`: `main.rs`, `lib.rs` (`pub mod agent;`), `console.rs`, `graph.rs`, `mcp.rs`,
`ratchet.rs`. All five non-main modules are wired into `main.rs`'s dispatch and reachable through
real, working commands: `agent::list()` ← `fleet agents list`; `ratchet::command()` ←
`fleet ratchet ...`; `console::run()` ← `fleet console`; `graph::graph_command()` /
`graph::impact_command()` ← `fleet graph` / `fleet impact`; `mcp::manifest_for_lease()` /
`mcp::serve()` ← `fleet mcp manifest` / `fleet mcp <repo> <lease>`. `grep -rn
"allow(dead_code)\|allow(unused" keel --include='*.rs'` returns nothing — no lint suppressions
hiding unreachable code — and `cargo clippy --all-targets -- -D warnings` passes clean, which is
meaningful given the absence of suppressions. Minor gap: `fleet graph` and `fleet mcp manifest`
are real and reachable but are not listed in the top-level `fleet --help` or in README.md's
command summary (discoverable only because `fleet impact`'s own error message points to `fleet
graph index`) — a documentation completeness gap, not dead code.

## 4. Cross-check against builder evidence — evidence path does not exist

`evidence/<task-id>/report.md` does not exist anywhere in this repository (checked from the repo
root, `find . -iname evidence*` and `find . -iname report.md` both empty). There is no
`evidence/` directory at all. The closest analogues are `docs/DELTA.md` (an append-only,
lead-owned honesty ledger) and the commit messages on `master`. I treated D12 in `docs/DELTA.md`
as the builder's strongest claim to cross-check — "fixed by adding 6 human-surface assertions...
every shipped module must be reachable... asserted, or it is dead code wearing a green build" —
and re-executed well beyond those 6 assertions (§3g above). The claim is accurate for exactly the
6 things it tested and not for the rest of the CLI's error surface. This is a genuine gap in the
delivered evidence trail (no `evidence/` directory was left for a verifier to check against) and
is reported as such rather than skipped silently.

## 5. Break-it round

- **Empty input**: `fleet run --task '' --repo <target> --agent stub` → exit 7, 0 bytes output,
  but `fleet ledger count` before/after confirms a receipt **was** written (matches `p0.sh` F2 —
  refusals are recorded even though they're silent to the terminal).
- **Wrong input**: `fleet attest verify garbage-not-a-hash` → exit 8, 0 bytes. `fleet
  totallybogus` → exit 7 with a real, clear message (inconsistent with the above — some paths
  explain themselves, most don't).
- **Run twice, identical input**: back-to-back identical `fleet run` calls against the same
  target produced *different* artifact ids at first — this is correctly explained, not a bug:
  the stub agent leaves its diff **uncommitted** in the target repo's working tree, so the second
  run starts from a different base (one prior stub line already present) and legitimately hashes
  to something else. Resetting the target (`git checkout -- ; git clean -fd`) between runs and
  repeating: **identical artifact id both times** — confirmed deterministic given identical
  starting state.
- **Restart mid-operation**: fired 20 immediate `kill -9` launches (all landed before any file
  write — ledger stayed completely absent, doctor still reported healthy) and a second round of
  30 staggered-delay `kill -9` launches that did land after the ledger append began: result was
  `ledger/chain.jsonl` with exactly 30 valid rows, `ledger verify` exit 0 ("verified checked=30
  total=30"), zero partial/`.tmp` files anywhere under `$FLEET_STATE` (only a legitimate
  zero-byte `chain.lock`), zero orphan processes, and a subsequent normal run succeeded and
  produced the expected deterministic hash. I could not force a kill during the artifact-freeze
  step specifically (the stub path is too fast), so I cannot claim that exact window is proven
  safe — only that ~50 kills across two rounds produced zero corruption of anything that did get
  touched.

## 6. Summary table

| # | Attack | Verdict |
|---|---|---|
| — | README-only install (`./install.sh`) | **FAIL** — crashes every time, phantom `completions` command |
| — | Test ladder reproduction | PASS (numbers match, once environment is prepared) |
| a | Contract tampering | PASS |
| b | Artifact immutability | PASS (matches documented threat model) |
| c | Independent hash verification | PASS |
| d | Ledger concurrency (50-/100-way) | PASS |
| e | Vacuous passes in the corpus | **PARTIAL FAIL** — 2 detectors provably cannot fire (T20, B10), 1 is dead code (T2) |
| f | Orphan processes | PASS |
| g | Human surface / doctor unhealthy reporting | **PARTIAL FAIL** — detects faults correctly, reports almost none of them |
| h | Module reachability | PASS (minor doc gap: `graph`, `mcp manifest` undocumented) |
| — | Cross-check builder evidence | **GAP** — no `evidence/` directory exists to cross-check |

## One-line verdict

**FAIL** — the cryptographic and concurrency core (hashing, tamper-evidence, ledger integrity
under 100-way concurrency, module reachability, contract integrity) is genuinely solid and I
could not break any of it, but the documented install procedure crashes on a fresh machine every
time, the corpus's "0 caught" hides at least two detectors that cannot ever catch what they claim
to, and "usable by a human" is true only for the ~6 exact invocations the acceptance suite
checks — every other error and refusal path in the CLI (the majority of what a real first-time
user will actually hit) prints nothing at all.
