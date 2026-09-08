# Anthropic-blog research vs. Speed-of-Thought blueprint, 2026-08-30

Background agent read Anthropic's engineering blog, Agent SDK docs, and Claude Code changelog
against `blueprints/Speed-of-Thought-L8-Deep-Dive/`. 5 findings, cited. `blueprints/` is off-limits
to edit directly (root `CLAUDE.md`: "NEVER touch... a governance/blueprints/ spec") — recorded here
for the blueprint owner to act on, with the two fleet-rs-side checks it flagged actually verified.

1. **Actionable, needs blueprint-owner review.** Claude Code's "Dynamic Workflows"
   (claude.com/blog/introducing-dynamic-workflows-in-claude-code, 2026-05-28) ships native
   worktree-isolated parallel subagent execution — up to ~1000 subagents, ~16 concurrent by
   default scaling with cores. Overlaps `05-PARALLELISM-AND-ISOLATION.md`'s plan to hand-build
   worktree-pool/cap-acquire logic in `swarm.rs` for the Claude-CLI half of dispatch; its default
   cap even matches the blueprint's own `min(16,cores-2)`. Does NOT cover the `codex`-CLI half, the
   SOW gate, typed lifecycle, or 9-element attestation — a partial, not full, replacement.
   **Recommend the blueprint owner decide, as a C1 reuse verdict**, whether the Claude-CLI half of
   `swarm dispatch` should sit on Dynamic Workflows rather than be reimplemented.
2. **Confirmatory, no action.** Agent SDK's `AgentDefinition` schema overlaps `07`'s
   `worker-payload.v1` shape but is in-process or SDK-driven, not subprocess-spawn across both
   `claude`+`codex` CLIs, and lacks the content-addressed skill-digest `07` needs for attestation.
   `07` already verified `[V]` on the CLI-flag injection path it actually needs.
3. **Checked against fleet-rs's own code — no gap.** Anthropic's "early victory problem" (verifier
   subagents reporting partial test runs as passing) is already mitigated structurally here:
   `tests/corpus/MANIFEST.sha256` hash-pins the exact detector file set (`bin/detector-integrity.sh`
   grepped, confirmed), `checked/total` denominators are published throughout `verify.sh`, and
   `policy/measured_nothing.rego` hard-refuses any verdict with `checked==0`. Stronger than an
   instruction to "run the complete suite" — it's mechanical, not a prompt.
4. **Checked against fleet-rs's own code — no gap, today.** "How we contain Claude"
   (anthropic.com/engineering/how-we-contain-claude, 2026-05-25) warns an allowlisted destination
   can become an exfiltration channel. Grepped `keel/fleet/src/mcp.rs` directly: zero
   http/fetch/network code of any kind. `fleet mcp` exposes only `impact`/`lesson_recall` — no
   outbound network capability exists to be misused. Re-check this the moment `fleet mcp` gains any
   network-touching tool.
5. **Confirmatory, no action.** `09-TOKENOMICS-AND-COST.md`'s pricing table matches
   platform.claude.com/docs/en/about-claude/pricing exactly (live-fetched). Anthropic's "Dreaming"
   memory feature is curation-only, no enforcement gate — validates rather than challenges `12`'s
   thesis that retrieval is not the same as a gate.

Full agent output (with URLs) not reproduced here in full; ask the session that ran it or re-run
the research if the citations are needed verbatim.
