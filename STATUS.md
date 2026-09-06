# Light/ build status

_Regenerated 2026-09-07 03:06 IST from `status/*.status` — do not hand-edit this file; edit your feature's own status file and re-run `bash status/render.sh`._

| # | Feature | Repo | Status | Agent | Since | Note |
|---|---|---|---|---|---|---|
| F01 | Orb build-mode toggle | orb | done | verifier (independent) | 2026-09-07 02:25 IST | client-side only, apps/macos cannot reach it (Swift can't import TS) - gap closed by redefined F04, server-side |
| F02 | lld.v1 contract (schema+TS+Rust+Python mirrors) | fleet+orb | contract | lead-architect (F02) - contract done, BLOCKED on lane | 2026-09-07 01:50 IST | needs both orb+fleet, both occupied by F01/F08; found 4 real defects in copied U1 artifacts + corrected 2 FEATURES.md errors (C1 verdict, Py-mirror repo). Start F02a in fleet once free. |
| F03 | Module-brief atomizer | orb | pending | unassigned | 2026-09-07 01:19 IST |  |
| F04 | Server-side build-mode session + depth-completeness belief registers (relay-py) - closes F01 reachability gap | orb | building | mid-engineer (building) | 2026-09-07 02:41 IST | contract excellent - found mode=build crashes (500), apps/macos dials nothing yet (F04b follow-up flagged) |
| F05 | Freeze protocol (propose->pushback->freeze) | orb | pending | unassigned | 2026-09-07 01:19 IST |  |
| F06 | lld-ready gate (depth-bar enforcement) | fleet | pending | unassigned | 2026-09-07 01:19 IST |  |
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
