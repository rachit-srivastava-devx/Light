mod console;
mod graph;
mod intent;
mod mcp;
mod ratchet;
mod repl;
mod roles;
mod route;
mod sow;
mod status;
mod swarm;
mod worktree;
use fleet::agent;
use fleet::lifecycle;
use fs2::FileExt;
use libc::{c_int, c_void};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const EXIT_OK: i32 = 0;
const EXIT_ENV: i32 = 3;
const EXIT_INVARIANT: i32 = 6;
const EXIT_REFUSAL: i32 = 7;
const EXIT_MISMATCH: i32 = 8;
const EXIT_SOW_READY: i32 = sow::EXIT_READY_AWAITING_REVIEW;

fn main() {
    use std::io::IsTerminal;

    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let print_mode = args.first().map(String::as_str) == Some("--print");
    if print_mode {
        args.remove(0);
    }
    if args.is_empty() && !print_mode && io::stdin().is_terminal() && io::stdout().is_terminal() {
        args.push("__repl".to_string());
    }
    let code = match dispatch(args) {
        Ok(()) => EXIT_OK,
        Err(code) => code,
    };
    std::process::exit(code);
}

fn dispatch(args: Vec<String>) -> Result<(), i32> {
    match args.first().map(String::as_str) {
        Some("__repl") if args.len() == 1 => repl::run(),
        Some("meter") => meter::command(&args[1..]),
        Some("route") => {
            let state = state_dir()?;
            route::command(&args[1..], &state)
        }
        Some("roles") if args.len() == 1 => {
            roles::print_roles();
            Ok(())
        }
        Some("swarm") => swarm_command(&args[1..]),
        Some("sow") => sow_command(&args[1..]),
        Some("plan") => plan_command(&args[1..]),
        Some("skills") if args.len() == 1 => fleet::skills::command(false),
        Some("skills") if args.len() == 2 && args[1] == "--check" => fleet::skills::command(true),
        Some("skills") => {
            eprintln!("usage: fleet skills [--check]");
            refusal_with_receipt(
                json!({"reason":"INVALID_SKILLS_ARGS","args":args[1..].to_vec()}),
                EXIT_REFUSAL,
            )
        }
        Some("roles") => {
            eprintln!("usage: fleet roles");
            refusal_with_receipt(
                json!({"reason":"INVALID_ROLES_ARGS","args":args[1..].to_vec()}),
                EXIT_REFUSAL,
            )
        }
        Some("role-check") => role_check_command(&args[1..]),
        Some("agents") if args.get(1).map(String::as_str) == Some("list") => agent::list(),
        Some("lifecycle") => lifecycle::command(&args[1..]),
        Some("run") => {
            // D53: structural checks BEFORE the policy gate. A non-existent repo and a non-git
            // directory both reported "task has no accepted SOW"; the advice (write a SOW) could
            // not possibly fix either. Report what the user must fix first.
            if let Some(repo) = argument_value(&args[1..], "--repo") {
                let repo_path = Path::new(repo);
                if !repo_path.is_dir() {
                    eprintln!("fleet: --repo {repo} does not exist (or is not a directory).");
                    return Err(EXIT_ENV);
                }
                if !repo_path.join(".git").exists() {
                    eprintln!(
                        "fleet: --repo {repo} is not a git repository.\n\
                         fleet freezes a git diff, so the target must be a repo with a commit:\n\
                         \n  git -C {repo} init && git -C {repo} commit --allow-empty -m init"
                    );
                    return Err(EXIT_ENV);
                }
            }
            if let Some(task) =
                argument_value(&args[1..], "--task").filter(|task| !task.trim().is_empty())
            {
                enforce_accepted_sow(task)?;
            }
            lifecycle::drive_run(&args[1..], || run_command(&args[1..]))
        }
        Some("oracle") if args.len() == 1 => {
            eprintln!("fleet oracle <o1|o2> --artifact <id> --suite <path> --author <name>");
            Err(EXIT_REFUSAL)
        }
        Some("oracle") => oracle_command(&args[1..]),
        Some("adjudicate") if args.len() == 1 => {
            eprintln!(
                "fleet adjudicate --artifact <id>   # runs O1+O2, applies the 2x2 discriminator"
            );
            Err(EXIT_REFUSAL)
        }
        Some("adjudicate") => {
            if args.len() == 3 && args[1] == "--artifact" {
                adjudicate_command(&args[2])
            } else {
                refusal_with_receipt(
                    json!({"reason":"INVALID_ADJUDICATE_ARGS","args":args[1..].to_vec()}),
                    EXIT_REFUSAL,
                )
            }
        }
        Some("attest") if args.get(1).map(String::as_str) == Some("verify") => {
            let id = args.get(2).ok_or(EXIT_REFUSAL)?;
            attest_verify(id)
        }
        Some("pr") if args.get(1).map(String::as_str) == Some("emit") => {
            pr_emit_command(&args[2..])
        }
        Some("pr") => {
            eprintln!("{PR_EMIT_USAGE}");
            Err(EXIT_REFUSAL)
        }
        // Test-only probe for F08's acceptance test, mirroring `__lanes_probe` above: seeds a
        // real fixture (real diff, real ledger receipts, real attestation file) through
        // production primitives, then calls the SAME production `pr_emit_run` that `fleet pr
        // emit` calls, so the acceptance test drives real code, not a test-only path. Not a
        // user-facing command. See `keel/fleet/tests/f08_pr_emit.rs`.
        Some("__pr_emit_probe") => pr_emit_probe_command(&args[1..]),
        Some("status") if args.len() == 1 => status_command(false),
        Some("status") if args.len() == 2 && args[1] == "--json" => status_command(true),
        Some("status") => refusal_with_receipt(
            json!({"reason":"INVALID_STATUS_ARGS","args":args[1..].to_vec()}),
            EXIT_REFUSAL,
        )
        .inspect_err(|_| {
            eprintln!("usage: fleet status [--json]\n  e.g. fleet status --json | jq .");
        }),
        Some("rollback") => rollback_command(&args[1..]),
        Some("ledger") => ledger_command(&args[1..]),
        Some("contract") => contract_command(&args[1..]),
        Some("__agent") => agent_command(&args[1..]),
        // Test-only probe for S3's concurrent, worktree-isolated role lanes. Not a user-facing
        // command (no entry in CMDS/help). It exists because `spawn_agent_with_args` re-execs
        // `env::current_exe()` -- inside `cargo test` that resolves to the test HARNESS binary,
        // not `fleet` itself, so a Rust `#[test]` cannot exercise the real re-exec path. This
        // subcommand lets a Cargo integration test drive the exact same production function
        // (`run_role_lanes_concurrently`) through the REAL compiled binary via `Command`, with a
        // deterministic "stub" agent so the proof needs no network/CLI credentials. See
        // `keel/fleet/tests/s3_lanes.rs`.
        Some("__lanes_probe") => lanes_probe_command(&args[1..]),
        Some("ratchet") => ratchet::command(&args[1..]),
        Some("console") => {
            if args[1..].iter().any(|a| a == "--help" || a == "-h") {
                println!(
                    "fleet console — operator TUI (read-only).\n\n\
                     USAGE\n  fleet console [--task <ID>]\n\n\
                     VIEWS\n  fleet  what is running · what is blocked · what it cost\n  \
                     task   agents on one task, their lifecycle state and verdicts\n  \
                     agent  brief · diff · verdict · receipts\n\n\
                     Reads $FLEET_STATE. Never mutates. Unknown values render as \"—\" with a reason."
                );
                return Ok(());
            }
            console::run(&args[1..])
        }
        Some("graph") if args.len() == 1 => {
            eprintln!("fleet graph index --repo <path>   # build the symbol/call index");
            Err(EXIT_REFUSAL)
        }
        Some("graph") => graph::graph_command(&args[1..]),
        Some("impact") => graph::impact_command(&args[1..]),
        Some("mcp") if args.len() == 1 => {
            eprintln!(
                "fleet mcp manifest <lease>  |  fleet mcp <repo> <lease>   # lease-scoped tools"
            );
            Err(EXIT_REFUSAL)
        }
        Some("mcp") if args.get(1).map(String::as_str) == Some("manifest") => {
            let lease = args.get(2).map(String::as_str).unwrap_or("**");
            let m = mcp::manifest_for_lease(lease)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&m).map_err(|_| EXIT_INVARIANT)?
            );
            Ok(())
        }
        Some("mcp") => {
            let repo = args.get(1).map(String::as_str).unwrap_or(".");
            let lease = args.get(2).map(String::as_str).unwrap_or("**");
            mcp::serve(repo, lease)
        }
        Some("completions") => {
            let shell = args.get(1).map(String::as_str).unwrap_or("");
            match shell {
                "bash" | "zsh" | "fish" => {
                    print_completions(shell);
                    Ok(())
                }
                _ => {
                    eprintln!("fleet completions <bash|zsh|fish>");
                    Err(EXIT_REFUSAL)
                }
            }
        }
        Some("doctor") => doctor(),
        Some("--version") | Some("-V") | Some("version") => {
            println!(
                "fleet {} ({}, built {})",
                env!("CARGO_PKG_VERSION"),
                option_env!("FLEET_GIT_SHA").unwrap_or("unknown-sha"),
                option_env!("FLEET_BUILD_DATE").unwrap_or("unknown-date")
            );
            Ok(())
        }
        Some("--help") | Some("-h") | Some("help") | None => {
            print_help();
            Ok(())
        }
        _ => {
            eprintln!("fleet: unknown command. Try `fleet --help`.");
            Err(EXIT_REFUSAL)
        }
    }
}

fn argument_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
}

fn sow_command(args: &[String]) -> Result<(), i32> {
    if let [flag, task] = args {
        if flag == "--task" {
            return create_sow(task);
        }
    }
    if let [command, flag, id] = args {
        if command == "accept" && flag == "--id" {
            return accept_sow(id);
        }
    }
    let _ = state_dir()?;
    eprintln!("fleet sow --task <T>  |  fleet sow accept --id <ID>");
    refusal_with_receipt(
        json!({"reason":"INVALID_SOW_ARGS","args":args}),
        EXIT_REFUSAL,
    )
}

fn create_sow(task: &str) -> Result<(), i32> {
    let state = state_dir()?;
    let sow_payload = match sow::invoke_crew(task) {
        Ok(payload) => payload,
        Err(sow::BuildError::Refused(detail)) => {
            eprintln!("{detail}");
            return refusal_with_receipt(
                json!({
                    "reason":"SOW_AMBIGUOUS",
                    "task_id":sow::id_for_task(task),
                    "detail":detail
                }),
                EXIT_REFUSAL,
            );
        }
        Err(sow::BuildError::Environment(detail)) => {
            eprintln!("fleet: environment fault invoking crew.sow: {detail}");
            return Err(EXIT_ENV);
        }
        Err(sow::BuildError::Invariant(detail)) => {
            eprintln!("fleet: invariant violation invoking crew.sow: {detail}");
            return Err(EXIT_INVARIANT);
        }
    };
    let id = sow::id_for_task(task);
    let lock_path = state.join("sows/.lock");
    fs::create_dir_all(state.join("sows")).map_err(|_| EXIT_ENV)?;
    let _guard = FileLock::acquire(&lock_path)?;
    let record = match sow::load(&state, &id)? {
        Some(existing)
            if existing.get("task").and_then(Value::as_str) == Some(task)
                && existing.get("id").and_then(Value::as_str) == Some(id.as_str()) =>
        {
            existing
        }
        Some(_) => return Err(EXIT_MISMATCH),
        None => {
            let created = sow::ready_record(&id, task, sow_payload, &now_rfc3339()?);
            sow::write_atomic(&state, &id, &created)?;
            created
        }
    };
    let checked = record
        .get("sow")
        .and_then(|value| value.get("leaves"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .ok_or(EXIT_INVARIANT)?;
    if checked == 0 {
        return Err(EXIT_INVARIANT);
    }
    append_receipt(
        "note",
        json!({
            "kind":"SOW_READY_AWAITING_REVIEW",
            "sow_id":id,
            "checked":checked,
            "total":checked
        }),
        "fleet-sow",
        None,
        Some(EXIT_SOW_READY),
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(record.get("sow").ok_or(EXIT_INVARIANT)?)
            .map_err(|_| EXIT_INVARIANT)?
    );
    eprintln!("SOW_READY_AWAITING_REVIEW id={id}");
    eprintln!("Accept with: fleet sow accept --id {id}");
    Err(EXIT_SOW_READY)
}

fn accept_sow(id: &str) -> Result<(), i32> {
    let state = state_dir()?;
    if !valid_artifact_id(id) {
        eprintln!("fleet: refusing SOW acceptance: invalid id {id:?}");
        return refusal_with_receipt(json!({"reason":"INVALID_SOW_ID","sow_id":id}), EXIT_REFUSAL);
    }
    let actor = env::var("USER")
        .or_else(|_| env::var("LOGNAME"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            eprintln!("fleet: environment fault: USER or LOGNAME is required to accept a SOW");
            EXIT_ENV
        })?;
    fs::create_dir_all(state.join("sows")).map_err(|_| EXIT_ENV)?;
    let _guard = FileLock::acquire(&state.join("sows/.lock"))?;
    let mut record = match sow::load(&state, id)? {
        Some(record) => record,
        None => {
            eprintln!("fleet: refusing SOW acceptance: no ready SOW with id {id}");
            return refusal_with_receipt(
                json!({"reason":"SOW_NOT_READY","sow_id":id}),
                EXIT_REFUSAL,
            );
        }
    };
    let newly_accepted = !record.get("accepted").is_some_and(Value::is_object);
    if newly_accepted {
        record["accepted"] = json!({"by":actor,"at":now_rfc3339()?});
    }
    let accepted = record.get("accepted").cloned().ok_or(EXIT_INVARIANT)?;
    let acceptance_receipt = append_receipt(
        "note",
        json!({"kind":"SOW_ACCEPTED","sow_id":id,"acceptance":accepted,"newly_accepted":newly_accepted}),
        "fleet-sow",
        None,
        None,
    )?;
    if newly_accepted {
        record["accepted"]["receipt"] = Value::String(acceptance_receipt);
        sow::write_atomic(&state, id, &record)?;
    }
    println!(
        "SOW_ACCEPTED id={id} by={} at={}",
        accepted
            .get("by")
            .and_then(Value::as_str)
            .ok_or(EXIT_INVARIANT)?,
        accepted
            .get("at")
            .and_then(Value::as_str)
            .ok_or(EXIT_INVARIANT)?
    );
    Ok(())
}

fn enforce_accepted_sow(task: &str) -> Result<(), i32> {
    let state = state_dir()?;
    let id = sow::id_for_task(task);
    if env::var("FLEET_SOW_BYPASS").as_deref() == Ok("1") {
        eprintln!("fleet: SOW BYPASS active (FLEET_SOW_BYPASS=1) task_id={id}");
        append_receipt(
            "note",
            json!({
                "kind":"SOW_BYPASS",
                "sow_id":id,
                "environment":"FLEET_SOW_BYPASS=1",
                "visible":true
            }),
            "fleet-sow",
            None,
            None,
        )?;
        return Ok(());
    }
    let record = sow::load(&state, &id)?;
    if sow::review_status(task, record.as_ref()) == sow::ReviewStatus::Accepted {
        let receipt = record
            .as_ref()
            .and_then(|value| value.get("accepted"))
            .and_then(|value| value.get("receipt"))
            .and_then(Value::as_str)
            .ok_or(EXIT_MISMATCH)?;
        let rows = ledger_rows(false)?;
        verify_rows(&rows)?;
        if rows.iter().any(|row| {
            row.get("hash").and_then(Value::as_str) == Some(receipt)
                && row.get("event").and_then(Value::as_str) == Some("gate_verdict")
                && row
                    .get("body")
                    .and_then(|body| body.get("kind"))
                    .and_then(Value::as_str)
                    == Some("SOW_ACCEPTED")
                && row
                    .get("body")
                    .and_then(|body| body.get("sow_id"))
                    .and_then(Value::as_str)
                    == Some(id.as_str())
        }) {
            return Ok(());
        }
        eprintln!("fleet: refusing execution: SOW acceptance receipt is missing or mismatched");
    }
    eprintln!(
        "fleet: refusing execution: task has no accepted SOW.\n\
         SOW id to accept: {id}\n\
         Create it with: fleet sow --task <exact task>\n\
         Then accept it with: fleet sow accept --id {id}"
    );
    refusal_with_receipt(
        json!({"reason":"SOW_NOT_ACCEPTED","sow_id":id,"task":task}),
        EXIT_REFUSAL,
    )
}

fn swarm_command(args: &[String]) -> Result<(), i32> {
    // Q1: `fleet swarm dispatch` with no arguments printed NOTHING and exited non-zero. Found by
    // asserting actionability across every refusal surface rather than patching a fourth instance.
    if matches!(args.first().map(String::as_str), Some("dispatch")) && args.len() < 2 {
        eprintln!(
            "usage: fleet swarm dispatch --task <T> --repo <P> [--agent <A> | --role <R>]\n\
             \n  Allocates the task across the five roles, advances each agent's own lifecycle,\n\
             and records evidence (ledger + attestation). Needs an accepted SOW unless\n\
             FLEET_SOW_BYPASS=1. Neither flag given defaults to --agent stub; --role consults the\n\
             same router as `fleet run --role` and never overrides an explicit --agent.\n\
             \n  e.g. fleet swarm dispatch --task \"add a --version flag\" --repo ."
        );
        return Err(EXIT_REFUSAL);
    }
    if args.first().map(String::as_str) == Some("dispatch") {
        return swarm_dispatch_command(&args[1..]);
    }
    if matches!(args, [command] if command == "bandwidth") {
        let state = state_dir()?;
        return match swarm::bandwidth(&state) {
            Ok(report) => {
                println!("AGENT            ROLE         CAPACITY LOAD FREE");
                for agent in report.agents {
                    println!(
                        "{:<16} {:<12} {:>8} {:>4} {:>4}",
                        agent.id, agent.role, agent.capacity, agent.load, agent.free
                    );
                }
                println!("-- checked={} total={} --", report.checked, report.total);
                Ok(())
            }
            Err(error) => swarm_error("bandwidth", error),
        };
    }
    if let [command, flag, value] = args {
        if command == "allocate" && flag == "--tasks" {
            let tasks = match value.parse::<u64>() {
                Ok(tasks) => tasks,
                Err(_) => {
                    return refusal_with_receipt(
                        json!({"reason":"INVALID_TASK_COUNT","value":value}),
                        EXIT_REFUSAL,
                    )
                }
            };
            let state = state_dir()?;
            return match swarm::allocate(&state, tasks) {
                Ok(allocation) => {
                    println!(
                        "{}",
                        serde_json::to_string(&allocation).map_err(|_| EXIT_INVARIANT)?
                    );
                    Ok(())
                }
                Err(error) => swarm_error("allocate", error),
            };
        }
    }
    if let [command, agent_flag, agent, score_flag, score] = args {
        if command == "complete" && agent_flag == "--agent" && score_flag == "--score" {
            let score = match score.parse::<u64>() {
                Ok(score) => score,
                Err(_) => {
                    return refusal_with_receipt(
                        json!({"reason":"INVALID_ACCEPTANCE_SCORE","value":score}),
                        EXIT_REFUSAL,
                    )
                }
            };
            let state = state_dir()?;
            return match swarm::complete_task(&state, agent, score) {
                Ok(()) => {
                    println!("completed agent={agent} acceptance_score={score}");
                    Ok(())
                }
                Err(error) => swarm_error("complete", error),
            };
        }
    }
    let json_output = match args {
        [command] if command == "status" => false,
        [command, flag] if command == "status" && flag == "--json" => true,
        _ => {
            eprintln!("fleet swarm status [--json]");
            eprintln!("fleet swarm bandwidth");
            eprintln!("fleet swarm allocate --tasks N");
            eprintln!("fleet swarm complete --agent ID --score SCORE");
            return refusal_with_receipt(
                json!({"reason":"INVALID_SWARM_ARGS","args":args}),
                EXIT_REFUSAL,
            );
        }
    };
    let state = state_dir()?;
    let agents = match swarm::load_or_seed(&state) {
        Ok(agents) => agents,
        Err(swarm::StatusError::ZeroAgents) => {
            eprintln!("fleet: refusing swarm status: agent store contains zero agents");
            return refusal_with_receipt(
                json!({"reason":"ZERO_AGENTS","checked":0,"total":0}),
                EXIT_REFUSAL,
            );
        }
        Err(swarm::StatusError::TooFewAgents { found }) => {
            eprintln!(
                "fleet: refusing swarm status: a swarm requires at least two agents; found {found}"
            );
            return refusal_with_receipt(
                json!({"reason":"TOO_FEW_AGENTS","checked":found,"total":2}),
                EXIT_REFUSAL,
            );
        }
        Err(swarm::StatusError::Environment(message)) => {
            eprintln!("fleet: environment fault: {message}");
            return Err(EXIT_ENV);
        }
        Err(swarm::StatusError::Invariant(message)) => {
            eprintln!("fleet: invariant violation: {message}");
            return Err(EXIT_INVARIANT);
        }
        Err(swarm::StatusError::Refusal { agent, reason }) => {
            eprintln!("fleet: agent {agent} refused: {reason}");
            return refusal_with_receipt(json!({"reason":reason,"agent":agent}), EXIT_REFUSAL);
        }
    };

    if json_output {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "agents": agents,
                "denominator": {
                    "agents": agents.len(),
                    "states_possible": swarm::STATES_POSSIBLE,
                }
            }))
            .map_err(|_| EXIT_INVARIANT)?
        );
    } else {
        println!("SWARM STATUS");
        println!("{:<16} {:<12} {:<12} TASK", "AGENT", "ROLE", "STATE");
        for agent in &agents {
            println!(
                "{:<16} {:<12} {:<12} {}",
                agent.id, agent.role, agent.state, agent.task
            );
        }
        println!(
            "-- {} agents; {} lifecycle states possible --",
            agents.len(),
            swarm::STATES_POSSIBLE
        );
    }
    Ok(())
}

fn swarm_error(operation: &str, error: swarm::StatusError) -> Result<(), i32> {
    match error {
        swarm::StatusError::Environment(message) => {
            eprintln!("fleet: environment fault: {message}");
            Err(EXIT_ENV)
        }
        swarm::StatusError::ZeroAgents => {
            eprintln!("fleet: refusing swarm {operation}: agent store contains zero agents");
            refusal_with_receipt(
                json!({"reason":"ZERO_AGENTS","operation":operation,"checked":0,"total":0}),
                EXIT_INVARIANT,
            )
        }
        swarm::StatusError::TooFewAgents { found } => {
            eprintln!("fleet: refusing swarm {operation}: a swarm requires at least two agents; found {found}");
            refusal_with_receipt(
                json!({"reason":"TOO_FEW_AGENTS","operation":operation,"checked":found,"total":2}),
                EXIT_INVARIANT,
            )
        }
        swarm::StatusError::Invariant(message) => {
            eprintln!("fleet: refusing swarm {operation}: {message}");
            refusal_with_receipt(
                json!({"reason":"SWARM_INVARIANT","operation":operation,"detail":message}),
                EXIT_INVARIANT,
            )
        }
        swarm::StatusError::Refusal { agent, reason } => {
            eprintln!("fleet: agent {agent} refused: {reason}");
            refusal_with_receipt(
                json!({"reason":reason,"agent":agent,"operation":operation}),
                EXIT_REFUSAL,
            )
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RoutedRole {
    role: roles::Role,
    agent: String,
    model: String,
    decided_at_stage: usize,
}

/// S2: append one `lane_status` receipt (contracts/lane-status.v1.json) to the ledger. This is
/// the live status stream `keel-console` renders -- every call here is a real ledger row, not a
/// mock; the console reads them back from the same chain.jsonl `fleet swarm dispatch` writes to.
fn append_lane_status(
    role: roles::Role,
    state: &str,
    agent: Option<&str>,
    model: Option<&str>,
) -> Result<String, i32> {
    append_receipt(
        "lane_status",
        json!({
            "schema_version": "1.0",
            "lane_id": role.name(),
            "role": role.name(),
            "state": state,
            "agent": agent,
            "resolved_model": model,
        }),
        "fleet",
        model,
        None,
    )
}

fn resolve_swarm_role(
    role: roles::Role,
    state: &Path,
    builder_model: Option<&str>,
) -> Result<RoutedRole, i32> {
    let routed = route::for_plan_with_builder(role, state, builder_model)?;
    if let Some(refusal) = routed.refusal {
        eprintln!(
            "fleet: swarm dispatch: router emptied for role {} at stage {} ({}): {}\n  fix: {}",
            role.name(),
            refusal.stage,
            refusal.stage_name,
            refusal.reason,
            refusal.fix
        );
        append_receipt(
            "refusal",
            json!({"reason":"ROUTER_EMPTY","operation":"swarm dispatch","role":role.name(),"stage":refusal.stage,"stage_name":refusal.stage_name}),
            "fleet",
            None,
            Some(EXIT_REFUSAL),
        )?;
        return Err(EXIT_REFUSAL);
    }
    Ok(RoutedRole {
        role,
        agent: routed.selected_adapter.ok_or(EXIT_INVARIANT)?.to_string(),
        model: routed.resolved_model.ok_or(EXIT_INVARIANT)?.to_string(),
        decided_at_stage: routed.decided_at_stage.ok_or(EXIT_INVARIANT)?,
    })
}

fn resolve_swarm_roles(state: &Path) -> Result<Vec<RoutedRole>, i32> {
    // The verifier-independence stage needs the actual builder identity, so route Builder first.
    // Output order remains the five-role lifecycle order, not the dependency-resolution order.
    let builder = resolve_swarm_role(roles::Role::Builder, state, None)?;
    let builder_model = builder.model.clone();
    let routed = vec![
        resolve_swarm_role(roles::Role::Lead, state, None)?,
        resolve_swarm_role(roles::Role::Designer, state, None)?,
        builder,
        resolve_swarm_role(roles::Role::Verifier, state, Some(builder_model.as_str()))?,
        resolve_swarm_role(roles::Role::Meter, state, None)?,
    ];
    for selection in &routed {
        println!(
            "routed: role={} -> agent={} (resolved model: {}; decided at stage {})",
            selection.role.name(),
            selection.agent,
            selection.model,
            selection.decided_at_stage
        );
        // S2 live status stream: every routed lane starts life as "queued" in the ledger, so
        // `keel-console` has something real to read even before any lane starts executing.
        append_lane_status(
            selection.role,
            "queued",
            Some(&selection.agent),
            Some(&selection.model),
        )?;
    }
    Ok(routed)
}

fn select_swarm_builder(routed: Vec<RoutedRole>) -> Result<RoutedRole, i32> {
    routed
        .into_iter()
        .find(|selection| selection.role == roles::Role::Builder)
        .ok_or(EXIT_INVARIANT)
}

/// One role lane's real, measured execution: a worktree-isolated subprocess run through the
/// exact same `spawn_agent_with_args` re-exec path Builder already used (S3 gives the other four
/// roles the same kind of real work, not a second mechanism). `started_ms`/`ended_ms` are wall
/// clock relative to a shared `Instant` epoch so the acceptance test can assert genuine overlap,
/// not just "it didn't crash".
#[derive(Debug, Clone)]
struct LaneOutcome {
    role: roles::Role,
    agent: String,
    started_ms: u128,
    ended_ms: u128,
    status: i32,
}

fn now_ms(epoch: Instant) -> u128 {
    Instant::now().saturating_duration_since(epoch).as_millis()
}

/// Run one routed role in its own `.worktrees/<role>-<pid>-<seq>` checkout: create, spawn the
/// role's agent for real against that isolated worktree, and remove the worktree afterward --
/// on EVERY path, success or failure, so a faulted lane never leaks a worktree (S3 acceptance).
fn run_one_role_lane(repo: &Path, task: &str, routed: &RoutedRole, epoch: Instant) -> LaneOutcome {
    let started_ms = now_ms(epoch);
    let _ = append_lane_status(
        routed.role,
        "running",
        Some(&routed.agent),
        Some(&routed.model),
    );
    let name = worktree::unique_name(routed.role.name());
    let outcome = (|| -> Result<i32, i32> {
        let lane_worktree = worktree::create(repo, &name)?;
        let lane_repo = lane_worktree.path.to_string_lossy().into_owned();
        let result = spawn_agent(&routed.agent, &lane_repo, task, Some(&routed.model));
        // Cleanup runs whether the agent succeeded or failed -- a faulted lane must not leak a
        // worktree (S3 acceptance bar). The removal's own error is deliberately not allowed to
        // mask the lane's real status: report the agent's status first if we have one.
        let removed = worktree::remove(repo, &lane_worktree);
        match result {
            Ok((status, _submission)) => {
                removed?;
                Ok(status)
            }
            Err(code) => {
                let _ = removed;
                Err(code)
            }
        }
    })();
    let status = match outcome {
        Ok(status) => status,
        Err(code) => code,
    };
    let final_state = if status == EXIT_OK {
        "passed"
    } else {
        "failed"
    };
    let _ = append_lane_status(
        routed.role,
        final_state,
        Some(&routed.agent),
        Some(&routed.model),
    );
    LaneOutcome {
        role: routed.role,
        agent: routed.agent.clone(),
        started_ms,
        ended_ms: now_ms(epoch),
        status,
    }
}

/// Test-only entry point, see the `__lanes_probe` match arm's comment. Parses `--repo <path>
/// --task <text>`, builds the four non-Builder roles with the deterministic "stub" agent
/// (bypassing the real router -- this probes S3's lane execution, not route.rs), runs them
/// through the exact production `run_role_lanes_concurrently`, and prints one `lane: ...` line
/// per outcome in the same format `swarm dispatch` uses. Exits 0 only if every lane exited 0.
fn lanes_probe_command(args: &[String]) -> Result<(), i32> {
    let mut repo = None;
    let mut task = None;
    let mut index = 0usize;
    while index < args.len() {
        let value = args.get(index + 1).ok_or(EXIT_REFUSAL)?.clone();
        match args[index].as_str() {
            "--repo" => repo = Some(value),
            "--task" => task = Some(value),
            _ => return Err(EXIT_REFUSAL),
        }
        index += 2;
    }
    let repo = repo.ok_or(EXIT_REFUSAL)?;
    let task = task.ok_or(EXIT_REFUSAL)?;
    let routed: Vec<RoutedRole> = [
        roles::Role::Lead,
        roles::Role::Designer,
        roles::Role::Verifier,
        roles::Role::Meter,
    ]
    .into_iter()
    .map(|role| RoutedRole {
        role,
        agent: "stub".to_string(),
        model: "n/a".to_string(),
        decided_at_stage: 0,
    })
    .collect();
    let outcomes = run_role_lanes_concurrently(Path::new(&repo), &task, &routed);
    let mut all_ok = true;
    for lane in &outcomes {
        println!(
            "lane: role={} agent={} started_ms={} ended_ms={} exit={}",
            lane.role.name(),
            lane.agent,
            lane.started_ms,
            lane.ended_ms,
            lane.status
        );
        if lane.status != EXIT_OK {
            all_ok = false;
        }
    }
    if all_ok {
        Ok(())
    } else {
        Err(EXIT_INVARIANT)
    }
}

/// S3: run every routed role's lane concurrently, each in its own isolated worktree, capped at
/// `worktree::lane_cap()` (`min(16, available_parallelism()-2)`). Batches of `cap` lanes run via
/// `thread::scope` so each batch is guaranteed joined (no leaked threads) before the next starts.
/// Every lane is real work: a real `git worktree add`, a real re-exec of `fleet __agent ...`
/// against that isolated checkout, and a real `git worktree remove --force` -- not a milestone
/// bump standing in for execution.
fn run_role_lanes_concurrently(repo: &Path, task: &str, routed: &[RoutedRole]) -> Vec<LaneOutcome> {
    let cap = worktree::lane_cap().max(1);
    let epoch = Instant::now();
    let mut outcomes = Vec::with_capacity(routed.len());
    for batch in routed.chunks(cap) {
        let batch_results = thread::scope(|scope| {
            let handles: Vec<_> = batch
                .iter()
                .map(|role_route| {
                    scope.spawn(move || run_one_role_lane(repo, task, role_route, epoch))
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| {
                    handle.join().unwrap_or_else(|_| LaneOutcome {
                        role: roles::Role::Meter,
                        agent: "panicked".to_string(),
                        started_ms: now_ms(epoch),
                        ended_ms: now_ms(epoch),
                        status: EXIT_ENV,
                    })
                })
                .collect::<Vec<_>>()
        });
        outcomes.extend(batch_results);
    }
    outcomes
}

const SWARM_DISPATCH_USAGE: &str =
    "usage: fleet swarm dispatch --task <T> --repo <P> [--agent <A> | --role <R>]";

fn swarm_dispatch_command(args: &[String]) -> Result<(), i32> {
    let mut task = None;
    let mut repo = None;
    let mut worker: Option<String> = None;
    let mut role_flag: Option<String> = None;
    let mut index = 0usize;
    while index < args.len() {
        let key = args[index].as_str();
        let value = match args.get(index + 1) {
            Some(value) => value.clone(),
            None => {
                eprintln!("fleet: swarm dispatch: {key} needs a value.\n{SWARM_DISPATCH_USAGE}");
                return Err(EXIT_REFUSAL);
            }
        };
        match key {
            "--task" => task = Some(value),
            "--repo" => repo = Some(value),
            "--agent" => worker = Some(value),
            "--role" => role_flag = Some(value),
            _ => {
                eprintln!("fleet: swarm dispatch: unknown flag {key}.\n{SWARM_DISPATCH_USAGE}");
                return Err(EXIT_REFUSAL);
            }
        }
        index += 2;
    }
    let task = task.ok_or_else(|| {
        eprintln!("fleet: swarm dispatch: --task is required.\n{SWARM_DISPATCH_USAGE}");
        EXIT_REFUSAL
    })?;
    let repo = repo.ok_or_else(|| {
        eprintln!("fleet: swarm dispatch: --repo is required.\n{SWARM_DISPATCH_USAGE}");
        EXIT_REFUSAL
    })?;
    if task.trim().is_empty() {
        eprintln!("fleet: swarm dispatch: --task is empty; an empty string is not a task.\n{SWARM_DISPATCH_USAGE}");
        return refusal_with_receipt(
            json!({"reason":"INVALID_SWARM_DISPATCH","task":task,"repo":repo}),
            EXIT_REFUSAL,
        );
    }
    // D53: validate the cheap structural facts before stateful routing or the SOW policy. A
    // missing/non-git repo cannot be repaired by changing quota state or writing a SOW.
    let repo_path = Path::new(&repo);
    if !repo_path.is_dir() {
        eprintln!("fleet: --repo {repo} does not exist (or is not a directory).");
        return Err(EXIT_ENV);
    }
    if !repo_path.join(".git").exists() {
        eprintln!(
            "fleet: --repo {repo} is not a git repository.\n\
             fleet freezes a git diff, so the target must be a repo with at least one commit:\n\
             \n  git -C {repo} init && git -C {repo} commit --allow-empty -m init"
        );
        return Err(EXIT_ENV);
    }
    // --role opts this dispatch into five independent role decisions. The selected role still
    // chooses the one worker handed to run_with_evidence; Lead/Designer/Verifier/Meter remain
    // allocations in the existing lifecycle until the separate C8 multi-lane work is built.
    // An explicit --agent continues to override --role, and flagless dispatch still defaults to
    // stub, preserving the established acceptance path.
    //
    // state_dir() is only touched when --role is actually in play (worker.is_none() && role_flag
    // is Some) -- every --agent-driven (or flagless) call stays exactly as before, including its
    // error ordering: D53 deliberately checks the repo path before FLEET_STATE-dependent work, and
    // resolving a role unconditionally here would have reversed that for every caller, not just
    // --role ones.
    let (worker, model, lane): (String, Option<String>, Option<roles::Role>) = if worker.is_none()
        && role_flag.is_some()
    {
        let role_name = role_flag.as_deref().ok_or(EXIT_INVARIANT)?;
        if roles::Role::parse(role_name).is_none() {
            eprintln!(
                "fleet: swarm dispatch: --role {role_name:?} is not a known role.\n\
                 usage: fleet swarm dispatch --task <T> --repo <P> [--agent <A> | --role <R>]"
            );
            return refusal_with_receipt(
                json!({"reason":"UNKNOWN_ROLE","operation":"swarm dispatch","role":role_name}),
                EXIT_REFUSAL,
            );
        }
        let routed = resolve_swarm_roles(&state_dir()?)?;
        // A swarm owns all five roles. `--role` opts into their routing but must not let Lead,
        // Designer, Verifier, or Meter become the implementation worker: only Builder may write
        // code (roles::evaluate's LEAD_WROTE_CODE gate). S3/C8: the other four roles now get
        // REAL execution too -- each in its own `.worktrees/<role>-<pid>-<seq>` checkout,
        // concurrently, capped at `worktree::lane_cap()` -- instead of only a milestone bump.
        // Builder keeps its existing single-repo `run_with_evidence` flow below unchanged
        // (it is the one lane allowed to freeze a real diff against `repo_path`).
        let non_builder_roles: Vec<RoutedRole> = routed
            .iter()
            .filter(|selection| selection.role != roles::Role::Builder)
            .cloned()
            .collect();
        let lane_outcomes = run_role_lanes_concurrently(repo_path, &task, &non_builder_roles);
        for lane in &lane_outcomes {
            println!(
                "lane: role={} agent={} started_ms={} ended_ms={} exit={}",
                lane.role.name(),
                lane.agent,
                lane.started_ms,
                lane.ended_ms,
                lane.status
            );
            if lane.status != EXIT_OK {
                eprintln!(
                        "fleet: swarm dispatch: role {} lane exited {} (worktree-isolated, non-fatal to the primary Builder lane)",
                        lane.role.name(),
                        lane.status
                    );
            }
        }
        let selected = select_swarm_builder(routed)?;
        (selected.agent, Some(selected.model), Some(selected.role))
    } else {
        // S2: even a plain (flagless / --agent) dispatch owns the Builder lane. Without
        // this, an ordinary `swarm dispatch` wrote zero lane_status rows, so the console's
        // LANES view stayed empty no matter how long it was open. Defaulting the lane to
        // Builder gives every dispatch a real live status stream (queued -> running ->
        // passed/failed/refused) that `fleet console`'s Lanes view tails in real time,
        // while leaving the --role five-role stream in charge of the other four lanes.
        (
            worker.unwrap_or_else(|| "stub".to_string()),
            None,
            Some(roles::Role::Builder),
        )
    };
    if !matches!(worker.as_str(), "stub" | "freelane" | "claude" | "codex") {
        return refusal_with_receipt(
            json!({"reason":"UNKNOWN_AGENT","agent":worker}),
            EXIT_REFUSAL,
        );
    }
    enforce_accepted_sow(&task)?;
    let state = state_dir()?;
    let before = git_diff(&repo)?;
    let mut run_args = vec![
        "--task".to_string(),
        task.clone(),
        "--repo".to_string(),
        repo.clone(),
        "--agent".to_string(),
        worker.clone(),
    ];
    if let Some(tier) = &model {
        run_args.push("--model".to_string());
        run_args.push(tier.clone());
    }
    if let Some(role) = lane {
        // S2: a lane enters the ledger as "queued" (the --role path does this in
        // resolve_swarm_roles; the plain path owns its Builder lane and must emit it here too)
        // so the console shows the lane the instant the dispatch begins, before it runs.
        append_lane_status(role, "queued", Some(&worker), model.as_deref())?;
        append_lane_status(role, "running", Some(&worker), model.as_deref())?;
    }
    let evidence = match run_with_evidence(&run_args, false, Some(&before)) {
        Ok(evidence) => evidence,
        Err(code) => {
            let outcome = if code == EXIT_MISMATCH {
                swarm::EvidenceOutcome::Unknown
            } else {
                swarm::EvidenceOutcome::Faulted
            };
            let _ = swarm::dispatch_verified(
                &state,
                &task,
                &worker,
                "unknown-builder-model",
                "unknown-verifier-model",
                false,
                outcome,
            );
            if let Some(role) = lane {
                let _ = append_lane_status(role, "failed", Some(&worker), model.as_deref());
            }
            return Err(code);
        }
    };
    let evidence_outcome = match attest_verify_inner(&evidence.artifact_id, false) {
        Ok(()) => swarm::EvidenceOutcome::Verified,
        Err(code) => {
            let _ = heal_applied_artifact(
                &state,
                Path::new(&repo),
                &evidence.artifact_id,
                "failed_attestation_verification",
            );
            let outcome = if code == EXIT_MISMATCH {
                swarm::EvidenceOutcome::Unknown
            } else {
                swarm::EvidenceOutcome::Faulted
            };
            let _ = swarm::dispatch_verified(
                &state,
                &task,
                &worker,
                &evidence.builder_model,
                &evidence.verifier_model,
                false,
                outcome,
            );
            if let Some(role) = lane {
                let _ = append_lane_status(role, "failed", Some(&worker), model.as_deref());
            }
            return Err(code);
        }
    };
    if let Err(code) = ratchet::record_dispatch(
        &state,
        &evidence.artifact_id,
        evidence.score_checked,
        evidence.score_total,
    ) {
        let _ = heal_applied_artifact(
            &state,
            Path::new(&repo),
            &evidence.artifact_id,
            "below_ratchet",
        );
        let _ = swarm::dispatch_verified(
            &state,
            &task,
            &worker,
            &evidence.builder_model,
            &evidence.verifier_model,
            false,
            swarm::EvidenceOutcome::Faulted,
        );
        if let Some(role) = lane {
            let _ = append_lane_status(role, "failed", Some(&worker), model.as_deref());
        }
        return Err(code);
    }

    match swarm::dispatch_verified(
        &state,
        &task,
        &worker,
        &evidence.builder_model,
        &evidence.verifier_model,
        false,
        evidence_outcome,
    ) {
        Ok(report) if report.checked > 0 => {
            println!(
                "dispatched task={} agent={} checked={} total={} artifact={}",
                report.task, report.agent, report.checked, report.total, evidence.artifact_id
            );
            if let Some(role) = lane {
                append_lane_status(role, "passed", Some(&worker), model.as_deref())?;
            }
            Ok(())
        }
        Ok(_) => {
            if let Some(role) = lane {
                let _ = append_lane_status(role, "failed", Some(&worker), model.as_deref());
            }
            Err(EXIT_INVARIANT)
        }
        Err(error) => {
            if let Some(role) = lane {
                let state_label = if matches!(error, swarm::StatusError::Refusal { .. }) {
                    "refused"
                } else {
                    "failed"
                };
                let _ = append_lane_status(role, state_label, Some(&worker), model.as_deref());
            }
            swarm_error("dispatch", error)
        }
    }
}

const ROLE_CHECK_USAGE: &str = "usage: fleet role-check --role <lead|builder|verifier|designer|meter> [--diff-adds-code <0|1>] [--builder-model M] [--verifier-model M]";

fn role_check_command(args: &[String]) -> Result<(), i32> {
    let mut role = None;
    let mut diff_adds_code = false;
    let mut builder_model = None;
    let mut verifier_model = None;
    let mut i = 0usize;
    while i < args.len() {
        let value = match args.get(i + 1) {
            Some(value) => value.as_str(),
            None => {
                eprintln!(
                    "fleet: role-check: {} needs a value.\n{ROLE_CHECK_USAGE}",
                    args[i]
                );
                return refusal_with_receipt(
                    json!({"reason":"INVALID_ROLE_CHECK_ARGS","args":args}),
                    EXIT_REFUSAL,
                );
            }
        };
        match args[i].as_str() {
            "--role" => role = roles::Role::parse(value),
            "--diff-adds-code" => match value {
                "0" => diff_adds_code = false,
                "1" => diff_adds_code = true,
                _ => {
                    eprintln!(
                        "fleet: role-check: --diff-adds-code must be 0 or 1, got {value:?}.\n{ROLE_CHECK_USAGE}"
                    );
                    return refusal_with_receipt(
                        json!({"reason":"INVALID_ROLE_CHECK_ARGS","args":args}),
                        EXIT_REFUSAL,
                    );
                }
            },
            "--builder-model" => builder_model = Some(value),
            "--verifier-model" => verifier_model = Some(value),
            _ => {
                eprintln!(
                    "fleet: role-check: unknown flag {}.\n{ROLE_CHECK_USAGE}",
                    args[i]
                );
                return refusal_with_receipt(
                    json!({"reason":"INVALID_ROLE_CHECK_ARGS","args":args}),
                    EXIT_REFUSAL,
                );
            }
        }
        i += 2;
    }
    let role = match role {
        Some(role) => role,
        None => {
            eprintln!("fleet: role-check: --role is required.\n{ROLE_CHECK_USAGE}");
            return refusal_with_receipt(
                json!({"reason":"INVALID_ROLE_CHECK_ARGS","args":args}),
                EXIT_REFUSAL,
            );
        }
    };
    let check = roles::Check {
        role,
        diff_adds_code,
        builder_model,
        verifier_model,
    };
    match roles::evaluate(&check) {
        Ok(()) => {
            println!("ACCEPT");
            Ok(())
        }
        Err(refusal) => {
            let reason = refusal.reason();
            eprintln!("REFUSE {reason}");
            refusal_with_receipt(json!({"reason":reason}), EXIT_REFUSAL)
        }
    }
}

const RUN_USAGE: &str = "usage: fleet run --task <T> --repo <P> --agent <stub|env-probe|freelane|claude|codex>\n   or: fleet run --task <T> --repo <P> --role <lead|builder|verifier|designer|meter>\n  e.g. fleet run --task \"add a --version flag\" --repo . --agent stub\n  --role consults the same router as `fleet plan` and refuses the same way `plan` would report;\n  it never overrides an explicit --agent.";

fn run_command(args: &[String]) -> Result<(), i32> {
    let evidence = run_with_evidence(args, true, None)?;
    println!("artifact={}", evidence.artifact_id);
    Ok(())
}

struct RunEvidence {
    artifact_id: String,
    builder_model: String,
    verifier_model: String,
    score_checked: u64,
    score_total: u64,
}

/// Resolves the agent (and, when routed, the model tier) `fleet run` should spawn, given an
/// explicit `--agent` and/or `--role`. An explicit `agent` is always returned unchanged, with no
/// model tier (nothing decided it) — `role` only fills a MISSING agent by consulting the same
/// router `fleet plan` already reports, and its `resolved_model` (sonnet/opus/haiku) comes along
/// with it so the caller can thread it to the actual worker CLI. `Ok(None)` means neither was
/// given; the caller's own "--agent or --role is required" refusal handles that case.
fn resolve_run_agent(
    agent: Option<String>,
    role_flag: Option<&str>,
    state: &Path,
    announce: bool,
) -> Result<Option<(String, Option<String>)>, i32> {
    if agent.is_some() {
        if role_flag.is_some() && announce {
            println!("routed: --role ignored, --agent was given explicitly");
        }
        return Ok(agent.map(|a| (a, None)));
    }
    let Some(role_name) = role_flag else {
        return Ok(None);
    };
    let parsed_role = roles::Role::parse(role_name).ok_or_else(|| {
        eprintln!("fleet: run: --role {role_name:?} is not a known role.\n{RUN_USAGE}");
        EXIT_REFUSAL
    })?;
    let routed = route::for_plan(parsed_role, state)?;
    if let Some(refusal) = routed.refusal {
        eprintln!(
            "fleet: run: router emptied at stage {} ({}): {}\n  fix: {}",
            refusal.stage, refusal.stage_name, refusal.reason, refusal.fix
        );
        append_receipt(
            "refusal",
            json!({"reason":"ROUTER_EMPTY","role":role_name,"stage":refusal.stage,"stage_name":refusal.stage_name}),
            "fleet",
            None,
            Some(EXIT_REFUSAL),
        )?;
        return Err(EXIT_REFUSAL);
    }
    let selected = routed.selected_adapter.ok_or(EXIT_INVARIANT)?;
    let model = routed.resolved_model.ok_or(EXIT_INVARIANT)?;
    if announce {
        println!(
            "routed: role={role_name} -> agent={selected} (resolved model: {model}; decided at stage {})",
            routed.decided_at_stage.ok_or(EXIT_INVARIANT)?
        );
    }
    Ok(Some((selected.to_string(), Some(model.to_string()))))
}

fn run_with_evidence(
    args: &[String],
    announce: bool,
    require_advance_from: Option<&[u8]>,
) -> Result<RunEvidence, i32> {
    let started = Instant::now();
    let mut task: Option<String> = None;
    let mut repo: Option<String> = None;
    let mut agent: Option<String> = None;
    let mut role_flag: Option<String> = None;
    let mut model_override: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        let key = args[i].as_str();
        // The missing-VALUE and unknown-FLAG branches both exited 7 in silence. `fleet run
        // --task` (no value) is the single likeliest typo in the tool and it printed nothing.
        let val = match args.get(i + 1) {
            Some(v) => v.clone(),
            None => {
                eprintln!("fleet: run: {key} needs a value.\n{RUN_USAGE}");
                return Err(EXIT_REFUSAL);
            }
        };
        match key {
            "--task" => task = Some(val),
            "--repo" => repo = Some(val),
            "--agent" => agent = Some(val),
            "--role" => role_flag = Some(val),
            // Not documented in RUN_USAGE -- this is an internal pass-through, not a user-facing
            // flag. `swarm dispatch` already resolves --role to a concrete (agent, model) pair
            // itself before building this command line; --model is how it hands the model tier
            // through without re-running the router a second time.
            "--model" => model_override = Some(val),
            _ => {
                eprintln!("fleet: run: unknown flag {key}.\n{RUN_USAGE}");
                return Err(EXIT_REFUSAL);
            }
        }
        i += 2;
    }
    // Q1: these three `ok_or(EXIT_REFUSAL)?` lines refused with ZERO output -- `fleet run --task`
    // with a missing value simply exited 7. Found by extending Q1's surface list to `run`, which
    // is the busiest command in the tool and the last place the D40 class was still hiding.
    let task = task.ok_or_else(|| {
        eprintln!("fleet: run: --task is required.\n{RUN_USAGE}");
        EXIT_REFUSAL
    })?;
    let repo = repo.ok_or_else(|| {
        eprintln!("fleet: run: --repo is required.\n{RUN_USAGE}");
        EXIT_REFUSAL
    })?;
    let early_state = state_dir()?;
    let resolved = resolve_run_agent(agent, role_flag.as_deref(), &early_state, announce)?;
    let (agent, model) = resolved.ok_or_else(|| {
        eprintln!("fleet: run: --agent or --role is required.\n{RUN_USAGE}");
        EXIT_REFUSAL
    })?;
    let model = model_override.or(model);
    let _state = &early_state;
    // B8: this refused with ZERO bytes on stdout and stderr -- `fleet run --task "" --repo . --agent
    // stub` (task present as an empty string reaches HERE, past the `--task is required` guard
    // above, which only catches a missing flag, not an empty value) exited 7 in total silence. It
    // writes a receipt for the ledger but nothing for the human running the command.
    if task.trim().is_empty() {
        eprintln!(
            "fleet: refusing to run: --task is empty.\n\
             fleet needs a concrete task description to plan and dispatch against; an empty\n\
             string is not a task.\n\
             \n  fleet run --task \"<what you want done>\" --repo {repo} --agent {agent}"
        );
        append_receipt(
            "refusal",
            json!({"reason":"EMPTY_TASK","task":task}),
            "fleet",
            None,
            Some(EXIT_REFUSAL),
        )?;
        return Err(EXIT_REFUSAL);
    }
    // D35: this allow-list and agent_command's match disagreed. `agent_command` supports
    // "claude" | "codex" and routes to the crew adapters, but had NO dispatch arm at all, and this
    // gate refused those names from `run`. Result: fleet used NEITHER codex NOR sonnet, and the
    // whole model-adapter path was dead code that every gate passed over.
    if !matches!(
        agent.as_str(),
        "stub" | "env-probe" | "freelane" | "claude" | "codex"
    ) {
        append_receipt(
            "refusal",
            json!({"reason":"UNKNOWN_AGENT","agent":agent}),
            "fleet",
            None,
            Some(EXIT_REFUSAL),
        )?;
        return Err(EXIT_REFUSAL);
    }

    if !git_status_porcelain(Path::new(&repo))?.is_empty() {
        eprintln!(
            "fleet: refusing to run: the target repo has uncommitted changes.\n\
             fleet freezes a diff against a KNOWN starting point; a dirty tree means the\n\
             artifact would contain work fleet did not do and the attestation would claim it.\n\
             \n  git -C {repo} status     # see what is modified\n\
             git -C {repo} stash      # set it aside, or commit it\n\
             \nThis is the commonest refusal after a previous run: it left the tree modified."
        );
        refusal_with_receipt(
            json!({"reason":"TARGET_REPO_NOT_CLEAN","repo":repo}),
            EXIT_REFUSAL,
        )?;
        return Err(EXIT_REFUSAL);
    }

    let state = state_dir()?;
    let run_dir = make_run_dir(&state)?;
    let start_hash = append_receipt(
        "run_start",
        json!({"run_id": run_dir.file_name().and_then(|v| v.to_str()).ok_or(EXIT_INVARIANT)?, "agent":agent, "repo":repo, "task":task}),
        "fleet",
        None,
        None,
    )?;

    let (child_status, submission) = spawn_agent(&agent, &repo, &task, model.as_deref())?;
    if let Some(value) = submission.as_ref() {
        let bytes = serde_json::to_vec(value).map_err(|_| EXIT_INVARIANT)?;
        fs::write(run_dir.join("submission.json"), bytes).map_err(|_| EXIT_ENV)?;
    }
    if child_status != 0 {
        if let Some(reason) = submission
            .as_ref()
            .and_then(|value| value.get("body"))
            .and_then(|body| body.get("reason"))
            .and_then(Value::as_str)
        {
            eprintln!("fleet: agent {} refused: {}", agent, reason);
        }
        let _ = append_receipt(
            "run_end",
            json!({"run_id":run_dir.file_name().and_then(|v| v.to_str()).unwrap_or(""), "status":"agent_failed"}),
            "fleet",
            None,
            Some(child_status),
        );
        return Err(if child_status == EXIT_ENV {
            EXIT_ENV
        } else {
            EXIT_INVARIANT
        });
    }

    let resolved_model = submission
        .as_ref()
        .and_then(|value| value.get("body"))
        .and_then(|body| body.get("resolved_model"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let tokens = submission
        .as_ref()
        .and_then(|value| value.get("body"))
        .and_then(|body| body.get("tokens"))
        .and_then(Value::as_u64);

    let diff = git_diff(&repo)?;
    if diff.is_empty() || require_advance_from.is_some_and(|before| before == diff) {
        // D49: this wrote a receipt and printed NOTHING. A receipt is for the ledger; the operator
        // needs the reason. It is the commonest refusal in practice -- a repeat run against a repo
        // the previous run already modified -- and it looked like a silent crash. Q1 did not cover
        // `run`, so the class D40 was built to eliminate survived on the busiest surface of all.
        eprintln!(
            "fleet: refusing to freeze: the agent landed no NEW work.\n\
             The working tree is unchanged from before this run{}.\n\
             \n  Commit or discard the previous run's changes, or point --repo at a clean tree:\n\
             \n    git -C <repo> status        # see what is already modified\n\
             \n  A run that changes nothing must not produce an attested artifact.",
            if require_advance_from.is_some() {
                " (the diff is byte-identical to the prior state)"
            } else {
                ""
            }
        );
        append_receipt(
            "refusal",
            json!({"reason":"NO_WORK_LANDED","run_id":run_dir.file_name().and_then(|v| v.to_str()).ok_or(EXIT_INVARIANT)?}),
            "fleet",
            None,
            Some(EXIT_INVARIANT),
        )?;
        return Err(EXIT_INVARIANT);
    }
    let artifact_id = blake3_hex(&diff);
    let artifact_path = state.join("artifacts").join(&artifact_id);
    freeze_artifact(&artifact_path, &diff)?;
    let frozen_hash = append_receipt(
        "artifact_frozen",
        json!({"artifact_id":artifact_id,"bytes":diff.len(),"repo":repo}),
        "fleet",
        None,
        None,
    )?;
    let att_path = state
        .join("attestations")
        .join(format!("{}.json", artifact_id));
    write_json_atomic(
        &att_path,
        &json!({
            "_type":"https://in-toto.io/Statement/v1",
            "subject":[{"name":"git-diff","digest":{"blake3":artifact_id}}],
            "predicateType":"https://fleet.local/DeliveryAttestation/v1",
            "predicate":{
                "status":"verification_pending",
                "sow":{"task":task},
                "builder":{"id":agent},
                "receipts":[start_hash,frozen_hash]
            }
        }),
    )?;
    let mut healing = LandedChangeGuard::new(
        PathBuf::from(&repo),
        artifact_path.clone(),
        artifact_id.clone(),
        run_dir
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or(EXIT_INVARIANT)?
            .to_string(),
    );
    let verifier_id = verifier_for(&agent);
    let builder_model = resolved_model
        .clone()
        .unwrap_or_else(|| format!("{agent}-builder-v1"));
    let verifier_model = format!("{verifier_id}-v1");
    if agent == verifier_id {
        append_receipt(
            "refusal",
            json!({"reason":"SELF_VERIFIED","builder":agent,"verifier":verifier_id}),
            "fleet",
            None,
            Some(EXIT_INVARIANT),
        )?;
        return Err(EXIT_INVARIANT);
    }
    let verification = run_verifier(
        &agent,
        &verifier_id,
        &repo,
        &task,
        &artifact_path,
        &artifact_id,
    )?;
    let _verification_hash = append_receipt(
        "note",
        json!({
            "kind":"independent_verification",
            "builder":agent,
            "verifier":verifier_id,
            "reproduced":verification.reproduced,
            "verdict":verification.verdict
        }),
        &verifier_id,
        None,
        None,
    )?;
    ensure_builtin_oracles(&state, &artifact_id, &task, announce)?;
    let blind_suite = measure_blind_suite(&state, &task, Path::new(&repo))?;
    let blast_files = diff_files(&diff)?;
    let blast_count = u64::try_from(blast_files.len()).map_err(|_| EXIT_INVARIANT)?;
    let adequacy = measure_adequacy(Path::new(&repo), &run_dir.join("adequacy-suite.log"))?;
    let (score_checked, score_total) = match (
        adequacy.get("checked").and_then(Value::as_u64),
        adequacy.get("total").and_then(Value::as_u64),
    ) {
        (Some(checked), Some(total)) if total > 0 && checked <= total => (checked, total),
        (None, None)
            if adequacy.get("status").and_then(Value::as_str) == Some("no-measurable-surface") =>
        {
            (1, 1)
        }
        _ => return Err(EXIT_MISMATCH),
    };
    let rollback = rollback_artifact(
        Path::new(&repo),
        &artifact_path,
        &diff,
        blast_files.len() as u64,
    )?;
    let wall_ms = started.elapsed().as_millis();
    let attested_hash = append_receipt(
        "attested",
        json!({"artifact_id":artifact_id,"attestation_id":artifact_id}),
        "fleet",
        None,
        None,
    )?;
    let ended_hash = append_receipt(
        "run_end",
        json!({"run_id":run_dir.file_name().and_then(|v| v.to_str()).ok_or(EXIT_INVARIANT)?,"artifact_id":artifact_id,"status":"ok","resolved_model":resolved_model.as_deref()}),
        "fleet",
        None,
        None,
    )?;
    let attestation = json!({
        "_type":"https://in-toto.io/Statement/v1",
        "subject":[{"name":"git-diff","digest":{"blake3":artifact_id}}],
        "predicateType":"https://fleet.local/DeliveryAttestation/v1",
        "predicate":{
            "tier":"T-min",
            "elements":{
                "sow":{"task":task},
                "blind_suite":blind_suite,
                "independent_verification":{
                    "builder":agent,
                    "verifier":verifier_id,
                    "distinct":agent != verifier_id,
                    "reproduced":verification.reproduced,
                    "verdict":verification.verdict
                },
                "adequacy":adequacy,
                "blast_radius":{"files":blast_files,"count":blast_count,"source":"recorded-diff"},
                "rollback":rollback,
                "cost":{"wall_ms":wall_ms,"characters_in_diff":String::from_utf8_lossy(&diff).chars().count(),"tokens":null,"tokenizer_generation":null,"source":"observed"},
                "oracle_independence":{"status":"pending-adjudication"}
            },
            "receipts":[start_hash,frozen_hash,attested_hash,ended_hash],
            "builder":{"id":agent,"resolved_model":resolved_model.as_deref()}
        }
    });
    write_json_atomic(&att_path, &attestation)?;
    adjudicate_command_inner(&artifact_id, announce)?;
    meter::record_observation_at(&state, &agent, tokens, resolved_model.as_deref())?;
    healing.disarm();
    Ok(RunEvidence {
        artifact_id,
        builder_model,
        verifier_model,
        score_checked,
        score_total,
    })
}

struct LandedChangeGuard {
    repo: PathBuf,
    artifact_path: PathBuf,
    artifact_id: String,
    run_id: String,
    armed: bool,
}

impl LandedChangeGuard {
    fn new(repo: PathBuf, artifact_path: PathBuf, artifact_id: String, run_id: String) -> Self {
        Self {
            repo,
            artifact_path,
            artifact_id,
            run_id,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for LandedChangeGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // Healing restores the known pre-run state. It deliberately does not retry:
        // retrying would hide the failed evidence instead of healing the repository.
        let result = reverse_landed_artifact(&self.repo, &self.artifact_path);
        let (status, exit_code) = if result.is_ok() {
            ("rolled_back", EXIT_OK)
        } else {
            ("rollback_failed", result.expect_err("checked above"))
        };
        let _ = append_receipt(
            "rollback",
            json!({
                "artifact_id":self.artifact_id,
                "reason":"failed_verification_or_gate",
                "repo":self.repo,
                "status":status
            }),
            "fleet-healer",
            None,
            Some(exit_code),
        );
        let _ = append_receipt(
            "run_end",
            json!({"run_id":self.run_id,"artifact_id":self.artifact_id,"status":status}),
            "fleet-healer",
            None,
            Some(exit_code),
        );
    }
}

struct Verification {
    reproduced: bool,
    verdict: String,
}

fn verifier_for(builder: &str) -> String {
    if builder == "stub-verifier" {
        "stub".to_string()
    } else {
        "stub-verifier".to_string()
    }
}

fn ensure_builtin_oracles(
    state: &Path,
    artifact: &str,
    task: &str,
    announce: bool,
) -> Result<(), i32> {
    let oracle_root = state.join("oracles").join(artifact);
    let o1_path = oracle_root.join("o1/registration.json");
    let o2_path = oracle_root.join("o2/registration.json");
    if o1_path.exists() || o2_path.exists() {
        return Ok(());
    }
    let suite = state
        .join("tasks")
        .join(sow::id_for_task(task))
        .join("acceptance")
        .join("artifact-nonempty.sh");
    if !suite.exists() {
        if let Some(parent) = suite.parent() {
            fs::create_dir_all(parent).map_err(|_| EXIT_ENV)?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&suite)
            .map_err(|_| EXIT_ENV)?;
        file.write_all(b"#!/bin/sh\n[ -s \"$1\" ]\n")
            .map_err(|_| EXIT_ENV)?;
        file.sync_all().map_err(|_| EXIT_ENV)?;
    }
    let suite = suite.to_string_lossy().into_owned();
    let args = |slot: &str, author: &str| {
        vec![
            slot.to_string(),
            "--artifact".to_string(),
            artifact.to_string(),
            "--suite".to_string(),
            suite.clone(),
            "--author".to_string(),
            author.to_string(),
        ]
    };
    oracle_command_inner(&args("o1", "lead"), announce)?;
    oracle_command_inner(&args("o2", "verifier"), announce)?;
    Ok(())
}

fn run_verifier(
    builder_id: &str,
    verifier_id: &str,
    repo: &str,
    task: &str,
    artifact_path: &Path,
    artifact_id: &str,
) -> Result<Verification, i32> {
    if builder_id == verifier_id {
        append_receipt(
            "refusal",
            json!({"reason":"SELF_VERIFIED","builder":builder_id,"verifier":verifier_id}),
            "fleet",
            None,
            Some(EXIT_INVARIANT),
        )?;
        return Err(EXIT_INVARIANT);
    }
    let (status, submission) = spawn_verifier(verifier_id, repo, task, artifact_path)?;
    if status != EXIT_OK {
        return Err(if status == EXIT_ENV {
            EXIT_ENV
        } else {
            EXIT_INVARIANT
        });
    }
    let body = submission
        .as_ref()
        .and_then(|value| value.get("body"))
        .ok_or(EXIT_MISMATCH)?;
    if body.get("agent").and_then(Value::as_str) != Some(verifier_id)
        || body.get("artifact").and_then(Value::as_str) != Some(artifact_id)
    {
        return Err(EXIT_MISMATCH);
    }
    let reproduced = body
        .get("reproduced")
        .and_then(Value::as_bool)
        .ok_or(EXIT_MISMATCH)?;
    let verdict = body
        .get("verdict")
        .and_then(Value::as_str)
        .ok_or(EXIT_MISMATCH)?
        .to_string();
    if !reproduced || verdict != "ACCEPT" {
        return Err(EXIT_INVARIANT);
    }
    Ok(Verification {
        reproduced,
        verdict,
    })
}

fn measure_blind_suite(state: &Path, task: &str, repo: &Path) -> Result<Value, i32> {
    if task.trim().is_empty() {
        return Err(EXIT_REFUSAL);
    }
    let suite = state
        .join("tasks")
        .join(sow::id_for_task(task))
        .join("acceptance");
    fs::create_dir_all(&suite).map_err(|_| EXIT_ENV)?;
    let _suite_stat = fs::symlink_metadata(&suite).map_err(|_| EXIT_ENV)?;
    let suite_canonical = fs::canonicalize(&suite).map_err(|_| EXIT_ENV)?;
    let repo_canonical = fs::canonicalize(repo).map_err(|_| EXIT_ENV)?;
    let in_worktree_tree = suite_canonical.starts_with(&repo_canonical);
    let in_object_store = path_in_git_objects(repo, &suite_canonical)?;
    let suite_text = suite_canonical.to_string_lossy().into_owned();
    let in_env = env::vars().any(|(_, value)| value.contains(&suite_text));
    let on_any_fd = path_on_any_fd(&suite_canonical);
    let suite_hash = hash_suite(&suite_canonical)?;
    Ok(json!({
        "in_worktree_tree":in_worktree_tree,
        "in_object_store":in_object_store,
        "in_env":in_env,
        "on_any_fd":on_any_fd,
        "suite_hash":suite_hash
    }))
}

fn path_in_git_objects(repo: &Path, suite: &Path) -> Result<bool, i32> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--git-path", "objects"])
        .output()
        .map_err(|_| EXIT_ENV)?;
    if !output.status.success() {
        return Err(EXIT_INVARIANT);
    }
    let raw = String::from_utf8(output.stdout).map_err(|_| EXIT_MISMATCH)?;
    let object_path = PathBuf::from(raw.trim());
    let object_path = if object_path.is_absolute() {
        object_path
    } else {
        repo.join(object_path)
    };
    let object_path = fs::canonicalize(object_path).map_err(|_| EXIT_ENV)?;
    Ok(suite.starts_with(object_path))
}

fn path_on_any_fd(suite: &Path) -> bool {
    let Ok(entries) = fs::read_dir("/dev/fd") else {
        return false;
    };
    entries.flatten().any(|entry| {
        fs::read_link(entry.path())
            .map(|target| target == suite || target.starts_with(suite))
            .unwrap_or(false)
    })
}

fn hash_suite(path: &Path) -> Result<String, i32> {
    fn visit(path: &Path, root: &Path, bytes: &mut Vec<u8>) -> Result<(), i32> {
        let metadata = fs::symlink_metadata(path).map_err(|_| EXIT_ENV)?;
        let relative = path.strip_prefix(root).map_err(|_| EXIT_INVARIANT)?;
        bytes.extend_from_slice(relative.to_string_lossy().as_bytes());
        bytes.push(if metadata.is_dir() { b'd' } else { b'f' });
        if metadata.is_dir() {
            let mut children = fs::read_dir(path)
                .map_err(|_| EXIT_ENV)?
                .flatten()
                .map(|entry| entry.path())
                .collect::<Vec<_>>();
            children.sort();
            for child in children {
                visit(&child, root, bytes)?;
            }
        } else {
            bytes.extend_from_slice(&fs::read(path).map_err(|_| EXIT_ENV)?);
        }
        Ok(())
    }
    let mut bytes = Vec::new();
    visit(path, path, &mut bytes)?;
    Ok(blake3_hex(&bytes))
}

/// `state_dir` without the diagnostic. `doctor` reports an unset FLEET_STATE as one of its own
/// rows, so the shared error text would print twice and above the report it belongs in.
fn state_dir_quiet() -> Result<PathBuf, i32> {
    let value = env::var_os("FLEET_STATE").ok_or(EXIT_ENV)?;
    if value.is_empty() {
        return Err(EXIT_ENV);
    }
    Ok(PathBuf::from(value))
}

fn state_dir() -> Result<PathBuf, i32> {
    // D21: this used to `ok_or(EXIT_ENV)?` silently. `fleet run --repo X --agent stub` with no
    // FLEET_STATE exited 3 and printed NOTHING -- correct code, unusable surface. An exit code a
    // user cannot act on is the reachability defect (blueprint element 9), not a diagnostic.
    let value = env::var_os("FLEET_STATE").ok_or_else(|| {
        eprintln!(
            "fleet: environment fault: FLEET_STATE is not set.\n\
             fleet keeps its ledger, artifacts and receipts under that directory and will not\n\
             guess a location. Set it and retry:\n\
             \n  export FLEET_STATE=\"$PWD/var/fleet\"\n\
             \nRun `fleet doctor` to check the rest of the environment."
        );
        EXIT_ENV
    })?;
    // B8: `fleet plan` (and every other command routed through here) hit this environment fault
    // in TOTAL SILENCE -- $FLEET_STATE set but empty, or set to a path that cannot be created
    // (points at a plain file, a read-only parent, a full disk) both exited 3 with zero bytes on
    // either stream. `state_dir()` already prints when the variable is unset entirely (D21); it
    // must print just as plainly for these two related environment faults, which are just as
    // unactionable without a reason.
    if value.is_empty() {
        eprintln!(
            "fleet: environment fault: FLEET_STATE is set but empty.\n\
             \n  export FLEET_STATE=\"$PWD/var/fleet\"\n\
             \nRun `fleet doctor` to check the rest of the environment."
        );
        return Err(EXIT_ENV);
    }
    let path = PathBuf::from(value);
    for name in [
        "runs",
        "artifacts",
        "attestations",
        "ledger",
        "oracles",
        "sows",
    ] {
        let target = path.join(name);
        fs::create_dir_all(&target).map_err(|error| {
            eprintln!(
                "fleet: environment fault: could not create {} under $FLEET_STATE: {error}.\n\
                 fleet needs to create its state layout there; check that {} exists, is a\n\
                 directory (not a file), and is writable.",
                target.display(),
                path.display()
            );
            EXIT_ENV
        })?;
    }
    Ok(path)
}

fn valid_artifact_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn refusal_with_receipt(body: Value, code: i32) -> Result<(), i32> {
    append_receipt("refusal", body, "fleet", None, Some(code))?;
    Err(code)
}

fn oracle_command(args: &[String]) -> Result<(), i32> {
    oracle_command_inner(args, true)
}

fn oracle_command_inner(args: &[String], announce: bool) -> Result<(), i32> {
    let slot = args.first().map(String::as_str);
    let mut artifact: Option<String> = None;
    let mut suite: Option<String> = None;
    let mut author: Option<String> = None;
    let mut i = 1usize;
    while i < args.len() {
        let value = args.get(i + 1).cloned();
        match (args[i].as_str(), value) {
            ("--artifact", Some(value)) => artifact = Some(value),
            ("--suite", Some(value)) => suite = Some(value),
            ("--author", Some(value)) => author = Some(value),
            _ => {
                let _ = state_dir()?;
                return refusal_with_receipt(
                    json!({"reason":"INVALID_ORACLE_ARGS","args":args}),
                    EXIT_REFUSAL,
                );
            }
        }
        i += 2;
    }
    let state = state_dir()?;
    let slot = match slot {
        Some("o1") | Some("o2") => slot.ok_or(EXIT_REFUSAL)?,
        _ => return refusal_with_receipt(json!({"reason":"INVALID_ORACLE_SLOT"}), EXIT_REFUSAL),
    };
    let artifact = artifact.filter(|value| !value.trim().is_empty());
    let suite = suite.filter(|value| !value.trim().is_empty());
    let author = author.filter(|value| !value.trim().is_empty());
    let (artifact, suite, author) = match (artifact, suite, author) {
        (Some(artifact), Some(suite), Some(author)) if valid_artifact_id(&artifact) => {
            (artifact, suite, author)
        }
        _ => {
            return refusal_with_receipt(
                json!({"reason":"INVALID_ORACLE_REGISTRATION"}),
                EXIT_REFUSAL,
            )
        }
    };
    let suite = fs::canonicalize(&suite).map_err(|_| EXIT_ENV)?;
    if !suite.is_file() {
        return Err(EXIT_ENV);
    }
    let suite_bytes = fs::read(&suite).map_err(|_| EXIT_ENV)?;
    let mut registration_input = Vec::new();
    registration_input.extend_from_slice(artifact.as_bytes());
    registration_input.extend_from_slice(slot.as_bytes());
    registration_input.extend_from_slice(author.as_bytes());
    registration_input.extend_from_slice(suite.to_string_lossy().as_bytes());
    registration_input.extend_from_slice(&suite_bytes);
    let registration_hash = blake3_hex(&registration_input);
    let oracle_dir = state.join("oracles").join(&artifact).join(slot);
    fs::create_dir_all(&oracle_dir).map_err(|_| EXIT_ENV)?;
    let registration_path = oracle_dir.join("registration.json");
    let registration = json!({
        "artifact":artifact,
        "slot":slot,
        "author":author,
        "suite":suite.to_string_lossy(),
        "registration_hash":registration_hash
    });
    if registration_path.exists() {
        let existing: Value =
            serde_json::from_slice(&fs::read(&registration_path).map_err(|_| EXIT_ENV)?)
                .map_err(|_| EXIT_MISMATCH)?;
        if existing != registration {
            return Err(EXIT_INVARIANT);
        }
        if announce {
            println!(
                "registered artifact={} oracle={} author={}",
                artifact, slot, author
            );
        }
        return Ok(());
    }
    if slot == "o2" {
        let o1_path = state
            .join("oracles")
            .join(&artifact)
            .join("o1")
            .join("registration.json");
        if o1_path.exists() {
            let o1: Value = serde_json::from_slice(&fs::read(o1_path).map_err(|_| EXIT_ENV)?)
                .map_err(|_| EXIT_MISMATCH)?;
            if o1.get("author").and_then(Value::as_str) == Some(&author) {
                return refusal_with_receipt(
                    json!({"reason":"ORACLES_NOT_INDEPENDENT","artifact":artifact,"o1_author":author,"o2_author":author}),
                    EXIT_INVARIANT,
                );
            }
        }
    }
    write_json_atomic(&registration_path, &registration)?;
    fs::write(oracle_dir.join("author"), author.as_bytes()).map_err(|_| EXIT_ENV)?;
    fs::write(oracle_dir.join("suite"), suite.to_string_lossy().as_bytes())
        .map_err(|_| EXIT_ENV)?;
    if announce {
        println!(
            "registered artifact={} oracle={} author={}",
            artifact, slot, author
        );
    }
    Ok(())
}

fn read_oracle_registration(state: &Path, artifact: &str, slot: &str) -> Result<Value, i32> {
    let path = state
        .join("oracles")
        .join(artifact)
        .join(slot)
        .join("registration.json");
    let value: Value = serde_json::from_slice(&fs::read(path).map_err(|_| EXIT_REFUSAL)?)
        .map_err(|_| EXIT_MISMATCH)?;
    if value.get("artifact").and_then(Value::as_str) != Some(artifact)
        || value.get("slot").and_then(Value::as_str) != Some(slot)
        || value.get("author").and_then(Value::as_str).is_none()
        || value.get("suite").and_then(Value::as_str).is_none()
        || value
            .get("registration_hash")
            .and_then(Value::as_str)
            .is_none()
    {
        return Err(EXIT_MISMATCH);
    }
    Ok(value)
}

fn run_oracle(registration: &Value, artifact_path: &Path) -> Result<(bool, String), i32> {
    let suite = registration
        .get("suite")
        .and_then(Value::as_str)
        .ok_or(EXIT_MISMATCH)?;
    let output = Command::new(suite)
        .arg(artifact_path)
        .output()
        .map_err(|_| EXIT_ENV)?;
    let pass = output.status.success();
    let status = output
        .status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "signal".to_string());
    let mut observed = status.into_bytes();
    observed.extend_from_slice(&output.stdout);
    observed.extend_from_slice(&output.stderr);
    Ok((pass, blake3_hex(&observed)))
}

struct Quadrant {
    name: &'static str,
    fault: &'static str,
    credit: &'static str,
    code: i32,
}

fn adjudication_table(o1_pass: bool, o2_pass: bool) -> Quadrant {
    match (o1_pass, o2_pass) {
        (true, true) => Quadrant {
            name: "ACCEPT",
            fault: "NONE",
            credit: "NONE",
            code: EXIT_OK,
        },
        (true, false) => Quadrant {
            name: "ORACLE_INADEQUATE",
            fault: "LEAD",
            credit: "BUILDER",
            code: EXIT_INVARIANT,
        },
        (false, true) => Quadrant {
            name: "ORACLE_OVERCONSTRAINED",
            fault: "LEAD",
            credit: "NONE",
            code: EXIT_INVARIANT,
        },
        (false, false) => Quadrant {
            name: "BUILDER_FAULT",
            fault: "BUILDER",
            credit: "NONE",
            code: EXIT_INVARIANT,
        },
    }
}

fn adjudicate_command(artifact: &str) -> Result<(), i32> {
    adjudicate_command_inner(artifact, true)
}

fn adjudicate_command_inner(artifact: &str, announce: bool) -> Result<(), i32> {
    let state = state_dir()?;
    if !valid_artifact_id(artifact) {
        return refusal_with_receipt(
            json!({"reason":"INVALID_ARTIFACT_ID","artifact":artifact}),
            EXIT_REFUSAL,
        );
    }
    let artifact_path = state.join("artifacts").join(artifact);
    let artifact_bytes = fs::read(&artifact_path).map_err(|_| EXIT_MISMATCH)?;
    if blake3_hex(&artifact_bytes) != artifact {
        return Err(EXIT_MISMATCH);
    }
    let o1 = read_oracle_registration(&state, artifact, "o1")?;
    let o2 = read_oracle_registration(&state, artifact, "o2")?;
    let o1_author = o1
        .get("author")
        .and_then(Value::as_str)
        .ok_or(EXIT_MISMATCH)?;
    let o2_author = o2
        .get("author")
        .and_then(Value::as_str)
        .ok_or(EXIT_MISMATCH)?;
    if o1_author == o2_author {
        return refusal_with_receipt(
            json!({"reason":"ORACLES_NOT_INDEPENDENT","artifact":artifact,"o1_author":o1_author,"o2_author":o2_author}),
            EXIT_INVARIANT,
        );
    }
    let (o1_pass, o1_hash) = run_oracle(&o1, &artifact_path)?;
    let (o2_pass, o2_hash) = run_oracle(&o2, &artifact_path)?;
    let quadrant = adjudication_table(o1_pass, o2_pass);
    let receipt = append_receipt(
        "note",
        json!({
            "kind":"oracle_adjudication",
            "artifact":artifact,
            "o1_author":o1_author,
            "o2_author":o2_author,
            "o1_pass":o1_pass,
            "o2_pass":o2_pass,
            "o1_hash":o1_hash,
            "o2_hash":o2_hash,
            "quadrant":quadrant.name,
            "fault":quadrant.fault,
            "credit":quadrant.credit
        }),
        "fleet",
        None,
        Some(quadrant.code),
    )?;
    let attestation_path = state
        .join("attestations")
        .join(format!("{}.json", artifact));
    let mut attestation: Value =
        serde_json::from_slice(&fs::read(&attestation_path).map_err(|_| EXIT_MISMATCH)?)
            .map_err(|_| EXIT_MISMATCH)?;
    let elements = attestation
        .get_mut("predicate")
        .and_then(Value::as_object_mut)
        .and_then(|predicate| predicate.get_mut("elements"))
        .and_then(Value::as_object_mut)
        .ok_or(EXIT_MISMATCH)?;
    elements.insert(
        "oracle_independence".to_string(),
        json!({
            "o1_author":o1_author,
            "o2_author":o2_author,
            "o1_hash":o1_hash,
            "o2_hash":o2_hash,
            "distinct":true,
            "quadrant":quadrant.name
        }),
    );
    attestation
        .get_mut("predicate")
        .and_then(Value::as_object_mut)
        .and_then(|predicate| predicate.get_mut("receipts"))
        .and_then(Value::as_array_mut)
        .ok_or(EXIT_MISMATCH)?
        .push(Value::String(receipt));
    write_json_atomic(&attestation_path, &attestation)?;
    if announce {
        println!(
            "artifact={} quadrant={} fault={} credit={} o1={} o2={}",
            artifact,
            quadrant.name,
            quadrant.fault,
            quadrant.credit,
            if o1_pass { "pass" } else { "fail" },
            if o2_pass { "pass" } else { "fail" }
        );
    }
    if quadrant.code == EXIT_OK {
        Ok(())
    } else {
        Err(quadrant.code)
    }
}

fn make_run_dir(state: &Path) -> Result<PathBuf, i32> {
    let mut bytes = state
        .join("runs")
        .join("fleet-XXXXXX")
        .to_string_lossy()
        .into_owned()
        .into_bytes();
    // Oracle independence adjudication is implemented in this module.
    bytes.push(0);
    let ptr = unsafe { libc::mkdtemp(bytes.as_mut_ptr() as *mut libc::c_char) };
    if ptr.is_null() {
        return Err(EXIT_ENV);
    }
    bytes.pop();
    Ok(PathBuf::from(
        String::from_utf8(bytes).map_err(|_| EXIT_ENV)?,
    ))
}

fn git_diff(repo: &str) -> Result<Vec<u8>, i32> {
    let output = Command::new("git")
        .args(["-C", repo, "diff", "HEAD", "--binary"])
        .output()
        .map_err(|_| EXIT_ENV)?;
    if !output.status.success() {
        return Err(EXIT_INVARIANT);
    }
    Ok(output.stdout)
}

fn git_status_porcelain(repo: &Path) -> Result<Vec<u8>, i32> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .output()
        .map_err(|_| EXIT_ENV)?;
    if !output.status.success() {
        return Err(EXIT_INVARIANT);
    }
    Ok(output.stdout)
}

fn reverse_landed_artifact(repo: &Path, artifact_path: &Path) -> Result<(), i32> {
    let expected = fs::read(artifact_path).map_err(|_| EXIT_ENV)?;
    let current = git_diff(repo.to_str().ok_or(EXIT_ENV)?)?;
    if expected.is_empty() || current != expected {
        return Err(EXIT_MISMATCH);
    }
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["apply", "--reverse", "--binary"])
        .arg(artifact_path)
        .status()
        .map_err(|_| EXIT_ENV)?;
    if !status.success() {
        return Err(EXIT_MISMATCH);
    }
    let reset = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["reset", "--quiet", "HEAD", "--", "."])
        .status()
        .map_err(|_| EXIT_ENV)?;
    if !reset.success() {
        return Err(EXIT_MISMATCH);
    }
    let clean = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["clean", "-fd"])
        .status()
        .map_err(|_| EXIT_ENV)?;
    if !clean.success() || !git_status_porcelain(repo)?.is_empty() {
        return Err(EXIT_MISMATCH);
    }
    Ok(())
}

fn heal_applied_artifact(
    state: &Path,
    repo: &Path,
    artifact: &str,
    reason: &str,
) -> Result<(), i32> {
    reverse_landed_artifact(repo, &state.join("artifacts").join(artifact))?;
    append_receipt(
        "rollback",
        json!({
            "artifact_id":artifact,
            "reason":reason,
            "repo":repo,
            "status":"rolled_back"
        }),
        "fleet-healer",
        None,
        Some(EXIT_OK),
    )?;
    Ok(())
}

pub(crate) fn heal_ledger_artifact(artifact: &str, reason: &str) -> Result<(), i32> {
    if !valid_artifact_id(artifact) {
        return Err(EXIT_REFUSAL);
    }
    let state = state_dir()?;
    let rows = ledger_rows(true)?;
    let repo = rollback_repo(&rows, artifact).map_err(|_| EXIT_REFUSAL)?;
    heal_applied_artifact(&state, &repo, artifact, reason)
}

fn rollback_command(args: &[String]) -> Result<(), i32> {
    // Q1: this refused with a receipt and printed NOTHING to the operator. A receipt is for the
    // ledger; a human needs the next step. Caught by extending Q1's surface list to `rollback`.
    if args.is_empty() {
        eprintln!(
            "usage: fleet rollback --artifact <ID>\n\
             \n  Restores the repository to its pre-run state and records a rollback receipt.\n\
             The artifact and its attestation are PRESERVED -- the evidence of what was\n\
             attempted survives the undo.\n\
             \n  Find an id with: fleet status   or   fleet ledger dump"
        );
        return Err(EXIT_REFUSAL);
    }
    let artifact = match args {
        [flag, artifact] if flag == "--artifact" && valid_artifact_id(artifact) => artifact,
        _ => {
            return refusal_with_receipt(
                json!({"reason":"INVALID_ROLLBACK_ARGS","args":args}),
                EXIT_REFUSAL,
            )
        }
    };
    let state = state_dir()?;
    let _guard = FileLock::acquire(&state.join(format!("rollback-{}.lock", artifact)))?;
    let rows = ledger_rows(true)?;
    let repo = match rollback_repo(&rows, artifact) {
        Ok(repo) => repo,
        Err(reason) => {
            return refusal_with_receipt(
                json!({"reason":reason,"artifact_id":artifact}),
                EXIT_REFUSAL,
            )
        }
    };
    let artifact_path = state.join("artifacts").join(artifact);
    reverse_landed_artifact(&repo, &artifact_path)?;
    append_receipt(
        "rollback",
        json!({
            "artifact_id":artifact,
            "reason":"operator_requested",
            "repo":repo,
            "status":"rolled_back"
        }),
        "fleet",
        None,
        Some(EXIT_OK),
    )?;
    println!(
        "rolled_back artifact={} reason=operator_requested",
        artifact
    );
    Ok(())
}

fn rollback_repo(rows: &[Value], artifact: &str) -> Result<PathBuf, &'static str> {
    if rows.iter().any(|row| {
        row.get("event").and_then(Value::as_str) == Some("rollback")
            && row
                .get("body")
                .and_then(|body| body.get("artifact_id"))
                .and_then(Value::as_str)
                == Some(artifact)
            && row
                .get("body")
                .and_then(|body| body.get("status"))
                .and_then(Value::as_str)
                == Some("rolled_back")
    }) {
        return Err("ARTIFACT_ALREADY_ROLLED_BACK");
    }
    let repo = rows
        .iter()
        .find(|row| {
            row.get("event").and_then(Value::as_str) == Some("artifact_frozen")
                && row
                    .get("body")
                    .and_then(|body| body.get("artifact_id"))
                    .and_then(Value::as_str)
                    == Some(artifact)
        })
        .and_then(|row| row.get("body"))
        .and_then(|body| body.get("repo"))
        .and_then(Value::as_str)
        .map(PathBuf::from);
    repo.ok_or("ARTIFACT_WAS_NEVER_APPLIED")
}

struct SuiteObservation {
    passed: bool,
    checked: u64,
    total: u64,
}

fn diff_files(diff: &[u8]) -> Result<Vec<String>, i32> {
    let text = String::from_utf8(diff.to_vec()).map_err(|_| EXIT_MISMATCH)?;
    let mut files = BTreeSet::new();
    for line in text.lines() {
        let Some(paths) = line.strip_prefix("diff --git a/") else {
            continue;
        };
        let Some((old_path, new_path)) = paths.split_once(" b/") else {
            return Err(EXIT_MISMATCH);
        };
        files.insert(old_path.to_string());
        files.insert(new_path.to_string());
    }
    if files.is_empty() {
        return Err(EXIT_INVARIANT);
    }
    Ok(files.into_iter().collect())
}

fn measure_adequacy(repo: &Path, log: &Path) -> Result<Value, i32> {
    let observation = run_suite(repo, log)?;
    if observation.total == 0 {
        return Ok(json!({"status":"no-measurable-surface"}));
    }
    let interval =
        ratchet::wilson_95(observation.checked, observation.total).ok_or(EXIT_INVARIANT)?;
    Ok(json!({
        "checked": observation.checked,
        "total": observation.total,
        "rate": observation.checked as f64 / observation.total as f64,
        "wilson_95": {"lower": interval.0, "upper": interval.1},
        "source": "unit-suite"
    }))
}

fn run_suite(repo: &Path, log: &Path) -> Result<SuiteObservation, i32> {
    let stdout = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(log)
        .map_err(|_| EXIT_ENV)?;
    let stderr = stdout.try_clone().map_err(|_| EXIT_ENV)?;
    let manifest = repo.join("keel/fleet/Cargo.toml");
    let mut command = Command::new("cargo");
    command
        .args(["test", "--manifest-path"])
        .arg(manifest)
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    start_process_group(&mut command);
    let mut child = command.spawn().map_err(|_| EXIT_ENV)?;
    let status = wait_with_deadline(&mut child, 120)?;
    let _ = child.wait().map_err(|_| EXIT_ENV)?;
    let output = fs::read(log).map_err(|_| EXIT_ENV)?;
    let (checked, total) = suite_counts(&output);
    Ok(SuiteObservation {
        passed: status == EXIT_OK,
        checked,
        total,
    })
}

fn suite_counts(output: &[u8]) -> (u64, u64) {
    let text = String::from_utf8_lossy(output);
    let mut checked = 0u64;
    let mut total = 0u64;
    for line in text.lines().map(str::trim_start) {
        let Some(test) = line.strip_prefix("test ") else {
            continue;
        };
        if test.ends_with(" ... ok") {
            checked += 1;
            total += 1;
        } else if test.ends_with(" ... FAILED") {
            total += 1;
        }
    }
    (checked, total)
}

fn rollback_artifact(
    repo: &Path,
    artifact_path: &Path,
    diff: &[u8],
    reverted_files: u64,
) -> Result<Value, i32> {
    if diff.is_empty() {
        return Err(EXIT_INVARIANT);
    }
    let scratch = mktemp_dir()?;
    let result = (|| {
        let mut add_command = Command::new("git");
        add_command
            .arg("-C")
            .arg(repo)
            .args(["worktree", "add", "--detach"])
            .arg(&scratch)
            .arg("HEAD");
        let added = run_bounded(&mut add_command, 30)?;
        if added != EXIT_OK {
            return Err(EXIT_INVARIANT);
        }
        let mut apply_command = Command::new("git");
        apply_command
            .arg("-C")
            .arg(&scratch)
            .args(["apply", "--binary"])
            .arg(artifact_path);
        let applied = run_bounded(&mut apply_command, 30)?;
        if applied != EXIT_OK {
            return Err(EXIT_MISMATCH);
        }
        let applied_suite = run_suite(&scratch, &scratch.join(".fleet-suite.log"))?;
        let mut reverse_command = Command::new("git");
        reverse_command
            .arg("-C")
            .arg(&scratch)
            .args(["apply", "--reverse", "--binary"])
            .arg(artifact_path);
        let reverted = run_bounded(&mut reverse_command, 30)?;
        if reverted != EXIT_OK {
            return Err(EXIT_MISMATCH);
        }
        let reverted_suite = run_suite(&scratch, &scratch.join(".fleet-suite.log"))?;
        let mut value = json!({
            "executed": true,
            "suite_went_red": !applied_suite.passed,
            "reverted_files": reverted_files
        });
        if applied_suite.passed && reverted_suite.passed {
            value["pins_nothing"] = Value::Bool(true);
        }
        Ok(value)
    })();
    cleanup_scratch(repo, &scratch);
    result
}

fn mktemp_dir() -> Result<PathBuf, i32> {
    let output = Command::new("mktemp")
        .arg("-d")
        .output()
        .map_err(|_| EXIT_ENV)?;
    if !output.status.success() {
        return Err(EXIT_ENV);
    }
    let path = String::from_utf8(output.stdout)
        .map_err(|_| EXIT_ENV)?
        .trim()
        .to_string();
    if path.is_empty() {
        return Err(EXIT_ENV);
    }
    Ok(PathBuf::from(path))
}

fn run_bounded(command: &mut Command, seconds: u64) -> Result<i32, i32> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    start_process_group(command);
    let mut child = command.spawn().map_err(|_| EXIT_ENV)?;
    wait_with_deadline(&mut child, seconds)
}

fn start_process_group(command: &mut Command) {
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
}

fn cleanup_scratch(repo: &Path, scratch: &Path) {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo)
        .args(["worktree", "remove", "--force"])
        .arg(scratch);
    let _ = run_bounded(&mut command, 30);
    if scratch.exists() {
        let _ = fs::remove_dir_all(scratch);
    }
}

fn freeze_artifact(path: &Path, bytes: &[u8]) -> Result<(), i32> {
    if path.exists() {
        let existing = fs::read(path).map_err(|_| EXIT_ENV)?;
        if existing != bytes {
            return Err(EXIT_INVARIANT);
        }
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| EXIT_ENV)?;
    file.write_all(bytes).map_err(|_| EXIT_ENV)?;
    file.sync_all().map_err(|_| EXIT_ENV)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o444)).map_err(|_| EXIT_ENV)?;
    Ok(())
}

fn write_json_atomic(path: &Path, value: &Value) -> Result<(), i32> {
    let parent = path.parent().ok_or(EXIT_ENV)?;
    let name = path.file_name().and_then(|n| n.to_str()).ok_or(EXIT_ENV)?;
    let tmp = parent.join(format!(".{}.tmp-{}", name, std::process::id()));
    let data = serde_json::to_vec_pretty(value).map_err(|_| EXIT_INVARIANT)?;
    fs::write(&tmp, data).map_err(|_| EXIT_ENV)?;
    fs::rename(tmp, path).map_err(|_| EXIT_ENV)?;
    Ok(())
}

fn spawn_agent(
    agent: &str,
    repo: &str,
    task: &str,
    model: Option<&str>,
) -> Result<(i32, Option<Value>), i32> {
    // Position 3 (after repo, task) is a per-agent-kind slot: "stub-verifier" already uses it for
    // an artifact path (spawn_verifier, agent_command's own match arm). "claude"/"codex" claim it
    // here for the routed model tier -- the two never collide, they are different match arms.
    let mut agent_args = vec![repo.to_string(), task.to_string()];
    if let Some(tier) = model {
        agent_args.push(tier.to_string());
    }
    spawn_agent_with_args(agent, &agent_args)
}

fn spawn_verifier(
    agent: &str,
    repo: &str,
    task: &str,
    artifact_path: &Path,
) -> Result<(i32, Option<Value>), i32> {
    spawn_agent_with_args(
        agent,
        &[
            repo.to_string(),
            task.to_string(),
            artifact_path.to_string_lossy().into_owned(),
        ],
    )
}

fn spawn_agent_with_args(agent: &str, agent_args: &[String]) -> Result<(i32, Option<Value>), i32> {
    let mut fds = [0 as c_int; 2];
    let mut socket_rc =
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) };
    // Darwin does not implement AF_UNIX/SOCK_SEQPACKET. Preserve the required
    // packet path where available and use a private one-message stream fallback
    // on that platform so the same fd-3 contract remains testable.
    if socket_rc != 0 {
        socket_rc =
            unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
    }
    if socket_rc != 0 {
        return Err(EXIT_ENV);
    }
    let parent_fd = fds[0];
    let child_fd = fds[1];
    let executable = env::current_exe().map_err(|_| EXIT_ENV)?;
    let path = env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string());
    let home = env::var("HOME").ok();
    let lang = env::var("LANG").ok();
    // S3: CARGO_TARGET_DIR was missing from this allowlist. env_clear() wiped it along with
    // everything else, so any worker spawned through this path silently fell back to the crate's
    // own default `target/` dir instead of the shared, repo-external cache AGENTS.md's worktree
    // section requires -- worktree-isolated lanes would each rebuild the crate from scratch and
    // risk re-tripping the M2 build-artifact-count detector. Pass it through only when the parent
    // actually set it (never hardcode a path) so isolated lanes share one build cache.
    let cargo_target_dir = env::var("CARGO_TARGET_DIR").ok();
    let mut command = Command::new(executable);
    command.arg("__agent").arg(agent).args(agent_args);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.env_clear().env("PATH", path);
    if let Some(value) = home {
        command.env("HOME", value);
    }
    if let Some(value) = lang {
        command.env("LANG", value);
    }
    if let Some(value) = cargo_target_dir {
        command.env("CARGO_TARGET_DIR", value);
    }
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            if child_fd != 3 && libc::dup2(child_fd, 3) < 0 {
                return Err(io::Error::last_os_error());
            }
            if child_fd != 3 {
                libc::close(child_fd);
            }
            Ok(())
        });
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            unsafe {
                libc::close(parent_fd);
                libc::close(child_fd);
            }
            return Err(EXIT_ENV);
        }
    };
    unsafe {
        libc::close(child_fd);
    }
    // freelane's own network deadline is 120 seconds. Give it time to emit its typed exit-3
    // refusal instead of killing it early and misclassifying an unavailable lane as invariant 6.
    let deadline_seconds = if agent == "freelane" { 125 } else { 30 };
    let status = match wait_with_deadline(&mut child, deadline_seconds) {
        Ok(status) => status,
        Err(code) => {
            unsafe {
                libc::close(parent_fd);
            }
            eprintln!(
                "fleet: agent {agent} did not complete its non-interactive adapter run within {deadline_seconds}s. Check CLI authentication/connectivity and retry, or use `--agent freelane`."
            );
            return Err(code);
        }
    };
    let mut packet = vec![0u8; 65536];
    let received = unsafe {
        libc::recv(
            parent_fd,
            packet.as_mut_ptr() as *mut c_void,
            packet.len(),
            0,
        )
    };
    unsafe {
        libc::close(parent_fd);
    }
    if received < 0 {
        return Err(EXIT_INVARIANT);
    }
    packet.truncate(received as usize);
    if packet.is_empty() {
        eprintln!(
            "fleet: agent {agent} exited without an fd-3 result. Its non-interactive adapter failed before it could report details; check the Python crew adapter and CLI authentication, or retry with `--agent freelane`."
        );
        return Err(EXIT_INVARIANT);
    }
    let value = serde_json::from_slice(&packet).map_err(|_| {
        eprintln!(
            "fleet: agent {agent} returned an invalid fd-3 result; check the Python crew adapter, or retry with `--agent freelane`."
        );
        EXIT_INVARIANT
    })?;
    validate_submission(&value)?;
    Ok((status, Some(value)))
}

fn wait_with_deadline(child: &mut Child, seconds: u64) -> Result<i32, i32> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.code().unwrap_or(EXIT_INVARIANT)),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                terminate_group(child.id() as i32);
                let kill_deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    match child.try_wait() {
                        Ok(Some(_)) => return Err(EXIT_INVARIANT),
                        Ok(None) if Instant::now() < kill_deadline => {
                            thread::sleep(Duration::from_millis(50))
                        }
                        Ok(None) => {
                            unsafe {
                                libc::kill(-(child.id() as i32), libc::SIGKILL);
                            }
                            let _ = child.wait();
                            return Err(EXIT_INVARIANT);
                        }
                        Err(_) => return Err(EXIT_ENV),
                    }
                }
            }
            Err(_) => return Err(EXIT_ENV),
        }
    }
}

fn terminate_group(pid: i32) {
    unsafe {
        libc::kill(-pid, libc::SIGTERM);
    }
}

fn agent_command(args: &[String]) -> Result<(), i32> {
    let agent = args.first().ok_or(EXIT_REFUSAL)?;
    let repo = args.get(1).ok_or(EXIT_REFUSAL)?;
    let task = args.get(2).map(String::as_str).unwrap_or("");
    let body = match agent.as_str() {
        "stub" => {
            let path = Path::new(repo).join("main.rs");
            let mut file = OpenOptions::new()
                .append(true)
                .open(path)
                .map_err(|_| EXIT_ENV)?;
            file.write_all(b"// fleet stub change\n")
                .map_err(|_| EXIT_ENV)?;
            json!({"agent":"stub","status":"done"})
        }
        "stub-verifier" => {
            let artifact = args.get(3).ok_or(EXIT_REFUSAL)?;
            let bytes = fs::read(artifact).map_err(|_| EXIT_ENV)?;
            let artifact_id = Path::new(artifact)
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(EXIT_MISMATCH)?;
            let reproduced = blake3_hex(&bytes) == artifact_id;
            json!({
                "agent":"stub-verifier",
                "artifact":artifact_id,
                "reproduced":reproduced,
                "verdict":if reproduced { "ACCEPT" } else { "REJECT" }
            })
        }
        "env-probe" => {
            let mut values = Map::new();
            for (key, value) in env::vars() {
                values.insert(key, Value::String(value));
            }
            json!({"agent":"env-probe","environment":Value::Object(values)})
        }
        "freelane" => return run_freelane_agent(repo, task),
        "claude" | "codex" => {
            let model = args.get(3).map(String::as_str).filter(|s| !s.is_empty());
            return run_model_agent(agent, repo, task, model);
        }
        _ => return Err(EXIT_REFUSAL),
    };
    let packet = json!({"schema_version":"1.0","kind":"done","body":body});
    let bytes = serde_json::to_vec(&packet).map_err(|_| EXIT_INVARIANT)?;
    let sent = unsafe { libc::send(3, bytes.as_ptr() as *const c_void, bytes.len(), 0) };
    if sent != bytes.len() as isize {
        return Err(EXIT_INVARIANT);
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct FreelaneOutput {
    response: String,
    log: String,
    resolved_model: Option<String>,
    tokens: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
struct FreelaneFailure {
    code: i32,
    reason: String,
}

fn parse_freelane_model(log: &str) -> Option<String> {
    log.lines().find_map(|line| {
        let value = line
            .trim()
            .strip_prefix("[resolved_model=")?
            .split_once(" requested=")?
            .0;
        if value.is_empty() || value == "UNRESOLVED" {
            None
        } else {
            Some(value.to_string())
        }
    })
}

fn parse_freelane_usage(log: &str) -> Option<u64> {
    log.lines().find_map(|line| {
        let raw = line.trim().split_once(" usage=")?.1.strip_suffix(']')?;
        let usage: Value = serde_json::from_str(raw).ok()?;
        let prompt = usage.get("prompt_tokens")?.as_u64()?;
        let completion = usage.get("completion_tokens")?.as_u64()?;
        let total = usage.get("total_tokens")?.as_u64()?;
        (prompt.checked_add(completion)? == total).then_some(total)
    })
}

fn interpret_freelane_output(
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
) -> Result<FreelaneOutput, FreelaneFailure> {
    let log = String::from_utf8_lossy(&stderr).trim().to_string();
    match code {
        Some(EXIT_OK) => {}
        Some(EXIT_ENV) => {
            return Err(FreelaneFailure {
                code: EXIT_ENV,
                reason: if log.is_empty() {
                    "freelane: network lane unavailable".to_string()
                } else {
                    log
                },
            })
        }
        Some(code) => {
            return Err(FreelaneFailure {
                code: EXIT_INVARIANT,
                reason: if log.is_empty() {
                    format!("freelane: worker exited {code}")
                } else {
                    log
                },
            })
        }
        None => {
            return Err(FreelaneFailure {
                code: EXIT_INVARIANT,
                reason: "freelane: worker terminated without an exit code".to_string(),
            })
        }
    }
    let response = String::from_utf8(stdout).map_err(|_| FreelaneFailure {
        code: EXIT_INVARIANT,
        reason: "freelane: response was not UTF-8".to_string(),
    })?;
    if response.trim().is_empty() {
        return Err(FreelaneFailure {
            code: EXIT_INVARIANT,
            reason: "freelane: successful invocation returned an empty response".to_string(),
        });
    }
    Ok(FreelaneOutput {
        resolved_model: parse_freelane_model(&log),
        tokens: parse_freelane_usage(&log),
        response,
        log,
    })
}

fn send_agent_packet(kind: &str, body: Value) -> Result<(), i32> {
    let packet = json!({"schema_version":"1.0","kind":kind,"body":body});
    let bytes = serde_json::to_vec(&packet).map_err(|_| EXIT_INVARIANT)?;
    let sent = unsafe { libc::send(3, bytes.as_ptr() as *const c_void, bytes.len(), 0) };
    if sent != bytes.len() as isize {
        return Err(EXIT_INVARIANT);
    }
    Ok(())
}

fn run_freelane_agent(repo: &str, task: &str) -> Result<(), i32> {
    if task.trim().is_empty() {
        return Err(EXIT_REFUSAL);
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("bin/freelane.sh");
    let output = match Command::new(&script)
        .arg(task)
        .current_dir(repo)
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            send_agent_packet(
                "refuse",
                json!({
                    "agent":"freelane",
                    "reason":format!("freelane: cannot launch keyless lane: {error}")
                }),
            )?;
            return Err(EXIT_ENV);
        }
    };
    let result = match interpret_freelane_output(output.status.code(), output.stdout, output.stderr)
    {
        Ok(result) => result,
        Err(failure) => {
            send_agent_packet(
                "refuse",
                json!({"agent":"freelane","reason":failure.reason}),
            )?;
            return Err(failure.code);
        }
    };

    // The keyless lane has no tool protocol: its textual answer is its work product. Preserve it
    // verbatim in the submission and as comments in the tracked fixture so the normal Fleet path
    // freezes a real, prompt-sensitive Git diff rather than fabricating a successful no-op.
    let path = Path::new(repo).join("main.rs");
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|_| EXIT_ENV)?;
    // D47: this used to comment out EVERY line of the model's reply and call the result an
    // implementation. The diff was non-empty, applied cleanly, froze, and its attestation
    // verified -- every structural check passed over 97 added lines containing ZERO lines of
    // code. A non-empty diff is a proxy; a diff that implements something is the property.
    // Refuse rather than land commented-out prose that looks like work.
    // The FIRST version of this guard filtered `//`-prefixed lines out of the model's RAW reply
    // -- but the reply is prose and the `//` is added below, so every prose line counted as code
    // and the guard never fired. It tested the text after the wrong transformation.
    // What actually distinguishes an implementation from an explanation is a fenced code block
    // or diff markers, so look for those in the reply as received.
    let has_fence = result.response.contains("```");
    let has_diff_marker = result
        .response
        .lines()
        .any(|l| l.starts_with("--- ") || l.starts_with("+++ ") || l.starts_with("@@"));
    if !has_fence && !has_diff_marker {
        eprintln!(
            "fleet: agent freelane produced no implementation: the reply contained {} lines and\n\
             it contained no fenced code block and no diff markers, so it is an explanation,\n\
             not a change. Landing it would freeze an attested artifact that implements nothing.\n\
             Use a model that emits a patch, or --agent stub for a deterministic run.",
            result.response.lines().count()
        );
        return Err(EXIT_REFUSAL);
    }
    // D47: this used to comment out EVERY line of the reply -- including the fenced code block --
    // and call the result an implementation. The diff was non-empty, applied, froze, and its
    // attestation verified over 97 added lines containing ZERO executable lines. The model had
    // returned working code all along; fleet was throwing it away and freezing the wrapper.
    // Extract the code; keep the prose as a comment beside it, not instead of it.
    let mut code = String::new();
    let mut in_fence = false;
    for line in result.response.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            code.push_str(line);
            code.push('\n');
        }
    }
    file.write_all(b"\n// fleet freelane output (prose elided; extracted code follows)\n")
        .map_err(|_| EXIT_ENV)?;
    file.write_all(code.as_bytes()).map_err(|_| EXIT_ENV)?;
    file.sync_all().map_err(|_| EXIT_ENV)?;

    send_agent_packet(
        "done",
        json!({
            "agent":"freelane",
            "status":"done",
            "diff":result.response,
            "log":result.log,
            "resolved_model":result.resolved_model,
            "tokens":result.tokens
        }),
    )
}

fn which_on_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}

/// `fleet plan "<free text>"` — the non-interactive door to the intent router.
///
/// The router shipped reachable only from the tty REPL, so no gate could drive it and no user
/// could script it: reachable by a human, unreachable by a test. This prints the plan and exits
/// WITHOUT side effects, so it is safe to call from the acceptance suite.
fn plan_command(args: &[String]) -> Result<(), i32> {
    let lines = plan_command_lines(args, state_dir)?;
    for line in lines {
        println!("{line}");
    }
    Ok(())
}

fn plan_command_lines(
    args: &[String],
    state: impl FnOnce() -> Result<PathBuf, i32>,
) -> Result<Vec<String>, i32> {
    let prompt = args.join(" ");
    if prompt.trim().is_empty() {
        eprintln!("usage: fleet plan \"<what you want done>\"");
        return Err(EXIT_REFUSAL);
    }
    match intent::classify(&prompt) {
        Ok(kind) => {
            let prefixes = kind.command_prefixes();
            let (agent_id, required_skills) = kind.agent_and_skills();
            let agents = agent::Registry::load_default()?;
            let skills = fleet::skills::Registry::load_default()?;
            skills.require_for_agent(&agents, agent_id, required_skills)?;
            let mut lines = Vec::new();
            lines.push(format!("intent: {}", kind.name()));
            lines.push(format!("agent: {agent_id}"));
            lines.push(format!("skills: {}", required_skills.join(", ")));
            let role = roles::Role::parse(agent_id).ok_or(EXIT_INVARIANT)?;
            if matches!(kind, intent::Intent::Change | intent::Intent::Diagnose) {
                // A PLAN IS NOT AN EXECUTION. Wiring the router in made `fleet plan` refuse
                // whenever no lane could be picked, so a user with no quota configured could not
                // even see the plan. Planning and running are different questions and the planner
                // was answering the wrong one. Report routing status as part of the plan; the gate
                // that must refuse is `run`, and it still does.
                //
                // S2b: `route::for_plan` can return a hard `Err` (not a `Decision` carrying
                // `refusal`) when the router's own inputs are unusable. Only a genuine
                // environment fault — fresh state with no meter windows configured yet — may
                // degrade to "could not be evaluated"; the plan still names its commands and only
                // `run` may actually refuse. EXIT_INVARIANT (a corrupt meter, untyped state) and
                // every other typed error must PROPAGATE: a plan built on a silently swallowed
                // invariant is a plan the user will run straight into the burned state. The old
                // catch-all `Err(_)` made a corrupt meter-v1.tsv indistinguishable from "no meter
                // windows yet" — both printed the plan and exited 0.
                match route::for_plan(role, &state()?) {
                    Ok(routed) => {
                        if let Some(refusal) = routed.refusal {
                            lines.push(format!(
                                "routed lane: UNAVAILABLE — route emptied at stage {} ({}): {}",
                                refusal.stage, refusal.stage_name, refusal.reason
                            ));
                            lines.push(format!("  fix before running: {}", refusal.fix));
                        } else {
                            lines.push(format!(
                                "routed lane: {} (resolved model: {}; decided at stage {})",
                                routed.selected_adapter.ok_or(EXIT_INVARIANT)?,
                                routed.resolved_model.ok_or(EXIT_INVARIANT)?,
                                routed.decided_at_stage.ok_or(EXIT_INVARIANT)?
                            ));
                        }
                    }
                    Err(EXIT_ENV) => {
                        lines.push(
                            "routed lane: UNAVAILABLE — routing could not be evaluated (see the environment fault printed above)."
                                .to_string(),
                        );
                        lines.push(
                            "  fix before running: resolve the fault above, then re-run `fleet plan` to see the routed lane."
                                .to_string(),
                        );
                    }
                    Err(code) => return Err(code),
                }
            } else {
                lines
                    .push("routed lane: local (decided at stage 2: no model dispatch)".to_string());
            }
            lines.push(format!(
                "commands: {} planned (denominator: {})",
                prefixes.len(),
                prefixes.len()
            ));
            for (i, prefix) in prefixes.iter().enumerate() {
                lines.push(format!("  {}. fleet {}", i + 1, prefix.join(" ")));
            }
            lines.push(
                "note: plan only — nothing was executed. Run the commands above, or use the REPL to confirm."
                    .to_string(),
            );
            Ok(lines)
        }
        Err(near) => {
            // Never silently pick one. Name the closest matches and refuse.
            eprintln!("fleet: no committed intent matches: {prompt:?}");
            if near.is_empty() {
                eprintln!("  no near matches; `fleet --help` lists every command.");
            } else {
                eprintln!("  closest intents ({} candidates):", near.len());
                for n in near {
                    eprintln!("    {n}");
                }
            }
            Err(EXIT_REFUSAL)
        }
    }
}

fn run_model_agent(agent: &str, repo: &str, task: &str, model: Option<&str>) -> Result<(), i32> {
    if task.trim().is_empty() {
        eprintln!("fleet: agent {agent}: refusing an empty task");
        return Err(EXIT_REFUSAL);
    }
    // D35: this whole path was unreachable (no `Some("agent")` arm, and `run`'s allow-list
    // refused claude|codex), so it had never been exercised and failed SILENTLY once routed.
    // A required CLI must be reported by name, not as a bare exit code.
    let cli = if agent == "claude" { "claude" } else { "codex" };
    if which_on_path(cli).is_none() {
        eprintln!(
            "fleet: agent {agent}: the `{cli}` CLI is not on PATH.\n\
             fleet drives model agents through their own authenticated CLIs and will not\n\
             substitute another. Install it, or use `--agent freelane` (keyless) or\n\
             `--agent stub` (deterministic, no model)."
        );
        return Err(EXIT_ENV);
    }
    let bridge = r#"
import json
import os
import sys
import tempfile
from pathlib import Path

agent, repo, task = sys.argv[1:4]
model = sys.argv[4] if len(sys.argv) > 4 and sys.argv[4] else None

def send(kind, body):
    packet = json.dumps({"schema_version": "1.0", "kind": kind, "body": body}).encode()
    sent = os.write(3, packet)
    if sent != len(packet):
        raise RuntimeError("fd 3 short write")

try:
    from crew.adapters import ClaudeAdapter, CodexAdapter
    adapter_type = ClaudeAdapter if agent == "claude" else CodexAdapter
    adapter = adapter_type(model=model)
except Exception as exc:
    send("refuse", {"agent": agent, "reason": "Python crew adapter failed to load: " + str(exc) + "; repair the crew environment or use --agent freelane"})
    raise SystemExit(3)

with tempfile.TemporaryDirectory(prefix="fleet-runscratch-") as directory:
    root = Path(directory)
    try:
        result = adapter.invoke(
            task,
            stdout_path=root / "transcript.stdout",
            stderr_path=root / "transcript.stderr",
            cwd=Path(repo),
            diff_path=root / "git.diff",
        )
    except Exception as exc:
        send("refuse", {"agent": agent, "reason": str(exc)})
        raise SystemExit(getattr(exc, "exit_code", 6))

    if result.returncode == 3:
        send("refuse", {"agent": agent, "reason": "HARNESS_CLI_UNAVAILABLE"})
        raise SystemExit(3)
    if result.returncode != 0:
        send("refuse", {"agent": agent, "reason": "HARNESS_EXIT", "returncode": result.returncode})
        raise SystemExit(result.returncode)
    usage = result.usage
    tokens = usage.total_tokens if usage is not None else None
    if tokens is None and usage is not None and usage.input_tokens is not None and usage.output_tokens is not None:
        tokens = usage.input_tokens + usage.output_tokens
    send("done", {"agent": agent, "status": "done", "requested_model": model, "resolved_model": result.resolved_model, "tokens": tokens})
"#;
    let python = env::var("PYTHON").unwrap_or_else(|_| "python3".to_string());
    let crew_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("crew");
    let mut command = Command::new(python);
    command
        .arg("-c")
        .arg(bridge)
        .args([agent, repo, task, model.unwrap_or("")])
        .env("PYTHONPATH", crew_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let status = command.status().map_err(|_| EXIT_ENV)?;
    match status.code() {
        Some(EXIT_OK) => Ok(()),
        Some(code) => Err(code),
        None => Err(EXIT_INVARIANT),
    }
}

fn validate_submission(value: &Value) -> Result<(), i32> {
    let object = value.as_object().ok_or(EXIT_INVARIANT)?;
    if object.get("schema_version") != Some(&Value::String("1.0".to_string())) {
        return Err(EXIT_INVARIANT);
    }
    match object.get("kind").and_then(Value::as_str) {
        Some("note") | Some("done") | Some("refuse") => {}
        _ => return Err(EXIT_INVARIANT),
    }
    if !object.get("body").is_some_and(Value::is_object) {
        return Err(EXIT_INVARIANT);
    }
    Ok(())
}

fn ledger_command(args: &[String]) -> Result<(), i32> {
    match args.first().map(String::as_str) {
        Some("verify") if args.len() == 1 => ledger_verify(),
        Some("count") if args.len() == 1 => {
            let rows = ledger_rows(false)?;
            println!("{}", rows.len());
            Ok(())
        }
        Some("dump") if args.len() == 1 => ledger_dump(),
        Some("append") => ledger_append_command(&args[1..]),
        _ => {
            eprintln!("usage: fleet ledger <verify|count|dump|append --event E --body J>");
            Err(EXIT_REFUSAL)
        }
    }
}

// S2 contract governance: a lane status is a PROJECTION of a ledger receipt — the authored body
// (schema_version, lane_id, role, state, agent, resolved_model) plus the immutable envelope
// stamps (seq, hash, actor, ts_wall) folded into `ledger_ref` and `ts_wall`. `fleet contract
// lane-status validate` re-assembles every `lane_status` row in the live ledger into its
// projection and validates it against contracts/lane-status.v1.json, publishing the
// denominator so the purer READ path (the console) and any consumer have a real, non-vacuous
// check that the projection is well-formed.
fn contract_command(args: &[String]) -> Result<(), i32> {
    if args == ["lane-status", "validate"] || args == ["lane_status", "validate"] {
        return contract_lane_status_validate();
    }
    eprintln!("usage: fleet contract lane-status validate   # validate every lane_status projection in the ledger");
    Err(EXIT_REFUSAL)
}

fn contract_lane_status_validate() -> Result<(), i32> {
    let rows = ledger_rows(false)?;
    validate_lane_status_rows(&rows)
}

// Assemble + validate every lane_status projection in a set of ledger rows, publishing the
// denominator. Split out from the subcommand so the counting logic is unit-testable without
// touching FLEET_STATE.
fn validate_lane_status_rows(rows: &[Value]) -> Result<(), i32> {
    let mut checked = 0u64;
    let mut valid = 0u64;
    let mut total = 0u64;
    let mut failures = Vec::new();
    for row in rows.iter() {
        let Some(object) = row.as_object() else {
            continue;
        };
        let event = object
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if event != "lane_status" {
            continue;
        }
        total += 1;
        // Assemble the projection exactly as the console does.
        let Some(body) = object.get("body").and_then(Value::as_object) else {
            failures.push(format!("seq {}: no body", seq_or_unknown(object)));
            continue;
        };
        let seq = object.get("seq").and_then(Value::as_u64);
        let hash = object.get("hash").and_then(Value::as_str);
        let actor = object.get("actor").and_then(Value::as_str);
        let ts_wall = object.get("ts_wall").and_then(Value::as_str);
        match (seq, hash, actor, ts_wall) {
            (Some(seq), Some(hash), Some(actor), Some(ts_wall)) => {
                let mut projection = body.clone();
                projection.insert(
                    "ledger_ref".to_string(),
                    json!({ "seq": seq, "hash": hash }),
                );
                // ts_wall and actor are already envelope-level; the projection surfaces them at
                // the top level so a consumer never has to haunt both objects.
                projection.insert("ts_wall".to_string(), json!(ts_wall));
                projection.insert("actor".to_string(), json!(actor));
                checked += 1;
                match validate_lane_status_projection(&projection) {
                    Ok(()) => valid += 1,
                    Err(reason) => failures.push(format!("seq {seq} ({hash}): {reason}")),
                }
            }
            _ => failures.push(format!(
                "seq {}: envelope missing seq/hash/actor/ts_wall",
                seq_or_unknown(object)
            )),
        }
    }
    if checked == 0 {
        // A projection check that examined zero lane inputs is vacuous. With a denominator and
        // no input, that is a failure, never a silent pass.
        eprintln!(
            "lanes: invalid (checked={checked}, total={total}, valid={valid}) — no lane_status rows to validate"
        );
        return Err(EXIT_MISMATCH);
    }
    for failure in &failures {
        eprintln!("lanes: {failure}");
    }
    if valid == total {
        println!("lanes: valid (checked={checked}, total={total})");
        Ok(())
    } else {
        println!("lanes: INVALID (checked={checked}, total={total}, valid={valid})");
        Err(EXIT_MISMATCH)
    }
}

fn seq_or_unknown(object: &Map<String, Value>) -> String {
    object
        .get("seq")
        .and_then(Value::as_u64)
        .map(|v| v.to_string())
        .unwrap_or_else(|| "?".to_string())
}

// Validate a projected lane-status object against the fields a consumer must be able to rely on.
// This mirrors the schema's required set and enums so the console's projection path has a real,
// runnable check rather than a hand-waved "schema says so".
fn validate_lane_status_projection(projection: &Map<String, Value>) -> Result<(), String> {
    if projection.get("schema_version").and_then(Value::as_str) != Some("1.0") {
        return Err("schema_version is not 1.0".to_string());
    }
    if projection
        .get("lane_id")
        .and_then(Value::as_str)
        .is_none_or(|s| s.is_empty())
    {
        return Err("lane_id is missing or empty".to_string());
    }
    let role_ok = ["lead", "designer", "builder", "verifier", "meter"].contains(
        &projection
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| "role is missing".to_string())?,
    );
    if !role_ok {
        return Err("role is not one of the five swarm roles".to_string());
    }
    let state_ok = ["queued", "running", "passed", "failed", "refused"].contains(
        &projection
            .get("state")
            .and_then(Value::as_str)
            .ok_or_else(|| "state is missing".to_string())?,
    );
    if !state_ok {
        return Err("state is not one of queued/running/passed/failed/refused".to_string());
    }
    let Some(ledger_ref) = projection.get("ledger_ref").and_then(Value::as_object) else {
        return Err("ledger_ref is missing (must be stamped at projection time)".to_string());
    };
    if ledger_ref.get("seq").and_then(Value::as_u64).is_none() {
        return Err("ledger_ref.seq is missing".to_string());
    }
    if !valid_hash(
        ledger_ref
            .get("hash")
            .and_then(Value::as_str)
            .ok_or_else(|| "ledger_ref.hash is missing".to_string())?,
    ) {
        return Err("ledger_ref.hash is not a valid blake3 hash".to_string());
    }
    Ok(())
}

fn status_command(json_output: bool) -> Result<(), i32> {
    let (chain, _) = ledger_paths()?;
    let rows = if chain.exists() {
        ledger_rows(true)?
    } else {
        Vec::new()
    };
    if !rows.is_empty() {
        verify_rows(&rows)?;
    }
    status::command(&rows, json_output, |artifact| {
        attest_verify_inner(artifact, false).is_ok()
    })
}

fn ledger_append_command(args: &[String]) -> Result<(), i32> {
    let mut event: Option<String> = None;
    let mut body: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        let val = args.get(i + 1).ok_or(EXIT_REFUSAL)?.clone();
        match args[i].as_str() {
            "--event" => event = Some(val),
            "--body" => body = Some(val),
            _ => return Err(EXIT_REFUSAL),
        }
        i += 2;
    }
    let event = event.ok_or(EXIT_REFUSAL)?;
    let body: Value = serde_json::from_str(&body.ok_or(EXIT_REFUSAL)?).map_err(|_| EXIT_REFUSAL)?;
    if !body.is_object() {
        return Err(EXIT_REFUSAL);
    }
    let hash = append_receipt(&event, body, "fleet-ledger", None, None)?;
    println!("{}", hash);
    Ok(())
}

fn ledger_paths() -> Result<(PathBuf, PathBuf), i32> {
    let state = state_dir()?;
    Ok((
        state.join("ledger/chain.jsonl"),
        state.join("ledger/chain.lock"),
    ))
}

struct FileLock(File);
impl FileLock {
    fn acquire(path: &Path) -> Result<Self, i32> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|_| EXIT_ENV)?;
        file.lock_exclusive().map_err(|_| EXIT_ENV)?;
        Ok(Self(file))
    }
}
impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

fn ledger_rows(allow_empty: bool) -> Result<Vec<Value>, i32> {
    let (chain, lock) = ledger_paths()?;
    let _guard = FileLock::acquire(&lock)?;
    let rows = read_rows(&chain)?;
    if !allow_empty && rows.is_empty() {
        return Err(EXIT_INVARIANT);
    }
    Ok(rows)
}

fn read_rows(path: &Path) -> Result<Vec<Value>, i32> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|_| EXIT_ENV)?;
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str(line).map_err(|_| EXIT_MISMATCH)?);
    }
    Ok(rows)
}

fn append_receipt(
    event: &str,
    body: Value,
    actor: &str,
    resolved_model: Option<&str>,
    exit_code: Option<i32>,
) -> Result<String, i32> {
    let (chain, lock) = ledger_paths()?;
    let _guard = FileLock::acquire(&lock)?;
    let rows = read_rows(&chain)?;
    if !rows.is_empty() {
        verify_rows(&rows)?;
    }
    let stored_event = if event == "note" {
        "gate_verdict"
    } else {
        event
    };
    if ![
        "run_start",
        "artifact_frozen",
        "attested",
        "refusal",
        "gate_verdict",
        "rollback",
        "run_end",
        "lane_status",
    ]
    .contains(&stored_event)
    {
        return Err(EXIT_REFUSAL);
    }
    let previous = match rows.last() {
        Some(row) => row
            .get("hash")
            .and_then(Value::as_str)
            .ok_or(EXIT_MISMATCH)?
            .to_string(),
        None => "GENESIS".to_string(),
    };
    let seq = rows.len() as u64;
    let mut row = Map::new();
    row.insert("schema_version".to_string(), json!("1.0"));
    row.insert("seq".to_string(), json!(seq));
    row.insert("prev_hash".to_string(), json!(previous));
    row.insert("ts_wall".to_string(), Value::String(now_rfc3339()?));
    row.insert("event".to_string(), Value::String(stored_event.to_string()));
    row.insert("actor".to_string(), Value::String(actor.to_string()));
    row.insert(
        "resolved_model".to_string(),
        resolved_model.map_or(Value::Null, |v| Value::String(v.to_string())),
    );
    row.insert(
        "exit_code".to_string(),
        exit_code.map_or(Value::Null, |v| json!(v)),
    );
    row.insert("body".to_string(), body);
    let canonical = serde_json::to_vec(&Value::Object(row.clone())).map_err(|_| EXIT_INVARIANT)?;
    let mut hash_input = previous.as_bytes().to_vec();
    hash_input.extend_from_slice(&canonical);
    let hash = format!("blake3:{}", blake3_hex(&hash_input));
    row.insert("hash".to_string(), Value::String(hash.clone()));
    let line = serde_json::to_string(&Value::Object(row)).map_err(|_| EXIT_INVARIANT)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&chain)
        .map_err(|_| EXIT_ENV)?;
    file.write_all(line.as_bytes()).map_err(|_| EXIT_ENV)?;
    file.write_all(b"\n").map_err(|_| EXIT_ENV)?;
    file.sync_data().map_err(|_| EXIT_ENV)?;
    Ok(hash)
}

fn ledger_verify() -> Result<(), i32> {
    let rows = ledger_rows(false)?;
    // B8: this printed NOTHING on the failure path. `fleet ledger verify` on a tampered chain
    // exited 8 in total silence -- the success path prints `verified checked=N total=N`, but the
    // one path the whole product's trust story rests on said nothing at all. `verify_rows` (and
    // the `validate_receipt_row` it calls) now name the specific row and the specific reason
    // before returning the mismatch, so this `?` always propagates past at least one eprintln.
    verify_rows(&rows)?;
    println!("verified checked={} total={}", rows.len(), rows.len());
    Ok(())
}

fn verify_rows(rows: &[Value]) -> Result<(), i32> {
    let mut previous = "GENESIS".to_string();
    let mut seen_prev = std::collections::HashSet::new();
    for (expected_seq, row) in rows.iter().enumerate() {
        let object = row.as_object().ok_or_else(|| {
            eprintln!("fleet: ledger corrupt: row {expected_seq} is not a JSON object");
            EXIT_MISMATCH
        })?;
        validate_receipt_row(expected_seq, object)?;
        let seq = object.get("seq").and_then(Value::as_u64).ok_or_else(|| {
            eprintln!("fleet: ledger corrupt: row {expected_seq} has no numeric seq");
            EXIT_MISMATCH
        })?;
        if seq != expected_seq as u64 {
            eprintln!(
                "fleet: ledger corrupt: row {expected_seq} carries seq={seq} (expected {expected_seq}) -- the chain has been reordered or a row was removed"
            );
            return Err(EXIT_MISMATCH);
        }
        let prev = object
            .get("prev_hash")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                eprintln!("fleet: ledger corrupt: row {expected_seq} has no prev_hash");
                EXIT_MISMATCH
            })?;
        if prev != previous {
            eprintln!(
                "fleet: ledger corrupt: row {expected_seq} prev_hash={prev} does not match the prior row's hash={previous} -- the chain link is broken (tampered or reordered)"
            );
            return Err(EXIT_MISMATCH);
        }
        if !seen_prev.insert(prev.to_string()) {
            eprintln!(
                "fleet: ledger corrupt: row {expected_seq} reuses prev_hash={prev}, which another row already claimed -- the chain forks"
            );
            return Err(EXIT_MISMATCH);
        }
        let supplied = object.get("hash").and_then(Value::as_str).ok_or_else(|| {
            eprintln!("fleet: ledger corrupt: row {expected_seq} has no hash");
            EXIT_MISMATCH
        })?;
        let mut without_hash = object.clone();
        without_hash.remove("hash");
        let canonical = serde_json::to_vec(&Value::Object(without_hash)).map_err(|_| {
            eprintln!("fleet: ledger corrupt: row {expected_seq} could not be re-serialized to verify its hash");
            EXIT_MISMATCH
        })?;
        let mut input = prev.as_bytes().to_vec();
        input.extend_from_slice(&canonical);
        let expected = format!("blake3:{}", blake3_hex(&input));
        if supplied != expected {
            eprintln!(
                "fleet: ledger corrupt: row {expected_seq} hash={supplied} does not match its recomputed content hash -- this row's content was tampered with after it was written"
            );
            return Err(EXIT_MISMATCH);
        }
        previous = supplied.to_string();
    }
    Ok(())
}

fn validate_receipt_row(seq_for_message: usize, object: &Map<String, Value>) -> Result<(), i32> {
    const ALLOWED: [&str; 9] = [
        "schema_version",
        "seq",
        "prev_hash",
        "hash",
        "ts_wall",
        "event",
        "actor",
        "resolved_model",
        "exit_code",
    ];
    for key in object.keys() {
        if key != "body" && !ALLOWED.contains(&key.as_str()) {
            eprintln!(
                "fleet: ledger corrupt: row {seq_for_message} has an unexpected field {key:?}"
            );
            return Err(EXIT_MISMATCH);
        }
    }
    if object.get("schema_version").and_then(Value::as_str) != Some("1.0")
        || object.get("seq").and_then(Value::as_u64).is_none()
        || object.get("ts_wall").and_then(Value::as_str).is_none()
        || object.get("actor").and_then(Value::as_str).is_none()
        || !object.get("body").is_some_and(Value::is_object)
    {
        eprintln!(
            "fleet: ledger corrupt: row {seq_for_message} is missing or has an invalid schema_version/seq/ts_wall/actor/body"
        );
        return Err(EXIT_MISMATCH);
    }
    let prev = object
        .get("prev_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            eprintln!("fleet: ledger corrupt: row {seq_for_message} has no prev_hash");
            EXIT_MISMATCH
        })?;
    if !valid_hash_or_genesis(prev) {
        eprintln!(
            "fleet: ledger corrupt: row {seq_for_message} prev_hash={prev:?} is not GENESIS and not a valid blake3 hash"
        );
        return Err(EXIT_MISMATCH);
    }
    let hash = object.get("hash").and_then(Value::as_str).ok_or_else(|| {
        eprintln!("fleet: ledger corrupt: row {seq_for_message} has no hash");
        EXIT_MISMATCH
    })?;
    if !valid_hash(hash) {
        eprintln!(
            "fleet: ledger corrupt: row {seq_for_message} hash={hash:?} is not a valid blake3 hash"
        );
        return Err(EXIT_MISMATCH);
    }
    let event = object.get("event").and_then(Value::as_str).ok_or_else(|| {
        eprintln!("fleet: ledger corrupt: row {seq_for_message} has no event");
        EXIT_MISMATCH
    })?;
    if ![
        "run_start",
        "artifact_frozen",
        "attested",
        "refusal",
        "gate_verdict",
        "rollback",
        "run_end",
        "lane_status",
    ]
    .contains(&event)
    {
        eprintln!("fleet: ledger corrupt: row {seq_for_message} has an unknown event {event:?}");
        return Err(EXIT_MISMATCH);
    }
    if let Some(value) = object.get("resolved_model") {
        if !value.is_null() && !value.is_string() {
            eprintln!(
                "fleet: ledger corrupt: row {seq_for_message} resolved_model is neither null nor a string"
            );
            return Err(EXIT_MISMATCH);
        }
    }
    if let Some(value) = object.get("exit_code") {
        if !value.is_null() && value.as_i64().is_none() {
            eprintln!(
                "fleet: ledger corrupt: row {seq_for_message} exit_code is neither null nor an integer"
            );
            return Err(EXIT_MISMATCH);
        }
    }
    Ok(())
}

fn valid_hash(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("blake3:")
        && value[7..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn valid_hash_or_genesis(value: &str) -> bool {
    value == "GENESIS" || valid_hash(value)
}

fn ledger_dump() -> Result<(), i32> {
    let (chain, lock) = ledger_paths()?;
    let _guard = FileLock::acquire(&lock)?;
    if chain.exists() {
        let mut file = File::open(chain).map_err(|_| EXIT_ENV)?;
        let mut text = String::new();
        file.read_to_string(&mut text).map_err(|_| EXIT_ENV)?;
        print!("{}", text);
    }
    Ok(())
}

fn attest_verify(id: &str) -> Result<(), i32> {
    attest_verify_inner(id, true)
}

fn attest_verify_inner(id: &str, announce: bool) -> Result<(), i32> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(EXIT_MISMATCH);
    }
    let state = state_dir().map_err(|_| EXIT_MISMATCH)?;
    let artifact = fs::read(state.join("artifacts").join(id)).map_err(|_| EXIT_MISMATCH)?;
    if blake3_hex(&artifact) != id {
        return Err(EXIT_MISMATCH);
    }
    let attestation: Value = serde_json::from_slice(
        &fs::read(state.join("attestations").join(format!("{}.json", id)))
            .map_err(|_| EXIT_MISMATCH)?,
    )
    .map_err(|_| EXIT_MISMATCH)?;
    let root = attestation.as_object().ok_or(EXIT_MISMATCH)?;
    if root.get("_type").and_then(Value::as_str) != Some("https://in-toto.io/Statement/v1")
        || root.get("predicateType").and_then(Value::as_str)
            != Some("https://fleet.local/DeliveryAttestation/v1")
    {
        return Err(EXIT_MISMATCH);
    }
    if root.len() != 4 {
        return Err(EXIT_MISMATCH);
    }
    let subjects = root
        .get("subject")
        .and_then(Value::as_array)
        .ok_or(EXIT_MISMATCH)?;
    if subjects.len() != 1 {
        return Err(EXIT_MISMATCH);
    }
    let subject = subjects[0].as_object().ok_or(EXIT_MISMATCH)?;
    if subject.len() != 2 || subject.get("name").and_then(Value::as_str) != Some("git-diff") {
        return Err(EXIT_MISMATCH);
    }
    let digest = subject
        .get("digest")
        .and_then(Value::as_object)
        .ok_or(EXIT_MISMATCH)?;
    if digest.len() != 1 || digest.get("blake3").and_then(Value::as_str) != Some(id) {
        return Err(EXIT_MISMATCH);
    }
    let predicate = root
        .get("predicate")
        .and_then(Value::as_object)
        .ok_or(EXIT_MISMATCH)?;
    if predicate.len() != 4 || predicate.get("tier").and_then(Value::as_str) != Some("T-min") {
        return Err(EXIT_MISMATCH);
    }
    let elements = predicate
        .get("elements")
        .and_then(Value::as_object)
        .ok_or(EXIT_MISMATCH)?;
    let oracle_element = elements
        .get("oracle_independence")
        .cloned()
        .ok_or(EXIT_MISMATCH)?;
    let oracle_is_real = oracle_element.as_object().is_some_and(|oracle| {
        oracle.len() == 6
            && oracle.get("o1_author").and_then(Value::as_str).is_some()
            && oracle.get("o2_author").and_then(Value::as_str).is_some()
            && oracle
                .get("o1_hash")
                .and_then(Value::as_str)
                .is_some_and(valid_artifact_id)
            && oracle
                .get("o2_hash")
                .and_then(Value::as_str)
                .is_some_and(valid_artifact_id)
            && oracle.get("distinct") == Some(&Value::Bool(true))
            && oracle
                .get("quadrant")
                .and_then(Value::as_str)
                .is_some_and(|quadrant| {
                    [
                        "ACCEPT",
                        "ORACLE_INADEQUATE",
                        "ORACLE_OVERCONSTRAINED",
                        "BUILDER_FAULT",
                    ]
                    .contains(&quadrant)
                })
    });
    if !oracle_is_real {
        return Err(EXIT_MISMATCH);
    }
    let blind_suite = elements
        .get("blind_suite")
        .and_then(Value::as_object)
        .ok_or(EXIT_MISMATCH)?;
    if blind_suite.len() != 5
        || blind_suite
            .get("in_worktree_tree")
            .and_then(Value::as_bool)
            .is_none()
        || blind_suite
            .get("in_object_store")
            .and_then(Value::as_bool)
            .is_none()
        || blind_suite.get("in_env").and_then(Value::as_bool).is_none()
        || blind_suite
            .get("on_any_fd")
            .and_then(Value::as_bool)
            .is_none()
        || blind_suite
            .get("suite_hash")
            .and_then(Value::as_str)
            .is_none()
    {
        return Err(EXIT_MISMATCH);
    }
    let independent = elements
        .get("independent_verification")
        .and_then(Value::as_object)
        .ok_or(EXIT_MISMATCH)?;
    if independent.len() != 5
        || independent.get("builder").and_then(Value::as_str).is_none()
        || independent
            .get("verifier")
            .and_then(Value::as_str)
            .is_none()
        || independent.get("distinct") != Some(&Value::Bool(true))
        || independent.get("reproduced").and_then(Value::as_bool) != Some(true)
        || independent.get("verdict").and_then(Value::as_str) != Some("ACCEPT")
    {
        return Err(EXIT_MISMATCH);
    }
    let expected_elements = json!({
        "sow":{"task":null},
        "blind_suite":elements.get("blind_suite").ok_or(EXIT_MISMATCH)?,
        "independent_verification":elements.get("independent_verification").ok_or(EXIT_MISMATCH)?,
        "adequacy":elements.get("adequacy").ok_or(EXIT_MISMATCH)?,
        "blast_radius":elements.get("blast_radius").ok_or(EXIT_MISMATCH)?,
        "rollback":elements.get("rollback").ok_or(EXIT_MISMATCH)?,
        "cost":elements.get("cost").ok_or(EXIT_MISMATCH)?,
        "oracle_independence":oracle_element
    });
    if elements.len() != 8 || !elements.contains_key("oracle_independence") {
        return Err(EXIT_MISMATCH);
    }
    validate_measured_elements(elements, &artifact)?;
    let receipt_ids = predicate
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or(EXIT_MISMATCH)?;
    if receipt_ids.len() < 4 {
        return Err(EXIT_MISMATCH);
    }
    let rows = ledger_rows(false).map_err(|_| EXIT_MISMATCH)?;
    verify_rows(&rows).map_err(|_| EXIT_MISMATCH)?;
    let mut by_hash = std::collections::HashMap::new();
    for row in rows {
        if let Some(hash) = row.get("hash").and_then(Value::as_str) {
            by_hash.insert(hash.to_string(), row);
        }
    }
    let expected_events = ["run_start", "artifact_frozen", "attested", "run_end"];
    let mut selected = Vec::new();
    for (index, receipt) in receipt_ids.iter().enumerate() {
        if index >= 4 {
            break;
        }
        let hash = receipt.as_str().ok_or(EXIT_MISMATCH)?;
        let row = by_hash.get(hash).ok_or(EXIT_MISMATCH)?;
        let event = row
            .get("event")
            .and_then(Value::as_str)
            .ok_or(EXIT_MISMATCH)?;
        if event != expected_events[index] {
            return Err(EXIT_MISMATCH);
        }
        selected.push(row);
    }
    for receipt in receipt_ids.iter().skip(4) {
        let hash = receipt.as_str().ok_or(EXIT_MISMATCH)?;
        let row = by_hash.get(hash).ok_or(EXIT_MISMATCH)?;
        if row.get("event").and_then(Value::as_str) != Some("gate_verdict")
            || row
                .get("body")
                .and_then(Value::as_object)
                .and_then(|body| body.get("kind"))
                .and_then(Value::as_str)
                != Some("oracle_adjudication")
            || row
                .get("body")
                .and_then(Value::as_object)
                .and_then(|body| body.get("artifact"))
                .and_then(Value::as_str)
                != Some(id)
        {
            return Err(EXIT_MISMATCH);
        }
    }
    let start = selected[0]
        .get("body")
        .and_then(Value::as_object)
        .ok_or(EXIT_MISMATCH)?;
    let frozen = selected[1]
        .get("body")
        .and_then(Value::as_object)
        .ok_or(EXIT_MISMATCH)?;
    let attested = selected[2]
        .get("body")
        .and_then(Value::as_object)
        .ok_or(EXIT_MISMATCH)?;
    let ended = selected[3]
        .get("body")
        .and_then(Value::as_object)
        .ok_or(EXIT_MISMATCH)?;
    let run_id = start
        .get("run_id")
        .and_then(Value::as_str)
        .ok_or(EXIT_MISMATCH)?;
    if start.get("task").and_then(Value::as_str).is_none()
        || start.get("agent").and_then(Value::as_str).is_none()
        || start.get("repo").and_then(Value::as_str).is_none()
        || frozen.get("artifact_id").and_then(Value::as_str) != Some(id)
        || frozen.get("bytes").and_then(Value::as_u64) != Some(artifact.len() as u64)
        || attested.get("artifact_id").and_then(Value::as_str) != Some(id)
        || attested.get("attestation_id").and_then(Value::as_str) != Some(id)
        || ended.get("run_id").and_then(Value::as_str) != Some(run_id)
        || ended.get("artifact_id").and_then(Value::as_str) != Some(id)
        || ended.get("status").and_then(Value::as_str) != Some("ok")
    {
        return Err(EXIT_MISMATCH);
    }
    let mut expected_sow = expected_elements;
    expected_sow["sow"] = json!({"task":start.get("task").ok_or(EXIT_MISMATCH)?});
    if Value::Object(elements.clone()) != expected_sow {
        return Err(EXIT_MISMATCH);
    }
    if predicate
        .get("builder")
        .and_then(Value::as_object)
        .and_then(|v| v.get("id"))
        .and_then(Value::as_str)
        != independent.get("builder").and_then(Value::as_str)
    {
        return Err(EXIT_MISMATCH);
    }
    if announce {
        println!("verified artifact={}", id);
    }
    Ok(())
}

fn validate_measured_elements(elements: &Map<String, Value>, artifact: &[u8]) -> Result<(), i32> {
    let adequacy = elements.get("adequacy").ok_or(EXIT_MISMATCH)?;
    if adequacy.get("status").and_then(Value::as_str) == Some("no-measurable-surface") {
        if adequacy.as_object().is_none_or(|value| value.len() != 1) {
            return Err(EXIT_MISMATCH);
        }
    } else {
        let value = adequacy.as_object().ok_or(EXIT_MISMATCH)?;
        if value.len() != 5 || value.get("source").and_then(Value::as_str) != Some("unit-suite") {
            return Err(EXIT_MISMATCH);
        }
        let checked = value
            .get("checked")
            .and_then(Value::as_u64)
            .ok_or(EXIT_MISMATCH)?;
        let total = value
            .get("total")
            .and_then(Value::as_u64)
            .ok_or(EXIT_MISMATCH)?;
        if total == 0 || checked > total || value.get("rate").and_then(Value::as_f64).is_none() {
            return Err(EXIT_MISMATCH);
        }
        let interval = value
            .get("wilson_95")
            .and_then(Value::as_object)
            .ok_or(EXIT_MISMATCH)?;
        if interval.len() != 2
            || interval.get("lower").and_then(Value::as_f64).is_none()
            || interval.get("upper").and_then(Value::as_f64).is_none()
        {
            return Err(EXIT_MISMATCH);
        }
    }

    let files = diff_files(artifact)?;
    let blast = elements
        .get("blast_radius")
        .and_then(Value::as_object)
        .ok_or(EXIT_MISMATCH)?;
    if blast.len() != 3
        || blast.get("files") != Some(&json!(files))
        || blast.get("count").and_then(Value::as_u64) != Some(files.len() as u64)
        || blast.get("source").and_then(Value::as_str) != Some("recorded-diff")
    {
        return Err(EXIT_MISMATCH);
    }

    let rollback = elements
        .get("rollback")
        .and_then(Value::as_object)
        .ok_or(EXIT_MISMATCH)?;
    if rollback.len() < 3
        || rollback.len() > 4
        || rollback.get("executed") != Some(&Value::Bool(true))
        || rollback
            .get("suite_went_red")
            .and_then(Value::as_bool)
            .is_none()
        || rollback.get("reverted_files").and_then(Value::as_u64) != Some(files.len() as u64)
    {
        return Err(EXIT_MISMATCH);
    }
    if let Some(pins_nothing) = rollback.get("pins_nothing") {
        if pins_nothing != &Value::Bool(true) {
            return Err(EXIT_MISMATCH);
        }
    }

    let cost = elements
        .get("cost")
        .and_then(Value::as_object)
        .ok_or(EXIT_MISMATCH)?;
    let characters = String::from_utf8_lossy(artifact).chars().count() as u64;
    if cost.len() != 5
        || cost.get("wall_ms").and_then(Value::as_u64).is_none()
        || cost.get("characters_in_diff").and_then(Value::as_u64) != Some(characters)
        || !cost.get("tokens").is_some_and(Value::is_null)
        || !cost.get("tokenizer_generation").is_some_and(Value::is_null)
        || cost.get("source").and_then(Value::as_str) != Some("observed")
    {
        return Err(EXIT_MISMATCH);
    }
    Ok(())
}

fn now_rfc3339() -> Result<String, i32> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| EXIT_ENV)?;
    let seconds = duration.as_secs() as libc::time_t;
    let mut tm = unsafe { std::mem::zeroed::<libc::tm>() };
    if unsafe { libc::gmtime_r(&seconds, &mut tm) }.is_null() {
        return Err(EXIT_ENV);
    }
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    ))
}

// Use the maintained BLAKE3 crate for the unkeyed 32-byte hash path used by
// artifacts, receipts, and attestations.
fn blake3_hex(input: &[u8]) -> String {
    blake3::hash(input).to_hex().to_string()
}

fn print_help() {
    println!(
        r#"fleet — a local macOS harness that takes a task to a frozen, attested change.

USAGE
  fleet                 Enter the interactive REPL when attached to a terminal.
  fleet --print [<command> ...]
                        Run without entering the REPL, even on a terminal.
  fleet <command> [options]

COMMANDS
  sow --task <T>        Build a complete SOW with crew.sow; exit 9 awaiting human review.
  sow accept --id <ID> Record who accepted the exact task-bound SOW and when.
  swarm dispatch --task <T> --repo <P> [--agent <A> | --role <R>]
                        Allocate role work; build with stub by default; verify and score.
  run --task <T> --repo <P> --agent <stub|env-probe|freelane|claude|codex> | --role <R>
                        Run a task; freeze the diff; emit an attestation.
                        e.g. fleet run --task "add --version" --repo . --agent stub
  oracle o1|o2 --artifact <ID> --suite <P> --author <NAME>
                        Register an independent oracle outside the worktree.
  adjudicate --artifact <ID>
                        Run both registered oracles and apply the 2x2 table.
  attest verify <ID>    Recompute an attestation from its receipts. Exit 8 on mismatch.
                        e.g. fleet attest verify 0094d223...
  status [--json]       Human task rollup derived from receipts. e.g. fleet status
  rollback --artifact <ID>
                        Restore the clean pre-run repository state; preserve all evidence.
  ledger verify         Verify the hash chain.       e.g. fleet ledger verify
  ledger append --event <E> --body <JSON>
                        Append a receipt.            e.g. fleet ledger append --event note --body '{{}}'
  ledger count | dump   Row count / raw JSONL.       e.g. fleet ledger count
  ratchet show|check    The quality bar; refuses a regression naming both values.
                        e.g. fleet ratchet show
  plan "<free text>"    Say what you want in English; fleet names the intent, agent, skills,
                        routed lane and the commands to run. Executes nothing.
                        e.g. fleet plan "add a --version flag"
  meter show|reserve|plan
                        Measured token usage per lane. Unknown is null, never 0.
  lifecycle states|show|advance
                        The typed SDLC: 15 states, illegal transitions do not compile.
  graph                 Module reachability over the dispatch surface.
  completions <shell>   Shell completions for bash, zsh or fish.
  roles                 List roles, owned gates, and implementation-write permission.
  route --role <ROLE>   Select an installed, quota-eligible model with six auditable stages.
  role-check [options]  Check role separation and implementation-write policy.
  agents list           Registry + scorecards.       e.g. fleet agents list
  skills [--check]      Resolve agent skills; --check fails on unresolved declarations.
  impact --symbol <S>   Blast radius for a symbol.   e.g. fleet impact --symbol die
  console               Operator TUI.                e.g. fleet console
  contract lane-status validate
                        Validate every lane_status projection in the ledger.
  mcp <repo> <lease>    Serve lease-scoped tools.    e.g. fleet mcp . 'keel/**'
  doctor                Health check.                e.g. fleet doctor
  --version | --help

INTERACTIVE
  Free text is planned as a task. The plan is shown before execution:
  Enter runs · e edits/regenerates · Esc cancels.
  /help /status /agents /swarm /run /lifecycle /meter /ratchet
  /ledger /doctor /clear /exit

EXIT CODES
  0 ok · 3 environment fault · 6 invariant violation · 7 refusal · 8 verification mismatch
  9 SOW ready and awaiting human review

STATE
  $FLEET_STATE (default ~/.local/state/fleet)"#
    );
}

fn doctor() -> Result<(), i32> {
    let mut pass = 0usize;
    let mut fail = 0usize;
    let mut worst = 0i32;
    let mut row = |name: &str, ok: bool, detail: String, code: i32| {
        if ok {
            pass += 1;
            println!("  ok    {name:<28} {detail}");
        } else {
            fail += 1;
            if code > worst {
                worst = code;
            }
            println!("  FAIL  {name:<28} {detail}");
        }
    };

    // D22: `doctor` used to `state_dir()?` and bail. That made the one command you run BECAUSE
    // your environment is broken refuse to run BECAUSE your environment is broken -- and its own
    // advice was "Run `fleet doctor`". doctor must diagnose a missing FLEET_STATE as a finding and
    // keep checking everything else; it is the only subcommand for which an unset state dir is
    // the expected input rather than a fault.
    let state = match state_dir_quiet() {
        Ok(dir) => dir,
        Err(_) => {
            row(
                "FLEET_STATE set",
                false,
                "unset -- export FLEET_STATE=\"$PWD/var/fleet\"".to_string(),
                EXIT_ENV,
            );
            PathBuf::new()
        }
    };
    let exists = !state.as_os_str().is_empty() && state.is_dir();
    row(
        "state dir exists",
        exists,
        state.display().to_string(),
        EXIT_ENV,
    );
    let writable = exists
        && fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(state.join(".doctor-probe"))
            .is_ok();
    if writable {
        let _ = fs::remove_file(state.join(".doctor-probe"));
    }
    row("state dir writable", writable, String::new(), EXIT_ENV);

    let chain = state.join("ledger").join("chain.jsonl");
    if chain.exists() {
        let rows = fs::read_to_string(&chain)
            .map(|c| c.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0);
        row("ledger chain", true, format!("{rows} receipts"), 0);
    } else {
        row(
            "ledger chain",
            true,
            "absent (no runs yet — not an error)".into(),
            0,
        );
    }

    let artifacts = state.join("artifacts");
    let n = fs::read_dir(&artifacts).map(|d| d.count()).unwrap_or(0);
    row("artifact store", true, format!("{n} frozen artifacts"), 0);

    for tool in ["git", "cargo"] {
        let out = Command::new(tool).arg("--version").output();
        match out {
            Ok(o) if o.status.success() => row(
                &format!("tool: {tool}"),
                true,
                String::from_utf8_lossy(&o.stdout).trim().to_string(),
                0,
            ),
            _ => row(
                &format!("tool: {tool}"),
                false,
                "not found or failed --version".into(),
                EXIT_ENV,
            ),
        }
    }

    let total = pass + fail;
    println!("-- {pass} of {total} checks passed --");
    if fail == 0 {
        Ok(())
    } else {
        Err(worst)
    }
}

fn print_completions(shell: &str) {
    // Hand-written rather than clap_complete: the CLI is a hand-rolled matcher, so generating
    // from a clap App we do not have would be a second source of truth (A4). One list, here.
    const CMDS: &[&str] = &[
        "run",
        "oracle",
        "adjudicate",
        "attest",
        "rollback",
        "ledger",
        "roles",
        "route",
        "role-check",
        "agents",
        "ratchet",
        "status",
        "console",
        "graph",
        "impact",
        "mcp",
        "doctor",
        "completions",
        "help",
    ];
    match shell {
        "zsh" => {
            println!("#compdef fleet");
            println!("_fleet() {{ _arguments '1:command:({})' }}", CMDS.join(" "));
            println!("compdef _fleet fleet");
        }
        "fish" => {
            for c in CMDS {
                println!("complete -c fleet -n __fish_use_subcommand -a {c}");
            }
        }
        _ => {
            println!(
                "_fleet() {{ COMPREPLY=($(compgen -W \"{}\" -- \"${{COMP_WORDS[1]}}\")); }}",
                CMDS.join(" ")
            );
            println!("complete -F _fleet fleet");
        }
    }
}

// ---------------------------------------------------------------------------------------
// F08: `Accepted -> Proposed` -- emit a real pull request carrying the attested diff.
//
// The typed edge and its gates live in `fleet::lifecycle` (`Task<Accepted>::propose`,
// `AttestationBundle`, `ChangeEmitter`). Everything below is the bin-side driver: real I/O
// (reading the artifact/attestation off disk, running `attest_verify_inner`, shelling out to
// `git`/`gh`) that the lib crate must not do itself (lifecycle.rs cannot depend on main.rs;
// see `ChangeEmitter`'s doc comment in lifecycle.rs).
// ---------------------------------------------------------------------------------------

const PR_EMIT_USAGE: &str =
    "usage: fleet pr emit --task <ID> --artifact <ARTIFACT_ID> --repo <PATH> --base <BRANCH> [--head <BRANCH>]";

fn pr_emit_command(args: &[String]) -> Result<(), i32> {
    let mut task = None;
    let mut artifact = None;
    let mut repo = None;
    let mut base = None;
    let mut head = None;
    let mut i = 0usize;
    while i < args.len() {
        let key = args[i].as_str();
        let val = match args.get(i + 1) {
            Some(v) => v.clone(),
            None => {
                eprintln!("fleet: pr emit: {key} needs a value.\n{PR_EMIT_USAGE}");
                return Err(EXIT_REFUSAL);
            }
        };
        match key {
            "--task" => task = Some(val),
            "--artifact" => artifact = Some(val),
            "--repo" => repo = Some(val),
            "--base" => base = Some(val),
            "--head" => head = Some(val),
            _ => {
                eprintln!("fleet: pr emit: unknown flag {key}.\n{PR_EMIT_USAGE}");
                return Err(EXIT_REFUSAL);
            }
        }
        i += 2;
    }
    let task = task.ok_or_else(|| {
        eprintln!("fleet: pr emit: --task is required.\n{PR_EMIT_USAGE}");
        EXIT_REFUSAL
    })?;
    let artifact = artifact.ok_or_else(|| {
        eprintln!("fleet: pr emit: --artifact is required.\n{PR_EMIT_USAGE}");
        EXIT_REFUSAL
    })?;
    let repo = repo.ok_or_else(|| {
        eprintln!("fleet: pr emit: --repo is required.\n{PR_EMIT_USAGE}");
        EXIT_REFUSAL
    })?;
    let base = base.ok_or_else(|| {
        eprintln!("fleet: pr emit: --base is required.\n{PR_EMIT_USAGE}");
        EXIT_REFUSAL
    })?;
    pr_emit_run(&task, &artifact, &repo, &base, head.as_deref())
}

/// The one production path both `fleet pr emit` and the test-only `__pr_emit_probe` drive
/// (F08 contract §4.4), so the acceptance test exercises real code, not a test-only
/// shortcut.
fn pr_emit_run(
    task: &str,
    artifact_id: &str,
    repo: &str,
    base: &str,
    head: Option<&str>,
) -> Result<(), i32> {
    let state = state_dir()?;
    let task_id = lifecycle::TaskId::new(task).map_err(|error| {
        eprintln!("fleet: pr emit: {error}");
        EXIT_REFUSAL
    })?;

    // Step 1 (§4.4): the cheap precondition first -- refuse before touching the attestation.
    let persisted = lifecycle::load_state(&state, &task_id).map_err(|error| {
        eprintln!("fleet: pr emit: {error}");
        EXIT_ENV
    })?;
    if persisted != "Accepted" {
        return pr_emit_refuse(
            "NOT_ACCEPTED",
            &format!("persisted state is {persisted}, not Accepted"),
            None,
        );
    }

    // Steps 2/3: re-derive the digest ourselves with the reused primitive (`blake3_hex`)
    // before doing anything else -- a tampered artifact must never reach the slower checks.
    if !valid_artifact_id(artifact_id) {
        return pr_emit_refuse(
            "ARTIFACT_DIGEST_MISMATCH",
            "--artifact is not a valid 64-hex blake3 digest",
            None,
        );
    }
    let artifact_path = state.join("artifacts").join(artifact_id);
    let diff = match fs::read(&artifact_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return pr_emit_refuse(
                "ARTIFACT_DIGEST_MISMATCH",
                "the artifact file could not be read",
                None,
            )
        }
    };
    if blake3_hex(&diff) != artifact_id {
        return pr_emit_refuse(
            "ARTIFACT_DIGEST_MISMATCH",
            "blake3(bytes on disk) does not match the artifact id",
            None,
        );
    }

    // Step 2 proper: the existing, real, full check -- reused, not re-implemented.
    let attest_verify_ok = attest_verify_inner(artifact_id, false).is_ok();

    // Step 4: build the bundle used both to name a missing element and as the propose
    // edge's own (compiled, unbypassable) gate.
    let attestation_path = state.join("attestations").join(format!("{artifact_id}.json"));
    let attestation: Value = match fs::read(&attestation_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    {
        Some(value) => value,
        None => {
            return pr_emit_refuse(
                "INCOMPLETE_ATTESTATION",
                "no attestation found for this artifact",
                Some("attestation"),
            )
        }
    };
    let elements = attestation
        .get("predicate")
        .and_then(Value::as_object)
        .and_then(|predicate| predicate.get("elements"))
        .cloned()
        .unwrap_or(Value::Null);
    let bundle = lifecycle::AttestationBundle::new(elements);

    if let Some(missing) = bundle.missing_element() {
        return pr_emit_refuse(
            "INCOMPLETE_ATTESTATION",
            &format!("missing required element: {missing}"),
            Some(missing),
        );
    }
    if !attest_verify_ok {
        // The bundle's own (shallower, filesystem-free) check found nothing missing, but the
        // real validator disagrees -- e.g. a measured element (blast_radius/cost) that does
        // not match the actual diff bytes. Refuse anyway; attest_verify_inner is the
        // authority, the bundle is only for naming.
        return pr_emit_refuse(
            "INCOMPLETE_ATTESTATION",
            "attest_verify_inner rejected this attestation",
            None,
        );
    }
    if diff.is_empty() {
        return pr_emit_refuse(
            "EMPTY_DIFF",
            "a run that changes nothing must not produce an attested artifact",
            None,
        );
    }

    let head = head
        .map(str::to_string)
        .unwrap_or_else(|| format!("fleet/{task}"));
    let title = format!("fleet: {task}");
    let body = build_pr_body(&attestation, artifact_id);
    let request = lifecycle::ProposalRequest {
        repo: PathBuf::from(repo),
        base: base.to_string(),
        head,
        artifact_id: artifact_id.to_string(),
        diff,
        title,
        body,
    };
    let emitter = RealChangeEmitter;
    match lifecycle::propose_change(&state, task_id, &bundle, &request, &emitter) {
        Ok(change) => {
            println!(
                "pr_emit: state=Proposed branch={} pr_url={} commit={} changed_files={}",
                change.head, change.url, change.commit, change.changed_files
            );
            Ok(())
        }
        Err(refusal) => {
            let missing = if refusal.code() == "INCOMPLETE_ATTESTATION" {
                bundle.missing_element()
            } else {
                None
            };
            pr_emit_refuse(refusal.code(), refusal.message(), missing)
        }
    }
}

fn pr_emit_refuse(code: &str, message: &str, missing: Option<&str>) -> Result<(), i32> {
    match missing {
        Some(missing) => println!("pr_emit_refused: code={code} missing={missing}"),
        None => println!("pr_emit_refused: code={code}"),
    }
    let _ = append_receipt(
        "refusal",
        json!({"reason":code,"detail":message}),
        "fleet",
        None,
        Some(EXIT_REFUSAL),
    );
    Err(EXIT_REFUSAL)
}

/// The PR body: the evidence bundle a human reviewer needs to see what the machine actually
/// proved (F08 contract §4.7, G8), plus the explicit "do not self-merge" line. Best-effort
/// field reads -- a placeholder here is a display nicety, not a gate; completeness is
/// `attest_verify_inner`'s and `AttestationBundle`'s job, already run before this is built.
fn build_pr_body(attestation: &Value, artifact_id: &str) -> String {
    let predicate = attestation.get("predicate").and_then(Value::as_object);
    let elements = predicate
        .and_then(|p| p.get("elements"))
        .and_then(Value::as_object);
    let str_field = |object: Option<&Map<String, Value>>, key: &str| -> String {
        object
            .and_then(|o| o.get(key))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string()
    };
    let builder = predicate.and_then(|p| p.get("builder")).and_then(Value::as_object);
    let builder_id = str_field(builder, "id");
    let resolved_model = str_field(builder, "resolved_model");
    let independent = elements
        .and_then(|e| e.get("independent_verification"))
        .and_then(Value::as_object);
    let verifier = str_field(independent, "verifier");
    let verdict = str_field(independent, "verdict");
    let reproduced = independent
        .and_then(|o| o.get("reproduced"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let adequacy = elements.and_then(|e| e.get("adequacy")).and_then(Value::as_object);
    let adequacy_line = match (
        adequacy.and_then(|o| o.get("checked")).and_then(Value::as_u64),
        adequacy.and_then(|o| o.get("total")).and_then(Value::as_u64),
    ) {
        (Some(checked), Some(total)) => format!("{checked}/{total}"),
        _ => "no-measurable-surface".to_string(),
    };
    let blast = elements.and_then(|e| e.get("blast_radius")).and_then(Value::as_object);
    let blast_count = blast
        .and_then(|o| o.get("count"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let blast_files = blast
        .and_then(|o| o.get("files"))
        .and_then(Value::as_array)
        .map(|files| {
            files
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let rollback = elements
        .and_then(|e| e.get("rollback"))
        .cloned()
        .unwrap_or(Value::Null);
    let wall_ms = elements
        .and_then(|e| e.get("cost"))
        .and_then(Value::as_object)
        .and_then(|o| o.get("wall_ms"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let quadrant = elements
        .and_then(|e| e.get("oracle_independence"))
        .and_then(Value::as_object)
        .and_then(|o| o.get("quadrant"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    format!(
        "## fleet delivery attestation\n\n\
         - artifact: `{artifact_id}` (blake3)\n\
         - builder: `{builder_id}` (resolved model: `{resolved_model}`)\n\
         - independent verifier: `{verifier}` -- verdict `{verdict}`, reproduced: {reproduced}\n\
         - adequacy: {adequacy_line}\n\
         - blast radius: {blast_count} file(s): {blast_files}\n\
         - rollback: `{rollback}`\n\
         - cost: {wall_ms}ms wall time\n\
         - oracle independence quadrant: `{quadrant}`\n\n\
         **This PR was opened by an automated agent and must not be self-merged.** \
         A human must review and merge it on the forge, under branch protection \
         (author != integrator).\n"
    )
}

/// The production [`fleet::lifecycle::ChangeEmitter`]: a real worktree
/// (`worktree::create`), a real `git apply` + commit + push, and a real `gh pr create`.
/// Lives here, not in `lifecycle.rs` -- the lib crate must not shell out to `git`/`gh`
/// itself (see `ChangeEmitter`'s doc comment there).
struct RealChangeEmitter;

impl lifecycle::ChangeEmitter for RealChangeEmitter {
    fn emit(
        &self,
        request: &lifecycle::ProposalRequest,
    ) -> Result<lifecycle::ProposedChange, lifecycle::Refusal> {
        use lifecycle::Refusal;

        let name = request.head.strip_prefix("fleet/").unwrap_or(&request.head);
        let wt = worktree::create(&request.repo, name).map_err(|_| {
            Refusal::new(
                "PR_EMIT_FAILED",
                "could not create a worktree for the proposal branch",
            )
        })?;

        let result = (|| -> Result<lifecycle::ProposedChange, Refusal> {
            // `git apply` reads the diff from stdin -- the bytes are already in memory
            // (read by the caller from $FLEET_STATE/artifacts/<id>), so there is no need for
            // an extra temp file. `--binary` mirrors `rollback_artifact`'s own invocation.
            let mut apply = Command::new("git")
                .arg("-C")
                .arg(&wt.path)
                .args(["apply", "--binary"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|_| Refusal::new("PR_EMIT_FAILED", "could not start git apply"))?;
            apply
                .stdin
                .take()
                .ok_or_else(|| Refusal::new("PR_EMIT_FAILED", "git apply has no stdin"))?
                .write_all(&request.diff)
                .map_err(|_| Refusal::new("PR_EMIT_FAILED", "could not write the diff to git apply"))?;
            let applied = apply
                .wait_with_output()
                .map_err(|_| Refusal::new("PR_EMIT_FAILED", "git apply did not exit"))?;
            if !applied.status.success() {
                return Err(Refusal::new(
                    "PR_EMIT_FAILED",
                    format!(
                        "git apply failed: {}",
                        String::from_utf8_lossy(&applied.stderr)
                    ),
                ));
            }

            git_ok(&wt.path, &["add", "-A"])?;
            git_ok(&wt.path, &["commit", "-q", "-m", &request.title])?;

            let rev_parse = Command::new("git")
                .arg("-C")
                .arg(&wt.path)
                .args(["rev-parse", "HEAD"])
                .output()
                .map_err(|_| Refusal::new("PR_EMIT_FAILED", "could not run git rev-parse"))?;
            if !rev_parse.status.success() {
                return Err(Refusal::new("PR_EMIT_FAILED", "git rev-parse HEAD failed"));
            }
            let commit = String::from_utf8_lossy(&rev_parse.stdout).trim().to_string();

            // Landmine (F08 contract §5): push BEFORE any cleanup -- `worktree::remove`
            // deletes the local branch, so the remote ref must already carry the commit.
            //
            // Force-push, not a plain push: a retry after a partial failure (push succeeded,
            // `gh pr create` did not) re-applies the identical attested diff onto the same
            // base, producing a new commit with the same tree but a different timestamp -- a
            // sibling of the already-pushed commit, not its descendant. A plain push rejects
            // that as non-fast-forward, which permanently wedges this task's `pr emit` path
            // until a human deletes the dangling remote branch by hand. `--force` is safe here
            // because `wt.branch` is this agent's own disposable proposal branch (deterministic
            // from the task id, never shared): this function is only ever reached while the
            // task is still `Accepted`, i.e. `gh pr create` has not yet succeeded for it, so
            // there is no open PR and nothing else has a reason to have pushed to this branch.
            git_ok(&wt.path, &["push", "-q", "-u", "--force", "origin", &wt.branch])?;

            let body_path = std::env::temp_dir().join(format!(
                "fleet-pr-body-{}-{}.md",
                std::process::id(),
                request.artifact_id
            ));
            fs::write(&body_path, &request.body)
                .map_err(|_| Refusal::new("PR_EMIT_FAILED", "could not write the PR body file"))?;
            let gh = Command::new("gh")
                .current_dir(&wt.path)
                .args([
                    "pr",
                    "create",
                    "--title",
                    &request.title,
                    "--body-file",
                    &body_path.to_string_lossy(),
                    "--head",
                    &wt.branch,
                    "--base",
                    &request.base,
                ])
                .output();
            let _ = fs::remove_file(&body_path);
            let gh = gh.map_err(|_| Refusal::new("PR_EMIT_FAILED", "could not run gh"))?;
            if !gh.status.success() {
                return Err(Refusal::new(
                    "PR_EMIT_FAILED",
                    format!("gh pr create failed: {}", String::from_utf8_lossy(&gh.stderr)),
                ));
            }
            // Exit 0 is not a pull request (F08 contract §3.1/§4.5, PR_URL_ABSENT): a real
            // `gh pr create` prints the URL on stdout, so an empty stdout is a refusal even
            // though the process exited 0.
            let url = String::from_utf8_lossy(&gh.stdout)
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("")
                .trim()
                .to_string();
            if url.is_empty() {
                return Err(Refusal::new("PR_URL_ABSENT", "gh exited 0 but printed no URL"));
            }

            let changed_files = diff_files(&request.diff)
                .map(|files| files.len() as u64)
                .unwrap_or(0);

            Ok(lifecycle::ProposedChange {
                url,
                head: wt.branch.clone(),
                commit,
                changed_files,
            })
        })();

        let _ = worktree::remove(&request.repo, &wt);
        result
    }
}

fn git_ok(repo: &Path, args: &[&str]) -> Result<(), lifecycle::Refusal> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|error| {
            lifecycle::Refusal::new(
                "PR_EMIT_FAILED",
                format!("could not run git {}: {error}", args.join(" ")),
            )
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(lifecycle::Refusal::new(
            "PR_EMIT_FAILED",
            format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            ),
        ))
    }
}

/// Test-only probe for F08's acceptance test (`keel/fleet/tests/f08_pr_emit.rs`), mirroring
/// `__lanes_probe` above: seeds a real fixture then calls the exact same `pr_emit_run` that
/// `fleet pr emit` calls, so the acceptance test drives real code, not a test-only path.
fn pr_emit_probe_command(args: &[String]) -> Result<(), i32> {
    let mut repo = None;
    let mut task = None;
    let mut base = None;
    let mut oracle = "adjudicated".to_string();
    let mut corrupt_artifact = false;
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == "--corrupt-artifact" {
            corrupt_artifact = true;
            i += 1;
            continue;
        }
        let value = args.get(i + 1).ok_or(EXIT_REFUSAL)?.clone();
        match args[i].as_str() {
            "--repo" => repo = Some(value),
            "--task" => task = Some(value),
            "--base" => base = Some(value),
            "--oracle" => oracle = value,
            _ => return Err(EXIT_REFUSAL),
        }
        i += 2;
    }
    let repo = repo.ok_or(EXIT_REFUSAL)?;
    let task = task.ok_or(EXIT_REFUSAL)?;
    let base = base.ok_or(EXIT_REFUSAL)?;

    let state = state_dir()?;
    let task_id = lifecycle::TaskId::new(task.clone()).map_err(|error| {
        eprintln!("fleet: __pr_emit_probe: {error}");
        EXIT_REFUSAL
    })?;
    let artifact_id = seed_pr_emit_fixture(&state, &repo, &task, &oracle, corrupt_artifact)?;
    lifecycle::persist_state(&state, &task_id, "Accepted").map_err(|error| {
        eprintln!("fleet: __pr_emit_probe: could not persist Accepted state: {error}");
        EXIT_ENV
    })?;

    pr_emit_run(&task, &artifact_id, &repo, &base, None)
}

/// Seeds a REAL, `attest_verify_inner`-passing attestation for `__pr_emit_probe`, through
/// production primitives (`append_receipt`, `write_json_atomic`, `blake3_hex`,
/// `diff_files`) -- never a second JSON validator. `oracle_status` is the ONLY thing that
/// varies between the pass and refusal cases (F08 contract §6.2, matching
/// `tests/f08_pr_emit.rs`'s own seeder comment); `corrupt_artifact` varies only the on-disk
/// artifact bytes, applied AFTER everything else has already been computed from the correct
/// bytes, so a corrupted artifact still carries an otherwise-complete attestation (proving
/// the digest check, not attestation shape, is what catches it).
fn seed_pr_emit_fixture(
    state: &Path,
    repo: &str,
    task: &str,
    oracle_status: &str,
    corrupt_artifact: bool,
) -> Result<String, i32> {
    let repo_path = Path::new(repo);

    // A REAL diff: mutate the fixture's tracked file, capture it with the same `git_diff`
    // production code uses, then restore the working tree (never `git stash` -- landmine).
    // Every fallible step here names itself on failure: a bare, unexplained exit is exactly
    // the D40/Q1-class defect this codebase's own conventions (see `state_dir`) exist to
    // eliminate, and it is the only way an intermittent, load-dependent failure here (this
    // probe forks many `git` subprocesses; the acceptance test runs four of these
    // concurrently) is diagnosable instead of just "flaky".
    let target = repo_path.join("main.rs");
    let original = fs::read(&target).map_err(|error| {
        eprintln!("fleet: __pr_emit_probe: could not read {}: {error}", target.display());
        EXIT_ENV
    })?;
    let mut mutated = original;
    mutated.extend_from_slice(b"\n// f08 proposed change\n");
    fs::write(&target, &mutated).map_err(|error| {
        eprintln!("fleet: __pr_emit_probe: could not write {}: {error}", target.display());
        EXIT_ENV
    })?;
    let diff = git_diff(repo).map_err(|code| {
        eprintln!("fleet: __pr_emit_probe: git diff HEAD failed in {repo} (exit {code})");
        code
    })?;
    let restored = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["checkout", "--", "main.rs"])
        .output()
        .map_err(|error| {
            eprintln!("fleet: __pr_emit_probe: could not run git checkout in {repo}: {error}");
            EXIT_ENV
        })?;
    if !restored.status.success() {
        eprintln!(
            "fleet: __pr_emit_probe: git checkout -- main.rs failed in {repo}: {}",
            String::from_utf8_lossy(&restored.stderr)
        );
        return Err(EXIT_ENV);
    }
    if diff.is_empty() {
        eprintln!("fleet: __pr_emit_probe: the mutation produced an empty diff in {repo}");
        return Err(EXIT_INVARIANT);
    }

    let artifact_id = blake3_hex(&diff);
    let artifact_path = state.join("artifacts").join(&artifact_id);
    freeze_artifact(&artifact_path, &diff).map_err(|code| {
        eprintln!(
            "fleet: __pr_emit_probe: could not freeze artifact {} (exit {code})",
            artifact_path.display()
        );
        code
    })?;

    let named_receipt = |event: &'static str, body: Value| -> Result<String, i32> {
        append_receipt(event, body, "fleet", None, None).map_err(|code| {
            eprintln!("fleet: __pr_emit_probe: could not append {event} receipt (exit {code})");
            code
        })
    };
    let run_id = format!("f08-probe-{}", std::process::id());
    let start_hash = named_receipt(
        "run_start",
        json!({"run_id":run_id,"agent":"stub","repo":repo,"task":task}),
    )?;
    let frozen_hash = named_receipt(
        "artifact_frozen",
        json!({"artifact_id":artifact_id,"bytes":diff.len(),"repo":repo}),
    )?;
    let attested_hash = named_receipt(
        "attested",
        json!({"artifact_id":artifact_id,"attestation_id":artifact_id}),
    )?;
    let ended_hash = named_receipt(
        "run_end",
        json!({"run_id":run_id,"artifact_id":artifact_id,"status":"ok","resolved_model":null}),
    )?;

    let blast_files = diff_files(&diff).map_err(|code| {
        eprintln!("fleet: __pr_emit_probe: diff_files could not parse the seeded diff (exit {code})");
        code
    })?;
    let blast_count = blast_files.len() as u64;
    let characters_in_diff = String::from_utf8_lossy(&diff).chars().count() as u64;

    // The ONLY element that varies with `--oracle`: `run_with_evidence` really does write
    // exactly `{"status":"pending-adjudication"}` (main.rs:1658) before adjudication fills
    // it in, so this is the real, routinely-produced incomplete attestation, not a
    // synthetic hole (F08 contract §4.5 note).
    let oracle_element = if oracle_status == "adjudicated" {
        json!({
            "o1_author":"lead",
            "o2_author":"verifier",
            "o1_hash": blake3_hex(b"f08-probe-o1"),
            "o2_hash": blake3_hex(b"f08-probe-o2"),
            "distinct": true,
            "quadrant": "ACCEPT"
        })
    } else {
        json!({"status": oracle_status})
    };

    let attestation = json!({
        "_type":"https://in-toto.io/Statement/v1",
        "subject":[{"name":"git-diff","digest":{"blake3":artifact_id}}],
        "predicateType":"https://fleet.local/DeliveryAttestation/v1",
        "predicate":{
            "tier":"T-min",
            "elements":{
                "sow":{"task":task},
                "blind_suite":{
                    "in_worktree_tree":true,
                    "in_object_store":false,
                    "in_env":false,
                    "on_any_fd":false,
                    "suite_hash":blake3_hex(b"f08-probe-suite")
                },
                "independent_verification":{
                    "builder":"stub",
                    "verifier":"stub-verifier",
                    "distinct":true,
                    "reproduced":true,
                    "verdict":"ACCEPT"
                },
                "adequacy":{"status":"no-measurable-surface"},
                "blast_radius":{"files":blast_files,"count":blast_count,"source":"recorded-diff"},
                "rollback":{"executed":true,"suite_went_red":false,"reverted_files":blast_count},
                "cost":{
                    "wall_ms":1,
                    "characters_in_diff":characters_in_diff,
                    "tokens":null,
                    "tokenizer_generation":null,
                    "source":"observed"
                },
                "oracle_independence":oracle_element
            },
            "receipts":[start_hash,frozen_hash,attested_hash,ended_hash],
            "builder":{"id":"stub","resolved_model":null}
        }
    });
    let att_path = state.join("attestations").join(format!("{artifact_id}.json"));
    write_json_atomic(&att_path, &attestation).map_err(|code| {
        eprintln!(
            "fleet: __pr_emit_probe: could not write attestation {} (exit {code})",
            att_path.display()
        );
        code
    })?;

    if corrupt_artifact {
        // Tamper the ON-DISK bytes AFTER everything above was computed from the correct
        // ones: the attestation stays structurally complete, but the file no longer hashes
        // to its own filename -- exactly a corrupted/tampered artifact, and the ONLY thing
        // this flag varies (the seeder is not what produces the PASS or this refusal).
        fs::set_permissions(&artifact_path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            eprintln!(
                "fleet: __pr_emit_probe: chmod 0600 on {} failed: {error}",
                artifact_path.display()
            );
            EXIT_ENV
        })?;
        let mut bytes = fs::read(&artifact_path).map_err(|error| {
            eprintln!(
                "fleet: __pr_emit_probe: read {} failed: {error}",
                artifact_path.display()
            );
            EXIT_ENV
        })?;
        bytes.push(b'\n');
        fs::write(&artifact_path, &bytes).map_err(|error| {
            eprintln!(
                "fleet: __pr_emit_probe: write {} failed: {error}",
                artifact_path.display()
            );
            EXIT_ENV
        })?;
        fs::set_permissions(&artifact_path, fs::Permissions::from_mode(0o444)).map_err(|error| {
            eprintln!(
                "fleet: __pr_emit_probe: chmod 0444 on {} failed: {error}",
                artifact_path.display()
            );
            EXIT_ENV
        })?;
    }

    Ok(artifact_id)
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::run_role_lanes_concurrently;
    use super::{
        adjudication_table, git_diff, git_status_porcelain, interpret_freelane_output, mktemp_dir,
        parse_freelane_model, parse_freelane_usage, resolve_run_agent, reverse_landed_artifact,
        rollback_repo, select_swarm_builder, RoutedRole, EXIT_ENV, EXIT_INVARIANT, EXIT_OK,
        EXIT_REFUSAL,
    };
    use crate::roles::Role;
    use serde_json::json;
    use serde_json::{Map, Value};
    use std::fs;
    use std::process::Command;

    const CORRECT_BUILD: &[u8] = b"correct implementation";
    const BUILDER_OWN_BROKEN_DIFF: &[u8] = b"broken implementation from builder";

    type TestOracle = fn(&[u8]) -> bool;

    fn fair_oracle(artifact: &[u8]) -> bool {
        artifact == CORRECT_BUILD
    }

    fn poisoned_toward_builder_oracle(artifact: &[u8]) -> bool {
        artifact == BUILDER_OWN_BROKEN_DIFF
    }

    fn rejects_every_build_oracle(_artifact: &[u8]) -> bool {
        false
    }

    fn adjudicate_fixture(artifact: &[u8], o1: TestOracle, o2: TestOracle) -> &'static str {
        adjudication_table(o1(artifact), o2(artifact)).name
    }

    #[test]
    fn resolve_run_agent_returns_none_when_neither_given() {
        let state = mktemp_dir().expect("temporary state dir");
        assert_eq!(
            resolve_run_agent(None, None, &state, false).expect("must not error"),
            None
        );
    }

    #[test]
    fn resolve_run_agent_never_lets_role_override_an_explicit_agent() {
        let state = mktemp_dir().expect("temporary state dir");
        // Deliberately pass a role too, and a state dir that was never populated with any
        // routable capability -- if --role were consulted at all here it could only refuse or
        // pick something other than "claude". An explicit --agent must short-circuit before the
        // router is ever called.
        let resolved =
            resolve_run_agent(Some("claude".to_string()), Some("builder"), &state, false)
                .expect("an explicit --agent must never be refused because of --role");
        // No model tier either: nothing routed, so nothing decided a tier.
        assert_eq!(resolved, Some(("claude".to_string(), None)));
    }

    #[test]
    fn resolve_run_agent_refuses_an_unknown_role_before_touching_the_router() {
        let state = mktemp_dir().expect("temporary state dir");
        let err = resolve_run_agent(None, Some("not-a-real-role"), &state, false)
            .expect_err("an unrecognised --role must refuse, not silently fall through");
        assert_eq!(err, EXIT_REFUSAL);
    }

    #[test]
    fn resolve_run_agent_with_role_only_either_picks_a_worker_adapter_or_refuses_cleanly() {
        let state = mktemp_dir().expect("temporary state dir");
        // "builder" is Tier::Worker; ORDER's worker-tier candidates all resolve to one of these
        // three adapter ids regardless of which specific CLI this machine has installed. This
        // must not panic and must not silently invent an agent name.
        //
        // A route refusal is logged via append_receipt(), which resolves the ledger through the
        // AMBIENT FLEET_STATE env var (ledger_paths(), no path parameter) -- not through the
        // `state` argument threaded into route::for_plan() above it. In a real invocation the two
        // agree (both ultimately come from state_dir()); in this isolated test FLEET_STATE is
        // deliberately unset, so a refusal here surfaces as EXIT_ENV (the receipt write itself
        // faults) rather than EXIT_REFUSAL. Both are legitimate, correctly-typed outcomes
        // (AGENTS.md rule 7: an environment fault is its own class, not a bug) -- only a panic or
        // a silently invented agent name would be a real defect.
        match resolve_run_agent(None, Some("builder"), &state, false) {
            Ok(Some((adapter, tier))) => {
                assert!(
                    ["codex", "claude", "freelane"].contains(&adapter.as_str()),
                    "unexpected adapter id from the router: {adapter}"
                );
                assert!(
                    tier.is_some(),
                    "a routed selection must carry the resolved model tier"
                );
            }
            Ok(None) => panic!("a role was given; None means the role branch was never taken"),
            Err(code) => assert!(
                code == EXIT_REFUSAL || code == EXIT_ENV,
                "a route refusal must be typed EXIT_REFUSAL or EXIT_ENV, got {code}"
            ),
        }
    }

    #[test]
    fn swarm_always_executes_the_builder_allocation() {
        let routed = vec![
            RoutedRole {
                role: Role::Lead,
                agent: "claude".to_string(),
                model: "opus".to_string(),
                decided_at_stage: 6,
            },
            RoutedRole {
                role: Role::Builder,
                agent: "codex".to_string(),
                model: "codex".to_string(),
                decided_at_stage: 6,
            },
            RoutedRole {
                role: Role::Verifier,
                agent: "claude".to_string(),
                model: "sonnet".to_string(),
                decided_at_stage: 6,
            },
        ];
        let selected = select_swarm_builder(routed).expect("builder allocation must exist");
        assert_eq!(selected.role, Role::Builder);
        assert_eq!(selected.agent, "codex");
        assert_eq!(selected.model, "codex");
    }

    #[test]
    fn oracle_adjudication_fires_all_four_quadrants() {
        let cases = [
            (true, true, "ACCEPT", "NONE", EXIT_OK),
            (true, false, "ORACLE_INADEQUATE", "LEAD", EXIT_INVARIANT),
            (
                false,
                true,
                "ORACLE_OVERCONSTRAINED",
                "LEAD",
                EXIT_INVARIANT,
            ),
            (false, false, "BUILDER_FAULT", "BUILDER", EXIT_INVARIANT),
        ];
        for (o1_pass, o2_pass, quadrant, fault, code) in cases {
            let result = adjudication_table(o1_pass, o2_pass);
            assert_eq!(result.name, quadrant);
            assert_eq!(result.fault, fault);
            assert_eq!(result.code, code);
        }
    }

    #[test]
    fn adversarial_builder_poisoned_oracle_is_inadequate() {
        assert_eq!(
            adjudicate_fixture(
                BUILDER_OWN_BROKEN_DIFF,
                poisoned_toward_builder_oracle,
                fair_oracle,
            ),
            "ORACLE_INADEQUATE"
        );
    }

    #[test]
    fn adversarial_impossibly_strict_oracle_is_overconstrained() {
        assert_eq!(
            adjudicate_fixture(CORRECT_BUILD, rejects_every_build_oracle, fair_oracle),
            "ORACLE_OVERCONSTRAINED"
        );
    }

    #[test]
    fn adversarial_broken_build_is_builder_fault() {
        assert_eq!(
            adjudicate_fixture(BUILDER_OWN_BROKEN_DIFF, fair_oracle, fair_oracle),
            "BUILDER_FAULT"
        );
    }

    #[test]
    fn adversarial_correct_build_is_accepted() {
        assert_eq!(
            adjudicate_fixture(CORRECT_BUILD, fair_oracle, fair_oracle),
            "ACCEPT"
        );
    }

    #[test]
    fn freelane_reads_back_reported_model_instead_of_requested_model() {
        let log = "[resolved_model=served-model requested=requested-model]\n";
        assert_eq!(parse_freelane_model(log).as_deref(), Some("served-model"));
        assert_ne!(
            parse_freelane_model(log).as_deref(),
            Some("requested-model")
        );
        assert_eq!(parse_freelane_model("gateway omitted metadata"), None);
        assert_eq!(
            parse_freelane_model("[resolved_model=UNRESOLVED requested=requested-model]"),
            None
        );
    }

    #[test]
    fn freelane_trace_with_usage_records_total_tokens() {
        let log = concat!(
            "[resolved_model=served-model requested=alias lane=1/3 tried=lane:answered ",
            "usage={\"prompt_tokens\":11,\"completion_tokens\":24,\"total_tokens\":35}]"
        );
        assert_eq!(parse_freelane_usage(log), Some(35));
        let output =
            interpret_freelane_output(Some(EXIT_OK), b"answer\n".to_vec(), log.as_bytes().to_vec())
                .expect("valid freelane response");
        assert_eq!(output.tokens, Some(35));
        assert_eq!(output.resolved_model.as_deref(), Some("served-model"));
    }

    #[test]
    fn freelane_trace_without_usage_records_null() {
        let output = interpret_freelane_output(
            Some(EXIT_OK),
            b"answer\n".to_vec(),
            b"[resolved_model=served-model requested=alias]".to_vec(),
        )
        .expect("usage metadata is optional");
        assert_eq!(output.tokens, None);
    }

    #[test]
    fn malformed_or_partial_freelane_usage_records_null_without_panicking() {
        for log in [
            "[resolved_model=m requested=a usage={not-json}]",
            "[resolved_model=m requested=a usage={\"prompt_tokens\":11,\"total_tokens\":35}]",
            "[resolved_model=m requested=a usage={\"prompt_tokens\":11,\"completion_tokens\":24,\"total_tokens\":36}]",
        ] {
            assert_eq!(parse_freelane_usage(log), None);
        }
    }

    #[test]
    fn freelane_network_absence_is_an_environment_fault() {
        let failure = interpret_freelane_output(
            Some(EXIT_ENV),
            Vec::new(),
            b"freelane: no response (lane unavailable)\n".to_vec(),
        )
        .expect_err("network absence must not look like a successful empty diff");
        assert_eq!(failure.code, EXIT_ENV);
        assert!(failure.reason.contains("lane unavailable"));
    }

    #[test]
    fn failed_run_leaves_repo_clean_and_preserves_evidence() {
        let repo = mktemp_dir().expect("temporary repo");
        let evidence = mktemp_dir().expect("temporary evidence store");
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .status()
                .expect("git must run")
                .success()
        };
        assert!(git(&["init", "--quiet"]));
        assert!(git(&["config", "user.email", "fleet-test@example.invalid"]));
        assert!(git(&["config", "user.name", "Fleet Test"]));
        fs::write(repo.join("landed.txt"), "before\n").expect("write baseline");
        assert!(git(&["add", "landed.txt"]));
        assert!(git(&["commit", "--quiet", "-m", "baseline"]));
        fs::write(repo.join("landed.txt"), "after\n").expect("land failed work");

        let artifact = evidence.join("artifact");
        let attestation = evidence.join("attestation.json");
        fs::write(&artifact, git_diff(repo.to_str().unwrap()).unwrap()).expect("freeze artifact");
        fs::write(&attestation, b"{\"status\":\"verification_failed\"}")
            .expect("freeze attestation");

        reverse_landed_artifact(&repo, &artifact).expect("automatic rollback");
        assert!(git_status_porcelain(&repo).unwrap().is_empty());
        assert_eq!(
            fs::read_to_string(repo.join("landed.txt")).unwrap(),
            "before\n"
        );
        assert!(artifact.exists(), "rollback must preserve the artifact");
        assert!(
            attestation.exists(),
            "rollback must preserve the attestation"
        );
        fs::remove_dir_all(repo).expect("remove temporary repo");
        fs::remove_dir_all(evidence).expect("remove temporary evidence");
    }

    #[test]
    fn rollback_of_unapplied_artifact_is_refused() {
        assert_eq!(
            rollback_repo(&[], "never-applied"),
            Err("ARTIFACT_WAS_NEVER_APPLIED")
        );
    }

    #[test]
    fn rollback_twice_is_refused_the_second_time() {
        let rows = vec![json!({
            "event":"rollback",
            "body":{
                "artifact_id":"already-healed",
                "status":"rolled_back"
            }
        })];
        assert_eq!(
            rollback_repo(&rows, "already-healed"),
            Err("ARTIFACT_ALREADY_ROLLED_BACK")
        );
    }

    // S2b: the router's environment faults must degrade the PLAN, not the router's
    // invariants. `route::for_plan` returns Err(EXIT_ENVIRONMENT) when fresh state has no meter
    // windows yet — an ordinary first-run situation. It returns Err(EXIT_INVARIANT) when the
    // meter it was given to plan against is corrupt. The old catch-all `Err(_)` treated both the
    // same: printed the plan and exited 0, silently swallowing the invariant. The reviewer
    // (docs/REVIEW-S2b.md) reproduced exactly that.
    #[test]
    fn plan_command_rate_limits_router_faults_to_environment_class_only() {
        std::env::remove_var("FLEET_METER_WINDOWS");
        let state = mktemp_dir().expect("temporary state dir");
        let lines =
            super::plan_command_lines(&["add a --version flag".to_string()], || Ok(state.clone()))
                .expect("a fresh-state router env fault must degrade the plan, not abort it");
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("routed lane: UNAVAILABLE — routing could not be evaluated")),
            "expected the benign degrade message, got: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l == "commands: 3 planned (denominator: 3)"),
            "the full command list must survive an env-class router fault, got: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.ends_with("fleet swarm dispatch")),
            "the plan must still name the dispatch it would gate, got: {lines:?}"
        );
    }

    #[test]
    fn plan_command_propagates_a_corrupt_meter_invariant() {
        std::env::remove_var("FLEET_METER_WINDOWS");
        let state = mktemp_dir().expect("temporary state dir");
        fs::write(
            state.join("meter-v1.tsv"),
            b"not-a-valid-meter-window-row\n",
        )
        .expect("write corrupt meter state");
        let err =
            super::plan_command_lines(&["add a --version flag".to_string()], || Ok(state.clone()))
                .expect_err("a corrupt meter is an invariant violation, never a benign env fault");
        assert_eq!(err, super::EXIT_INVARIANT);
    }

    // S2: a lane_status projection must carry the ledger_ref the authored body cannot self-supply.
    // The projection validator assembles body + envelope and FAILS when the stamped ledger_ref is
    // absent — the exact regression the reviewer found (0 of 7 lane_status bodies carried it).
    #[test]
    fn lane_status_projection_rejects_an_authored_body_without_ledger_ref() {
        let mut projection: Map<String, Value> = Map::new();
        projection.insert("schema_version".to_string(), json!("1.0"));
        projection.insert("lane_id".to_string(), json!("builder"));
        projection.insert("role".to_string(), json!("builder"));
        projection.insert("state".to_string(), json!("running"));
        projection.insert("agent".to_string(), json!("codex"));
        let err = super::validate_lane_status_projection(&projection)
            .expect_err("an authored body with no stamped ledger_ref must fail validation");
        assert!(
            err.contains("ledger_ref"),
            "missing ledger_ref must be the reported failure, got: {err}"
        );
    }

    #[test]
    fn lane_status_projection_accepts_a_stamped_projection() {
        let mut projection: Map<String, Value> = Map::new();
        projection.insert("schema_version".to_string(), json!("1.0"));
        projection.insert("lane_id".to_string(), json!("builder"));
        projection.insert("role".to_string(), json!("builder"));
        projection.insert("state".to_string(), json!("running"));
        projection.insert("agent".to_string(), json!("codex"));
        projection.insert("actor".to_string(), json!("fleet"));
        projection.insert("ts_wall".to_string(), json!("2026-09-02T00:00:00Z"));
        projection.insert(
            "ledger_ref".to_string(),
            json!({"seq": 3, "hash": format!("blake3:{}", "a".repeat(64))}),
        );
        super::validate_lane_status_projection(&projection).expect("valid projection");
    }

    #[test]
    fn lane_status_projection_rejects_invalid_state_and_hash() {
        let hash = "a".repeat(64);
        let valid_ref = json!({"seq": 3, "hash": format!("blake3:{hash}")});
        let bad_state = json!({
            "schema_version": "1.0",
            "lane_id": "builder",
            "role": "builder",
            "state": "unknown-state",
            "agent": "codex",
            "actor": "fleet",
            "ts_wall": "2026-09-02T00:00:00Z",
            "ledger_ref": valid_ref
        });
        assert!(
            super::validate_lane_status_projection(&bad_state.as_object().unwrap().clone())
                .unwrap_err()
                .contains("state"),
            "an invalid state enum must fail"
        );

        let bad_hash = json!({
            "schema_version": "1.0",
            "lane_id": "builder",
            "role": "builder",
            "state": "running",
            "agent": "codex",
            "actor": "fleet",
            "ts_wall": "2026-09-02T00:00:00Z",
            "ledger_ref": {"seq": 3, "hash": "not-a-hash"}
        });
        assert!(
            super::validate_lane_status_projection(&bad_hash.as_object().unwrap().clone())
                .unwrap_err()
                .contains("hash"),
            "an invalid ledger_ref.hash must fail"
        );
    }

    // S2: the projection validator must publish a real denominator over real receipt rows — and
    // must FAIL when it examined zero lane inputs (never a silent pass).
    #[test]
    fn lane_status_row_validator_publishes_denominator_and_rejects_zero_input() {
        let hash = "a".repeat(64);
        let rows = vec![
            json!({
                "schema_version":"1.0","seq":0,"hash":format!("blake3:{hash}"),
                "event":"lane_status","actor":"fleet","ts_wall":"2026-09-02T00:00:00Z",
                "body":{"schema_version":"1.0","lane_id":"builder","role":"builder","state":"queued","agent":"codex"}
            }),
            json!({
                "schema_version":"1.0","seq":1,"hash":format!("blake3:{}","b".repeat(64)),
                "event":"lane_status","actor":"fleet","ts_wall":"2026-09-02T00:00:00Z",
                "body":{"schema_version":"1.0","lane_id":"builder","role":"builder","state":"running","agent":"codex"}
            }),
            json!({
                "schema_version":"1.0","seq":2,"hash":format!("blake3:{}","c".repeat(64)),
                "event":"run_start","actor":"fleet","ts_wall":"2026-09-02T00:00:00Z",
                "body":{"task":"t"}
            }),
        ];
        super::validate_lane_status_rows(&rows)
            .expect("all 2 lane_status projections must validate");

        // Zero lane_status rows => examined counts are all zero => that is a failure, not a pass.
        let absence = json!({
            "schema_version":"1.0","seq":0,"hash":format!("blake3:{hash}"),
            "event":"run_start","actor":"fleet","ts_wall":"2026-09-02T00:00:00Z",
            "body":{"task":"t"}
        });
        assert_eq!(
            super::validate_lane_status_rows(&[absence]),
            Err(super::EXIT_MISMATCH),
            "a projection check that examined zero lane inputs must fail, never pass"
        );
    }

    // ---- S3: parallel, worktree-isolated role lanes -------------------------------------------
    // The real, subprocess-level concurrency proof (per-lane wall-clock start/end timestamps,
    // asserted overlap) lives in `keel/fleet/tests/s3_lanes.rs`, a Cargo INTEGRATION test, not
    // here: `spawn_agent_with_args` re-execs `env::current_exe()`, and inside `cargo test`'s unit
    // harness that resolves to the test binary itself, not `fleet` -- so a `#[test]` in this
    // module can exercise worktree lifecycle and error-path cleanup (below), but not the real
    // re-exec hop. See `__lanes_probe` and its doc comment for how the integration test drives
    // the exact same `run_role_lanes_concurrently` through the real compiled binary instead.
    fn init_lane_fixture_repo() -> std::path::PathBuf {
        let dir = mktemp_dir().expect("temp fixture dir");
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .status()
                .expect("git must be on PATH for this test");
            assert!(status.success(), "git {args:?} failed");
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "s3-test@example.com"]);
        run(&["config", "user.name", "s3-test"]);
        fs::write(dir.join("main.rs"), b"fn main() {}\n").expect("write fixture main.rs");
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "init"]);
        dir
    }

    #[test]
    fn worktree_lane_cap_respects_the_s3_formula() {
        // min(16, available_parallelism() - 2), floored at 1 -- not re-derived here, just
        // asserted against the same bound the acceptance bar names verbatim.
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let expected = cores.saturating_sub(2).clamp(1, 16);
        assert_eq!(crate::worktree::lane_cap(), expected);
    }

    #[test]
    fn a_failed_lane_never_leaks_its_worktree() {
        let repo = init_lane_fixture_repo();
        // The spawned agent fails inside `cargo test`'s harness (see the header comment above:
        // `env::current_exe()` resolves to the test binary here, not `fleet`), which is itself a
        // real, non-EXIT_OK failure of the spawn step -- so this still genuinely exercises
        // worktree::create -> failed spawn -> worktree::remove, proving cleanup runs on a real
        // error path. The full happy-path (agent succeeds, exit 0) is proven in the integration
        // test instead, where the real binary re-execs itself correctly.
        let routed = vec![RoutedRole {
            role: Role::Meter,
            agent: "unknown-agent".to_string(),
            model: "n/a".to_string(),
            decided_at_stage: 0,
        }];
        let outcomes = super::run_role_lanes_concurrently(&repo, "s3 failure probe", &routed);
        assert_eq!(outcomes.len(), 1);
        assert_ne!(
            outcomes[0].status, EXIT_OK,
            "the unknown agent must fail, not succeed"
        );

        let worktrees_dir = repo.join(".worktrees");
        let leaked = worktrees_dir.exists()
            && fs::read_dir(&worktrees_dir)
                .expect("read .worktrees")
                .next()
                .is_some();
        assert!(
            !leaked,
            "a failed lane leaked a worktree under {worktrees_dir:?}"
        );
        fs::remove_dir_all(&repo).ok();
    }
}
mod meter;
