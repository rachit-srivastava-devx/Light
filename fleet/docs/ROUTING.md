# Model routing for fleet

## Decision

Do **not** adopt a learned or hosted LLM router on fleet's default path. The better alternative for
this use case is deliberately small: retain the static role-to-model policy, complete the local
capability/availability/quota probe around it, and make selection a deterministic constraint solver.

This is not a temporary recommendation disguised as architecture. Fleet has only 2–4 already
authenticated local CLIs, and its decisive inputs are known facts:

1. the role fixes the eligible tier (`lead = opus`, `builder = codex / sonnet`);
2. the selected CLI must be installed, authenticated, operational, and have enough quota for the
   dispatch; and
3. a verifier's **resolved** model must differ from the builder's resolved model. The kernel already
   refuses `SELF_VERIFIED` in [`crates/fleet-router/src/role_check.rs`](../crates/fleet-router/src/role_check.rs).

A prompt classifier cannot improve those facts. At best it re-derives an explicit role from prose;
at worst it makes a safety or quota decision probabilistic. Hosted routers also replace the user's
existing CLI subscriptions with a new account, API key, model gateway, and usage bill.

The evidence says to keep this investment small. The Coding-Agent-OS honesty ledger isolated model
routing at **₹6.4 of ₹197.1 saved per merged task, or 3.2%**; context discipline produced the other
96.8%. Its reported routing multiple was only **1.09×**, versus **3.55×** for context discipline
(honesty calculation and operating rule, from the Coding-Agent-OS blueprint's cost-control-plane
and honesty-ledger sections — that blueprint tree has since been restructured and these sections
no longer resolve to a stable path here). Those
numbers describe a different, API-priced system, so they are not a direct estimate of fleet's CLI
subscriptions. They are nevertheless the only repository measurement offered for the value of
model routing, and they say plainly that **routing is low-value**. Fleet should build the minimum
needed for role separation and graceful exhaustion, not a routing product.

## What fleet actually needs

The resolver should apply constraints in this order:

| Order | Input | Required behavior |
|---:|---|---|
| 1 | explicit role | Map to an auditable eligibility set: lead → Opus; builder → Codex/Sonnet; verifier → worker-tier models excluding the builder's resolved model. Never infer a role from prompt text. |
| 2 | safety policy | Refuse human-only task classes and `lead` implementation. These are gates, not model recommendations. |
| 3 | local capability | Keep only adapters whose binary is installed, non-interactive invocation works, operator authentication is valid, and required readback/usage capabilities exist. |
| 4 | availability and quota | Keep only CLIs not in cooldown and with a sufficient known quota window. Treat auth failure, rate limit, and exhausted quota as typed states; do not relabel them as model failure. |
| 5 | verifier independence | Remove the builder's **resolved model**, not merely its requested alias or CLI name. If no independent verifier remains, refuse rather than silently self-verify. |
| 6 | deterministic preference | Pick the first eligible entry in a checked-in order. Record the candidate set, exclusions, selected adapter, resolved model, and reason. |

[`crates/fleet-crew/crew/adapters/capability_probe.py`](../crates/fleet-crew/crew/adapters/capability_probe.py) is the right seam and the
do-nothing baseline, but it is not yet the whole resolver. Today it checks adapter method shape,
resolved-model readback, a usage-record method, and `operator_credentials()`. The Claude and Codex
adapters currently return `True` for operator credentials, and the probe does not itself test
whether the executable is on `PATH` or whether subscription quota remains. Therefore the
recommendation is **complete the existing deterministic design**, not claim that availability and
quota routing already work.

The intended policy already appears in the fleet blueprint as a table, including the different-model
verifier invariant and a single pre-dispatch exhaustion check (routing policy and quota scheduling,
from the Fleet-L8-Deep-Dive blueprint's keyless-tokenomics-and-routing section — that blueprint tree
has since been restructured per-crate and this section no longer resolves to a stable path here).

## Candidate assessment

“Keyless” below means the **required end-to-end fleet path** needs no new API key, login, or hosted
service. A router process being locally runnable is insufficient if its normal embedding, training,
or model-call path still requires provider credentials.

### RouteLLM (LMSYS / `lm-sys`)

- **Keyless?** **No on its recommended/default path.** The recommended matrix-factorization router
  still requires `OPENAI_API_KEY` for embeddings, and its quickstart uses OpenAI and Anyscale keys.
  It can target an OpenAI-compatible local endpoint, and some non-default router choices could be
  made local, but that is a material alternate integration rather than a keyless fleet default.
- **Local-CLI-aware?** No. It serves or wraps chat-completions APIs through LiteLLM. It does not
  discover or invoke authenticated `claude` and `codex` processes, read their subscription quota,
  or enforce builder/verifier identity.
- **What does it optimise?** A strong-versus-weak model's predicted win rate under a cost/quality
  threshold, learned from preference data. It is explicitly designed to send simpler prompts to a
  cheaper model, normally between two models.
- **Adoption cost:** Add a Python serving layer and LiteLLM, bridge CLIs into API-shaped endpoints,
  choose/retrain a router for fleet's coding-task distribution, calibrate thresholds, operate model
  artifacts/embeddings, and separately rebuild all role, quota, and independence constraints.
- **Verdict:** **Reject for the default path.** It is the strongest open-source answer to a different
  problem. Its two-model focus fits fleet's choice-set size better than most candidates, but its
  signal and integration boundary do not. Source: [RouteLLM README](https://github.com/lm-sys/RouteLLM#readme)
  and [paper](https://arxiv.org/abs/2406.18665).

### Microsoft `best-route-llm` (HybridLLM / BEST-Route)

- **Keyless?** **Not end-to-end for the published recipe.** Router inference can run locally after
  training, but the recipe generates up to 20 responses per prompt/model and says to configure a
  Hugging Face token or other API keys where required. The paper's experiments used paid OpenAI,
  AzureML, and Mistral access and an NVIDIA A100. That is not a zero-setup fleet path.
- **Local-CLI-aware?** No. Candidate models are dataset labels and inference providers, not local CLI
  adapters with installation, authentication, quota, and resolved-model state.
- **What does it optimise?** HybridLLM learns a quality/cost choice between a pair of models.
  BEST-Route also selects best-of-*n* sample count for a small model, optimizing test-time
  cost/quality by spending more samples on harder prompts.
- **Adoption cost:** Construct a representative labelled corpus, sample every candidate repeatedly,
  score outputs with an oracle reward model, train proxy reward and DeBERTa routers, provision the
  training stack, then write a CLI execution and quota layer it does not contain. Best-of-*n* also
  consumes exactly the subscription quota fleet is trying to preserve.
- **Verdict:** **Reject.** A research training pipeline is not a drop-in runtime, and multi-sampling
  points in the wrong direction under quota exhaustion. Source: [Microsoft repository](https://github.com/microsoft/best-route-llm#readme)
  and [BEST-Route paper](https://arxiv.org/abs/2506.22716).

#### Where LLM-Blender fits

Microsoft's requirements depend on the separate
[LLM-Blender project](https://github.com/yuchenlin/LLM-Blender#readme); LLM-Blender is not a
Microsoft local-CLI router. Its core method ranks multiple already-generated candidate responses
and can fuse the top ones into another response.

- **Keyless?** The ranker/fuser weights can run locally after download, so that component can be
  keyless. Obtaining all candidate responses still requires invoking all local CLIs.
- **Local-CLI-aware?** No; fleet would have to produce and attribute the candidates itself.
- **What does it optimise?** Ensemble output quality through pairwise ranking and generative fusion,
  not pre-call model selection, availability, or quota preservation.
- **Adoption cost:** Multiple model calls per task, large local ranker/fuser weights and inference,
  response normalization, and a new provenance question for fused output.
- **Verdict:** **Reject.** It multiplies quota use and changes authorship semantics instead of
  deciding which one CLI should receive a role-bound job.

### Not Diamond

- **Keyless?** **No.** Both pretrained selection and custom-router training require a
  `NOTDIAMOND_API_KEY` and a hosted Not Diamond call. Its create path additionally expects provider
  keys when Not Diamond invokes the selected model.
- **Local-CLI-aware?** No. A custom router can accept abstract/custom candidate labels and return a
  selection, but fleet would still need to map it to local processes and independently manage CLI
  state, quota, and resolved identity.
- **What does it optimise?** Predicted per-prompt quality across candidate models, with optional
  cost and latency trade-offs. Custom training consumes representative prompts, each candidate's
  responses, and numeric evaluation scores.
- **Adoption cost:** A new account/key and hosted required dependency; prompt disclosure to a third
  party; evaluation data generation and retraining; API integration; and all fleet-specific
  constraints still implemented locally.
- **Verdict:** **Disqualified.** The hosted key is enough to reject it from the default path, before
  considering fit. Source: [official SDK README](https://github.com/Not-Diamond/notdiamond-python#readme)
  and [custom-router training](https://docs.notdiamond.ai/docs/router-training-quickstart).

### Martian

- **Keyless?** **No.** The current Martian Gateway requires signup/login, a `MARTIAN_API_KEY`, a
  hosted endpoint, and credits.
- **Local-CLI-aware?** No. It can be configured as the backend *inside* Claude Code or Codex, but
  that replaces the user's existing vendor-authenticated path with Martian bearer authentication.
  Its model catalog is hosted API models, not installed local CLI sessions.
- **What does it optimise?** The public product is now a unified gateway over 200+ models, with
  model availability/pricing, provider compatibility, and optional gateway/load-balancing or cost
  optimization. This is large-model-zoo/provider routing.
- **Adoption cost:** New account, key, credits, hosted dependency, rerouting CLI traffic through a
  third party, and separate local role/independence logic. It also expands fleet's choice set from a
  few known CLIs to hundreds of models without a requirement for doing so.
- **Verdict:** **Disqualified.** Even its CLI integrations violate fleet's keyless meaning. Source:
  [authentication](https://docs.withmartian.com/api-reference/authentication),
  [model catalog](https://docs.withmartian.com/api-reference/models), and
  [CLI integrations](https://docs.withmartian.com/integrations).

### OpenRouter Auto Router

- **Keyless?** **No.** `openrouter/auto` is a hosted OpenRouter API call requiring an
  `OPENROUTER_API_KEY` (or an OAuth login that ultimately issues a key) and model usage billing.
- **Local-CLI-aware?** No. It selects hosted model slugs from a curated pool. Allowed-model filters
  can narrow that pool but cannot select an installed/authenticated local process or see its CLI
  subscription quota.
- **What does it optimise?** Not Diamond-powered selection using prompt complexity, task type, and
  model capabilities, with a quality/cost preference. The pool is updated by the service.
- **Adoption cost:** New hosted account/key/credits, changing the only model-call site from CLIs to
  an API gateway, provider/model drift management, and reimplementation of fleet's role and
  verifier constraints around the returned model id.
- **Verdict:** **Disqualified.** It is convenient precisely because it owns the hosted model path
  fleet has ruled out. Source: [Auto Router documentation](https://openrouter.ai/docs/guides/routing/routers/auto-router)
  and [OpenRouter quickstart](https://openrouter.ai/docs/quickstart).

### `semantic-router` (Aurelio Labs)

- **Keyless?** **Yes, if configured deliberately.** Its default examples use OpenAI or Cohere
  embeddings, but it documents a fully local `HuggingFaceEncoder`/`LlamaCppLLM` installation.
- **Local-CLI-aware?** No. It returns a semantic route label. It has no built-in Claude/Codex
  process discovery, authentication check, quota state, resolved-model readback, or
  builder/verifier exclusion.
- **What does it optimise?** Fast intent classification: similarity between an input and example
  utterances for named routes, with tunable thresholds. It does not learn which candidate model
  produces the best fleet outcome unless fleet builds that evaluation layer around it.
- **Adoption cost:** Add a Python dependency and local embedding model, curate example utterances,
  tune thresholds, version artifacts, handle no-match behavior, and still implement every
  operational constraint. Since fleet already receives an explicit role, the classifier would
  probabilistically infer a value it already has.
- **Verdict:** **Reject now; retain only as an experiment arm.** It is the only evaluated library
  that can satisfy keyless without a hosted service, but explicit role dispatch makes its semantic
  signal redundant. Source: [semantic-router README](https://github.com/aurelio-labs/semantic-router#readme)
  and [local execution guide](https://github.com/aurelio-labs/semantic-router/blob/main/docs/05-local-execution.ipynb).

### Do nothing: static table plus local probe

- **Keyless?** **Yes.** It invokes only CLIs to which the operator is already authenticated.
- **Local-CLI-aware?** **Yes by design, partially implemented today.** The adapters and capability
  report are local-CLI-specific; executable/auth health and quota-window state still need completing.
- **What does it optimise?** Constraint satisfaction and continuity: correct role, verifier
  independence, installed/working CLI, and enough quota. A fixed preference order breaks ties; it
  does not claim to predict which model is “smartest” for a prompt.
- **Adoption cost:** Low and bounded: strengthen the existing probes, add typed availability/quota
  state and cooldown, centralize the one pre-dispatch check, and ledger the full decision. There is
  no training corpus, model artifact, daemon, key, or hosted dependency.
- **Verdict:** **Adopt this baseline.** In fleet, “do nothing” means declining a learned router, not
  declining routing correctness.

## Recommendation

Build no third-party router in the next phase. Complete a deterministic `role × capability ×
availability × quota × independence` resolver at the existing adapter seam. Keep policy in a small
checked-in table and keep the kernel's post-resolution `SELF_VERIFIED` refusal authoritative. Never
let an inferred task class override an explicit role or turn an unavailable CLI into an eligible
one. When quota is unknown, publish `unknown`; do not fabricate zero or “available.”

This recommendation should be revisited only if fleet collects evidence that, **among candidates
already eligible under those hard constraints**, task content predicts a material outcome difference.

## Falsifiable experiment

Pre-register and run this before adding any learned router to production:

1. Collect **120 representative builder tasks** stratified by repository language and task shape
   (bug fix, feature, refactor, mechanical change). Freeze the briefs and acceptance tests before
   execution. Lead and verifier routing remain fixed because they are policy constraints.
2. On the first 60 tasks, run each eligible builder CLI from the same commit in fresh worktrees.
   Use the same limits and an independent verifier whose resolved model differs from that candidate
   builder. Record accepted/not accepted, wall time, truncation/rate-limit events, resolved model,
   and availability/quota state. Publish `{checked,total}` for every arm.
3. Fit/tune a fully local `semantic-router` arm (or a smaller local classifier) only on those 60
   tasks. Freeze it. The control is the deterministic static preference order. Neither arm may
   override role, capability, quota, or verifier-independence filters.
4. Shadow both policies on the 60-task holdout, but execute the selected routes needed to obtain
   paired outcomes. Primary metric: independently accepted tasks per 60. Secondary metrics:
   quota/rate-limit-truncated tasks, median completion time, and route regret (selected candidate
   failed while another eligible candidate passed). Count any invariant violation as a failed arm,
   not as missing data.

**Adopt a learned content router only if**, on the untouched holdout, it delivers at least **6 more
accepted tasks out of 60** (a 10 percentage-point absolute improvement), has fewer quota-truncated
runs, has **zero** role or `SELF_VERIFIED` violations, and remains fully local/keyless. Repeat the
holdout once on a later 60-task cohort before putting it on the required path.

**Do not adopt it** if the improvement is fewer than 6/60, either arm checks zero inputs, the gain
disappears on the second cohort, quota truncations increase, any hard constraint is violated, or a
new API key/login/hosted dependency is required. That result would confirm the present
recommendation: the useful router is the deterministic availability-and-invariant resolver, and
content-based model selection is maintenance cost for a roughly 3% lever.
