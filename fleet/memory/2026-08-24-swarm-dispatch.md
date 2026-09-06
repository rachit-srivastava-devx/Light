# Swarm dispatch debug report

- Symptom: `fleet swarm status` left all five agents at `Intake`, and all scorecards had zero denominators.
- Root cause: the CLI exposed status, bandwidth, allocation, and completion, but had no `swarm dispatch` route connecting worker execution to per-agent lifecycle and scorecard persistence.
- Fix: dispatch now runs the builder and independent verifier, applies the existing role gates, projects each role through its own typed task, persists distinct milestones, and records credited/faulted/unknown outcomes with denominators.
- Evidence: the end-to-end stub dispatch reported `checked=5 total=5`; status showed `Specified`, `Reviewed`, `Built`, `Verified`, and `Observed`; all five scorecards reported `credited=1 checked=1 total=1`.
- Regression tests: `swarm::tests::{verified_dispatch_advances_distinct_agent_states_and_scorecards,lead_code_gate_refuses_code_and_accepts_no_code,verifier_model_gate_refuses_same_and_accepts_distinct}` and `agent::tests::unknown_is_published_but_never_credited`.
- Verification: full Rust tests passed; clippy passed with warnings denied; P0 acceptance passed 34/34 after building its independent hash oracle.
- Status: DONE.
