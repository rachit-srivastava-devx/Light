# Light/ build status

_Regenerated 2026-09-07 06:18 IST from `status/*.status` — do not hand-edit this file; edit your feature's own status file and re-run `bash status/render.sh`._

| # | Feature | Repo | Status | Agent | Since | Note |
|---|---|---|---|---|---|---|
| F01 | Orb build-mode toggle | orb | done | verifier (independent) | 2026-09-07 02:25 IST | client-side only, apps/macos cannot reach it (Swift can't import TS) - gap closed by redefined F04, server-side |
| F02 | lld.v1 contract (schema+TS+Rust+Python mirrors) | fleet+orb | verified | verifier (independent, Sonnet 5, worktree Light/.worktrees/F02-verify, commit 6fd66f3) | 2026-09-07 05:10 IST | PASS. Re-derived fresh (orb npm install + relay-py venv, no undocumented steps); all 4 ladder commands matched exactly (lld-crosslang.sh "ok 3 mirrors agree on 6 fixtures" exit 0; cargo test f02_lld_crosslang 7/7; vitest lld-v1+LldReadyGate 18/18; pytest schema-conformance 9/9). Read the actual code (not just green tests) for all 6 claimed defect-fixes -- stamped_by, alternatives/killed_alternatives flat floor incl. LldReadyGate.ts + both hand-validators, sha256: prefix, numeric-leaf refusal at depth in all 3 canonicalizers, TS additionalProperties rejection, Rust valid_node_id's b.len()<3 -- all genuine. Reproduced the stamped_by mutation myself (flip->F02-T7 red + comparator exit 6 "MIRROR FAILED: ts", revert->clean, git diff empty). Independently re-ran fleet/verify.sh on this tree (14 passed/7 failed/22 stages) and on a disposable git-worktree at merge-base dafffe5 (15 passed/5 failed/21 stages); the only deltas are F02's own new green lld-crosslang stage and the F08 tests/f08_pr_emit.rs race intermittently tripping unit-tests/coverage (reproduced the flip directly, 3 reruns, no F02 file involved) -- fmt/clippy/gitleaks/semgrep/corpus failure sets confirmed byte-identical on both trees by direct content diff, zero touch F02 files. Re-executed 2+ builder-claimed commands independently (crew pytest 22/22 both trees; gitleaks 2 findings/17 commits both trees) plus extra (orb full vitest 750/750 + 2 pre-existing @pe/* collect failures, full relay-py pytest 430/430, tsc apps/mobile clean). Adversarial CLI testing (fleet contract lld validate): nonexistent file, empty file, malformed JSON, empty object, bare array, missing args -- all clean typed exit codes (0/3/7/8), no crashes, deterministic across repeat runs. |
| F03 | Module-brief atomizer | orb | verified | verifier (independent) | 2026-09-07 06:17 IST | Independently re-derived: fresh venv, full suite matched builder's claim exactly (3 failed/441 passed) before fix, F03's 14/14 acceptance suite matched, real diff confirms blind-repair fix uses validate_module_brief_detailed and BUILD branch calls decompose() never atomize(). Reproduced M2 and M7 mutations myself with matching failure text. Fixed the disclosed F04 conflict (root cause confirmed genuinely F03 contract 2.4, not a builder bug): test_f04_build_mode_wire.py's a1_2/a7_1/a7_2 updated to find-by-content instead of assume-last-call and to cover decompose's mandatory extra call, without weakening the byte-match or veto assertions - commit 1c1e8d8 on lane/F03-atomizer. Full suite now 444 passed/0 failed, confirmed on 2 fresh runs plus 3 repeats of the fixed file alone. Live 2-process E2E drive + adversarial battery (empty text/missing field/bad mode/malformed JSON all clean 422s, SIGKILL mid-op + restart recovers persisted state, duplicate identical requests don't crash) - no server-side errors logged. Not merged (touches proxy/schemas.py, human-merge A15/D4) - ship to PR. |
| F04 | Server-side build-mode session + depth-completeness belief registers (relay-py) - closes F01 reachability gap | orb | verified | verifier (independent) | 2026-09-07 03:22 IST | Independently re-derived in a separate worktree: fresh venv+423 passed x2 (no flake), belief math hand-verified against the code (kappa=0.4, lambda=0.6, tier2 clamp 0.8, no wall-clock decay), untouched-list diff clean, M6 mutation reproduced myself (breaks A1.2 not A1.3, builder's self-disclosed discrepancy confirmed accurate), manual E2E driven on 2 self-started live processes incl. self-triggered kill+restart persistence and live tenant-isolation probe; no discrepancies found - PASS |
| F05 | Freeze protocol (propose->pushback->freeze) | orb | pending | unassigned | 2026-09-07 01:19 IST |  |
| F06 | lld-ready gate (depth-bar enforcement) | fleet | verifying | mid-engineer (dispatched) | 2026-09-07 06:16 IST | Built and self-verified. fleet::lld_ready (new) ships GATE_CHECK_IDS/GateRefs/Verdict/evaluate/evaluate_with/depth_evidence/check_and_evaluate/STAMPED_BY; lld.rs's valid_node_id/number_calc_ok/structural_enforced_by_ok made pub(crate), zero logic change (lld-crosslang.sh still ok 3/6). fleet gate lld-ready <file> |
| F07 | Fleet SOW-intake extension | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F08 | PR emit step | fleet | verified | verifier | 2026-09-07T03:05:45+05:30 | Re-derived everything + real CLI/repo/PR e2e drive; fixed the diagnosed git-init race (0/16 after); a separate unique_dir() collision still flakes cargo test -p fleet at default parallelism, see FLEET-LEARNINGS.md |
| F09 | Orb->Fleet handoff wiring | orb+fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F10 | P0 capstone: one module spoken->frozen->built->attested->PR | orb+fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F11 | Thin status echo | orb | pending | unassigned | 2026-09-07 01:19 IST |  |
| F12 | L1 working memory: freeze ledger as session object | orb | pending | unassigned | 2026-09-07 01:19 IST |  |
| F13 | Model routing wired into actual run | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F14 | Verifier!=builder enforcement confirmed airtight | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F15 | Mutation adequacy non-optional on lane gate | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F16 | Review narration surface | orb-or-apps-macos | pending | unassigned | 2026-09-07 01:19 IST |  |
| F17 | Depth-bar tightening (eval/threshold checks) | orb+fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F18 | Streaming STT partials | orb | pending | unassigned | 2026-09-07 01:19 IST |  |
| F19 | Streaming TTS first-audio | orb | pending | unassigned | 2026-09-07 01:19 IST |  |
| F20 | stream-live gate | orb | pending | unassigned | 2026-09-07 01:19 IST |  |
| F21 | design-graph.v1 + lane-status.v1 push transport | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F22 | Live graph render (embedded WKWebView pane) | apps/macos | pending | unassigned | 2026-09-07 01:19 IST |  |
| F23 | Barge-in during design dialogue | orb | pending | unassigned | 2026-09-07 01:19 IST |  |
| F24 | Real concurrent worktree lanes | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F25 | Registry install/extract/build-new gate | fleet+registry | pending | unassigned | 2026-09-07 01:19 IST |  |
| F26 | Knowledge-map-before-build | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F27 | planner role (D8) | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F28 | Extract voice-io service | registry | pending | unassigned | 2026-09-07 01:19 IST |  |
| F29 | Extract dialogue-engine feature | registry | pending | unassigned | 2026-09-07 01:19 IST |  |
| F30 | Extract keel-kernel service | registry | pending | unassigned | 2026-09-07 01:19 IST |  |
| F31 | Build delivery-lane feature | registry | pending | unassigned | 2026-09-07 01:19 IST |  |
| F32 | worker-payload.v1 schema | registry+fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F33 | D10 trigger mechanism | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F34 | no-ambient injection gate | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F35 | Contract-codegen pipeline | apps/macos+fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F36 | AEC/VPIO wiring | apps/macos | blocked | verifier | 2026-09-07 02:43 IST | FAIL: build/68 tests reproduce clean, but VoiceProcessingIOManager.start() throws propertySetFailed(-10863) on real hardware (never calls AudioUnitInitialize) and its render callback is an unconditional no-op that can never invoke onProcessedFrame/onFrame even if start succeeded — AEC path delivers zero frames, not an honest CI-only gap as claimed. |
| F37 | Two-pane chat UI shell | apps/macos | pending | unassigned | 2026-09-07 01:19 IST |  |
| F38 | L3 procedural memory / learning loop | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F39 | Retro loop | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
| F40 | Local cost dashboard (D12) | apps/macos | pending | unassigned | 2026-09-07 01:19 IST |  |
| F41 | Multi-harness dispatch (generalize claude+codex adapter pair) | fleet | pending | unassigned | 2026-09-07 01:25 IST | owner's explicit standing rule, found via speed-of-thought-fleet-rules skill; upstream backlog S5 |
| S0 | Baseline gate check (each repo's own pre-existing verify gate, unchanged) | orb+fleet+apps/macos | building | main session | 2026-09-07 01:31 IST | illegal_lifecycle_transitions_do_not_compile: CONFIRMED GREEN (1 passed, 165s, isolated); full cargo test running now |
| S1 | Step 1 — copy adhd-focus-orb+fleet-rs+OrbMac into Light/, reorganize | Light (all) | done | main session | 2026-09-07 01:17 IST | 1545 files, single fresh git history, commit 8f86440 |
| S2 | Step 2 — write Light/FEATURES.md (atomic list, ordered by time-to-visible-output) | Light (all) | done | main session | 2026-09-07 01:23 IST | 40 features, P0-P4, committed |
