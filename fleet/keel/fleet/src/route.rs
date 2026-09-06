//! Auditable deterministic role-to-model routing.

use crate::meter::PlanningSnapshot;
use crate::roles::{self, Check, Role};
use serde::Serialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::Path;
use std::process::{Command, Stdio};

const EXIT_INVARIANT: i32 = 6;
const EXIT_REFUSAL: i32 = 7;

/// Policy and deterministic tie-breaker. Order changes require review.
const ORDER: &[CandidateSpec] = &[
    // D50: `freelane` was a runnable adapter the router could never select. Found by running a
    // whole session: the plan said "routed lane: UNAVAILABLE" while the one lane that was up and
    // measured sat outside the table. Ordered LAST deliberately -- it is the keyless fallback,
    // preferred only when the authenticated CLIs are unavailable or out of quota, never over them.
    CandidateSpec {
        id: "codex",
        adapter: "codex",
        requested: "codex-worker",
        resolved: "codex",
        tier: Tier::Worker,
    },
    CandidateSpec {
        id: "sonnet",
        adapter: "claude",
        requested: "claude-sonnet",
        resolved: "sonnet",
        tier: Tier::Worker,
    },
    CandidateSpec {
        id: "opus",
        adapter: "claude",
        requested: "opus",
        resolved: "opus",
        tier: Tier::Lead,
    },
    CandidateSpec {
        id: "haiku",
        adapter: "claude",
        requested: "haiku",
        resolved: "haiku",
        tier: Tier::Cheap,
    },
    CandidateSpec {
        id: "freelane",
        adapter: "freelane",
        requested: "codestral-latest",
        resolved: "codestral-latest",
        tier: Tier::Worker,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tier {
    Lead,
    Worker,
    Cheap,
}

#[derive(Clone, Copy, Debug)]
struct CandidateSpec {
    id: &'static str,
    adapter: &'static str,
    requested: &'static str,
    resolved: &'static str,
    tier: Tier,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskClass {
    General,
    Implementation,
    HumanOnly,
}

#[derive(Clone, Debug)]
struct RuntimeState {
    capable: BTreeSet<&'static str>,
    remaining: BTreeMap<String, Option<u64>>,
    cooldown: BTreeSet<String>,
    required_tokens: u64,
    preference: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Stage {
    pub(crate) number: usize,
    pub(crate) name: &'static str,
    pub(crate) checked: usize,
    pub(crate) total: usize,
    pub(crate) candidates: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Decision {
    pub(crate) role: &'static str,
    pub(crate) stages: Vec<Stage>,
    pub(crate) selected_adapter: Option<&'static str>,
    pub(crate) requested_model: Option<&'static str>,
    pub(crate) resolved_model: Option<&'static str>,
    pub(crate) decided_at_stage: Option<usize>,
    pub(crate) refusal: Option<Refusal>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Refusal {
    pub(crate) stage: usize,
    pub(crate) stage_name: &'static str,
    pub(crate) reason: String,
    pub(crate) fix: String,
}

fn role_allows(role: Role, candidate: CandidateSpec) -> bool {
    match role {
        Role::Lead | Role::Designer => candidate.tier == Tier::Lead,
        Role::Builder | Role::Verifier => candidate.tier == Tier::Worker,
        Role::Meter => candidate.tier == Tier::Cheap,
    }
}

fn stage(number: usize, name: &'static str, candidates: &[CandidateSpec]) -> Stage {
    Stage {
        number,
        name,
        checked: candidates.len(),
        total: ORDER.len(),
        candidates: candidates.iter().map(|candidate| candidate.id).collect(),
    }
}

fn decide(
    role: Option<Role>,
    class: TaskClass,
    builder_resolved_model: Option<&str>,
    runtime: &RuntimeState,
) -> Decision {
    let role_name = role.map(Role::name).unwrap_or("invalid");
    let mut stages = Vec::with_capacity(6);
    let mut first_empty: Option<Refusal> = None;
    let mut candidates = role
        .map(|role| {
            ORDER
                .iter()
                .copied()
                .filter(|candidate| role_allows(role, *candidate))
                // the turbofish is required: with `unwrap_or_default()` below there is no longer
                // a `Vec::new` for inference to key off, so collect's target must be explicit.
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    stages.push(stage(1, "explicit role", &candidates));
    if candidates.is_empty() {
        first_empty = Some(Refusal {
            stage: 1,
            stage_name: "explicit role",
            reason: "role has no eligible tier set".into(),
            fix: "pass --role lead|builder|verifier|designer|meter".into(),
        });
    }

    let safety_refusal = match (role, class) {
        (_, TaskClass::HumanOnly) => Some((
            "HUMAN_ONLY",
            "assign the money, contract, or migration decision to a human",
        )),
        (Some(Role::Lead), TaskClass::Implementation) => {
            let check = Check {
                role: Role::Lead,
                diff_adds_code: true,
                builder_model: None,
                verifier_model: None,
            };
            roles::evaluate(&check)
                .err()
                .map(|reason| (reason.reason(), "route implementation with --role builder"))
        }
        _ => None,
    };
    if let Some((reason, fix)) = safety_refusal {
        candidates.clear();
        first_empty.get_or_insert(Refusal {
            stage: 2,
            stage_name: "safety policy",
            reason: reason.into(),
            fix: fix.into(),
        });
    }
    stages.push(stage(2, "safety policy", &candidates));

    candidates.retain(|candidate| runtime.capable.contains(candidate.adapter));
    if candidates.is_empty() {
        first_empty.get_or_insert(Refusal {
            stage: 3,
            stage_name: "local capability",
            reason: "no eligible adapter is installed and usable non-interactively".into(),
            fix: "install and authenticate the eligible Claude Code or Codex CLI, then retry"
                .into(),
        });
    }
    stages.push(stage(3, "local capability", &candidates));

    candidates.retain(|candidate| {
        !runtime.cooldown.contains(candidate.adapter)
            && runtime
                .remaining
                .get(candidate.adapter)
                .copied()
                .flatten()
                .is_some_and(|remaining| remaining >= runtime.required_tokens)
    });
    if candidates.is_empty() {
        first_empty.get_or_insert(Refusal {
            stage: 4,
            stage_name: "availability/quota",
            reason: "all eligible lanes are cooling down or lack a sufficient known quota window"
                .into(),
            fix: format!(
                "wait for cooldown/reset or configure a measured window of at least {} tokens",
                runtime.required_tokens
            ),
        });
    }
    stages.push(stage(4, "availability/quota", &candidates));

    if role == Some(Role::Verifier) {
        match builder_resolved_model {
            Some(builder) => candidates.retain(|candidate| {
                let check = Check {
                    role: Role::Verifier,
                    diff_adds_code: false,
                    builder_model: Some(builder),
                    verifier_model: Some(candidate.resolved),
                };
                roles::evaluate(&check).is_ok()
            }),
            None => candidates.clear(),
        }
    }
    if candidates.is_empty() {
        first_empty.get_or_insert(Refusal { stage: 5, stage_name: "verifier independence", reason: "no verifier with a resolved model distinct from the builder remains".into(), fix: "pass --builder-model with the builder's resolved model and make another worker model available".into() });
    }
    stages.push(stage(5, "verifier independence", &candidates));

    let selected = ORDER
        .iter()
        .find(|ordered| {
            runtime.preference.contains(&ordered.id)
                && candidates
                    .iter()
                    .any(|candidate| candidate.id == ordered.id)
        })
        .copied();
    let final_candidates = selected.into_iter().collect::<Vec<_>>();
    if selected.is_none() {
        first_empty.get_or_insert(Refusal {
            stage: 6,
            stage_name: "deterministic pick",
            reason: "no surviving candidate appears in the committed preference order".into(),
            fix: "add an eligible candidate to the committed route order".into(),
        });
    }
    stages.push(stage(6, "deterministic pick", &final_candidates));

    Decision {
        role: role_name,
        stages,
        selected_adapter: selected.map(|candidate| candidate.adapter),
        requested_model: selected.map(|candidate| candidate.requested),
        resolved_model: selected.map(|candidate| candidate.resolved),
        decided_at_stage: selected.map(|_| 6),
        refusal: first_empty,
    }
}

fn adapter_contract(adapter: &str, verifier: bool) -> bool {
    let class = if adapter == "claude" {
        "ClaudeAdapter"
    } else {
        "CodexAdapter"
    };
    let field = if verifier {
        "verifier_eligible"
    } else {
        "builder_eligible"
    };
    let script = format!("from crew.adapters import {class}; from crew.adapters.capability import probe_adapter; raise SystemExit(0 if probe_adapter({class}()).{field} else 1)");
    let crew = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("crew");
    Command::new("python3")
        .arg("-c")
        .arg(script)
        .env("PYTHONPATH", crew)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn installed_non_interactive(adapter: &str) -> bool {
    Command::new(adapter)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn runtime(snapshot: PlanningSnapshot, verifier: bool, required_tokens: u64) -> RuntimeState {
    // D50: the capability probe only looked for CLIs on PATH, so `freelane` -- a repo-local
    // script at bin/freelane.sh, and the only keyless lane -- could never pass stage 3. Stage 3
    // was correct by its own rule; the rule did not know repo-local adapters exist.
    let mut capable: BTreeSet<&'static str> = ["codex", "claude"]
        .into_iter()
        .filter(|adapter| installed_non_interactive(adapter) && adapter_contract(adapter, verifier))
        .collect();
    let cooldown = env::var("FLEET_ROUTE_COOLDOWNS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    RuntimeState {
        capable: {
            let script = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("bin/freelane.sh");
            if script.is_file() {
                capable.insert("freelane");
            }
            capable
        },
        remaining: snapshot.remaining,
        cooldown,
        required_tokens,
        preference: ORDER.iter().map(|candidate| candidate.id).collect(),
    }
}

pub(crate) fn for_plan(role: Role, state: &Path) -> Result<Decision, i32> {
    for_plan_with_builder(role, state, None)
}

pub(crate) fn for_plan_with_builder(
    role: Role,
    state: &Path,
    builder_resolved_model: Option<&str>,
) -> Result<Decision, i32> {
    // Missing/corrupt meter evidence is an environment fault, not a measured zero-lane result.
    // Propagate the typed error so `run --role` and five-role swarm routing cannot fabricate a
    // `{checked:0,total:0}` snapshot and misreport it as an ordinary router refusal.
    let snapshot = crate::meter::planning_snapshot_at(state)?;
    let required = snapshot.estimated_task_tokens.unwrap_or(1);
    let runtime = runtime(snapshot, role == Role::Verifier, required);
    Ok(decide(
        Some(role),
        if role == Role::Builder {
            TaskClass::Implementation
        } else {
            TaskClass::General
        },
        builder_resolved_model,
        &runtime,
    ))
}

fn print_human(decision: &Decision) {
    for item in &decision.stages {
        println!(
            "stage {}: {} of {} remain [{}] — {}",
            item.number,
            item.checked,
            item.total,
            item.candidates.join(", "),
            item.name
        );
    }
    if let Some(refusal) = &decision.refusal {
        eprintln!(
            "REFUSE at stage {} ({}): {}. Fix: {}.",
            refusal.stage, refusal.stage_name, refusal.reason, refusal.fix
        );
    } else {
        println!(
            "selected: adapter={} requested_model={} resolved_model={} decided_at_stage={}",
            decision.selected_adapter.unwrap_or(""),
            decision.requested_model.unwrap_or(""),
            decision.resolved_model.unwrap_or(""),
            decision.decided_at_stage.unwrap_or(0)
        );
    }
}

pub(crate) fn command(args: &[String], state: &Path) -> Result<(), i32> {
    let mut role_raw = None;
    let mut builder_model = None;
    let mut class = TaskClass::General;
    let mut required_tokens = 1_u64;
    let mut json_output = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json_output = true;
                index += 1;
            }
            "--role" | "--builder-model" | "--task-class" | "--tokens" => {
                let flag = args[index].as_str();
                let Some(value) = args.get(index + 1) else {
                    return invalid_args(args);
                };
                match flag {
                    "--role" => role_raw = Some(value.as_str()),
                    "--builder-model" => builder_model = Some(value.as_str()),
                    "--task-class" => {
                        class = match value.as_str() {
                            "general" => TaskClass::General,
                            "implementation" => TaskClass::Implementation,
                            "human-only" => TaskClass::HumanOnly,
                            _ => return invalid_args(args),
                        }
                    }
                    "--tokens" => {
                        required_tokens = match value.parse::<u64>() {
                            Ok(n) if n > 0 => n,
                            _ => return invalid_args(args),
                        }
                    }
                    _ => unreachable!(),
                }
                index += 2;
            }
            _ => return invalid_args(args),
        }
    }
    let role = role_raw.and_then(Role::parse);
    let snapshot = crate::meter::planning_snapshot_at(state).unwrap_or(PlanningSnapshot {
        estimated_task_tokens: None,
        checked: 0,
        total: 0,
        remaining: BTreeMap::new(),
    });
    let runtime = runtime(snapshot, role == Some(Role::Verifier), required_tokens);
    let decision = decide(role, class, builder_model, &runtime);
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&decision).map_err(|_| EXIT_INVARIANT)?
        );
    } else {
        print_human(&decision);
    }
    if let Some(refusal) = &decision.refusal {
        crate::append_receipt(
            "refusal",
            json!({"reason":"ROUTE_REFUSED","stage":refusal.stage,"stage_name":refusal.stage_name,"detail":refusal.reason,"fix":refusal.fix,"checked":decision.stages[refusal.stage - 1].checked,"total":decision.stages[refusal.stage - 1].total}),
            "fleet-route",
            None,
            Some(EXIT_REFUSAL),
        )?;
        Err(EXIT_REFUSAL)
    } else {
        Ok(())
    }
}

fn invalid_args(args: &[String]) -> Result<(), i32> {
    eprintln!("usage: fleet route --role <lead|builder|verifier|designer|meter> [--builder-model M] [--task-class general|implementation|human-only] [--tokens N] [--json]");
    crate::append_receipt(
        "refusal",
        json!({"reason":"INVALID_ROUTE_ARGS","args":args,"checked":0,"total":ORDER.len()}),
        "fleet-route",
        None,
        Some(EXIT_REFUSAL),
    )?;
    Err(EXIT_REFUSAL)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_runtime() -> RuntimeState {
        RuntimeState {
            capable: ["codex", "claude"].into_iter().collect(),
            remaining: [("codex".into(), Some(100)), ("claude".into(), Some(100))]
                .into_iter()
                .collect(),
            cooldown: BTreeSet::new(),
            required_tokens: 1,
            preference: ORDER.iter().map(|candidate| candidate.id).collect(),
        }
    }

    #[test]
    fn every_filter_stage_names_its_empty_set() {
        assert_eq!(
            decide(None, TaskClass::General, None, &full_runtime())
                .refusal
                .unwrap()
                .stage,
            1
        );
        assert_eq!(
            decide(
                Some(Role::Lead),
                TaskClass::Implementation,
                None,
                &full_runtime()
            )
            .refusal
            .unwrap()
            .stage,
            2
        );
        let mut no_capability = full_runtime();
        no_capability.capable.clear();
        assert_eq!(
            decide(
                Some(Role::Builder),
                TaskClass::General,
                None,
                &no_capability
            )
            .refusal
            .unwrap()
            .stage,
            3
        );
        let mut no_quota = full_runtime();
        no_quota.remaining.clear();
        assert_eq!(
            decide(Some(Role::Builder), TaskClass::General, None, &no_quota)
                .refusal
                .unwrap()
                .stage,
            4
        );
        let one_model = RuntimeState {
            capable: ["codex"].into_iter().collect(),
            remaining: [("codex".into(), Some(100))].into_iter().collect(),
            cooldown: BTreeSet::new(),
            required_tokens: 1,
            preference: ORDER.iter().map(|candidate| candidate.id).collect(),
        };
        assert_eq!(
            decide(
                Some(Role::Verifier),
                TaskClass::General,
                Some("codex"),
                &one_model
            )
            .refusal
            .unwrap()
            .stage,
            5
        );
        let mut no_order = full_runtime();
        no_order.preference.clear();
        assert_eq!(
            decide(Some(Role::Builder), TaskClass::General, None, &no_order)
                .refusal
                .unwrap()
                .stage,
            6
        );
        assert_eq!(
            decide(
                Some(Role::Verifier),
                TaskClass::General,
                None,
                &full_runtime()
            )
            .refusal
            .unwrap()
            .stage,
            5
        );
    }

    #[test]
    fn verifier_compares_resolved_identity_in_both_directions() {
        assert_eq!(
            decide(
                Some(Role::Verifier),
                TaskClass::General,
                Some("codex"),
                &full_runtime()
            )
            .resolved_model,
            Some("sonnet")
        );
        assert_eq!(
            decide(
                Some(Role::Verifier),
                TaskClass::General,
                Some("requested-codex-alias"),
                &full_runtime()
            )
            .resolved_model,
            Some("codex")
        );
    }

    #[test]
    fn lead_implementation_refuses_and_lead_contract_routes() {
        assert_eq!(
            decide(
                Some(Role::Lead),
                TaskClass::Implementation,
                None,
                &full_runtime()
            )
            .refusal
            .unwrap()
            .reason,
            "LEAD_WROTE_CODE"
        );
        assert_eq!(
            decide(Some(Role::Lead), TaskClass::General, None, &full_runtime()).resolved_model,
            Some("opus")
        );
        assert_eq!(
            decide(
                Some(Role::Builder),
                TaskClass::HumanOnly,
                None,
                &full_runtime()
            )
            .refusal
            .unwrap()
            .reason,
            "HUMAN_ONLY"
        );
        assert!(decide(
            Some(Role::Builder),
            TaskClass::General,
            None,
            &full_runtime()
        )
        .refusal
        .is_none());
    }

    #[test]
    fn deterministic_across_runs_and_both_availability_directions() {
        let first = decide(
            Some(Role::Builder),
            TaskClass::General,
            None,
            &full_runtime(),
        );
        for _ in 0..20 {
            assert_eq!(
                decide(
                    Some(Role::Builder),
                    TaskClass::General,
                    None,
                    &full_runtime()
                )
                .resolved_model,
                first.resolved_model
            );
        }
        let mut codex_cooling = full_runtime();
        codex_cooling.cooldown.insert("codex".into());
        assert_eq!(
            decide(
                Some(Role::Builder),
                TaskClass::General,
                None,
                &codex_cooling
            )
            .resolved_model,
            Some("sonnet")
        );
        assert_eq!(first.resolved_model, Some("codex"));
    }
}
