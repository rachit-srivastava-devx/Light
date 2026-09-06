use crossterm::terminal;
use ratatui::{
    backend::TestBackend,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Terminal,
};
use serde::Serialize;
use serde_json::{Map, Value};

const EXIT_INVARIANT: i32 = 6;
const STATES: [TaskState; 4] = [
    TaskState::Done,
    TaskState::Pending,
    TaskState::Failed,
    TaskState::NeedsIteration,
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
enum TaskState {
    Done,
    Pending,
    Failed,
    NeedsIteration,
}

impl TaskState {
    fn label(self) -> &'static str {
        match self {
            Self::Done => "DONE",
            Self::Pending => "PENDING",
            Self::Failed => "FAILED",
            Self::NeedsIteration => "NEEDS-ITERATION",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct StatusTask {
    task: Option<String>,
    state: TaskState,
    agent: Option<String>,
    attestation_verifies: Option<bool>,
    outcome: Option<String>,
    receipts: Vec<String>,
}

#[derive(Debug, Serialize)]
struct StatusGroup<'a> {
    state: TaskState,
    checked: usize,
    total: usize,
    tasks: Vec<&'a StatusTask>,
}

#[derive(Debug, Serialize)]
struct StatusReport<'a> {
    checked: usize,
    total: usize,
    empty: bool,
    groups: Vec<StatusGroup<'a>>,
}

#[derive(Debug, Default)]
struct RunReceipts {
    task: Option<String>,
    agent: Option<String>,
    run_id: Option<String>,
    artifact: Option<String>,
    attested: bool,
    work_landed: bool,
    run_status: Option<String>,
    refusal: Option<String>,
    receipts: Vec<String>,
}

pub fn command(
    rows: &[Value],
    json_output: bool,
    verifies: impl Fn(&str) -> bool,
) -> Result<(), i32> {
    let tasks = derive_tasks(rows, verifies);
    let report = report(&tasks);
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|_| EXIT_INVARIANT)?
        );
    } else {
        print!("{}", render_human(&report)?);
    }
    Ok(())
}

fn report(tasks: &[StatusTask]) -> StatusReport<'_> {
    let total = tasks.len();
    let groups = STATES
        .iter()
        .copied()
        .map(|state| {
            let selected = tasks
                .iter()
                .filter(|task| task.state == state)
                .collect::<Vec<_>>();
            StatusGroup {
                state,
                checked: selected.len(),
                total,
                tasks: selected,
            }
        })
        .collect();
    StatusReport {
        checked: total,
        total,
        empty: tasks.is_empty(),
        groups,
    }
}

fn derive_tasks(rows: &[Value], verifies: impl Fn(&str) -> bool) -> Vec<StatusTask> {
    let mut runs = Vec::<RunReceipts>::new();
    for row in rows {
        let Some(object) = row.as_object() else {
            continue;
        };
        let Some(body) = object.get("body").and_then(Value::as_object) else {
            continue;
        };
        let event = object.get("event").and_then(Value::as_str);
        let receipt = object
            .get("hash")
            .and_then(Value::as_str)
            .map(str::to_owned);
        match event {
            Some("run_start") => {
                let mut run = RunReceipts {
                    task: string(body, "task"),
                    agent: string(body, "agent"),
                    run_id: string(body, "run_id"),
                    ..RunReceipts::default()
                };
                push_receipt(&mut run, receipt);
                runs.push(run);
            }
            Some("artifact_frozen") => {
                if let Some(index) = current_run(&runs) {
                    let run = &mut runs[index];
                    run.artifact = string(body, "artifact_id");
                    run.work_landed = true;
                    push_receipt(run, receipt);
                }
            }
            Some("attested") => {
                if let Some(index) = run_for_artifact(&runs, body).or_else(|| current_run(&runs)) {
                    let run = &mut runs[index];
                    if run.artifact.is_none() {
                        run.artifact = string(body, "artifact_id");
                    }
                    run.attested = true;
                    push_receipt(run, receipt);
                }
            }
            Some("run_end") => {
                if let Some(index) = run_for_id(&runs, body).or_else(|| current_run(&runs)) {
                    let run = &mut runs[index];
                    run.run_status = string(body, "status");
                    if run.artifact.is_none() {
                        run.artifact = string(body, "artifact_id");
                    }
                    push_receipt(run, receipt);
                }
            }
            Some("refusal") => {
                let named_task = string(body, "task");
                let target = run_for_id(&runs, body).or_else(|| {
                    named_task.as_ref().and_then(|task| {
                        runs.iter().rposition(|run| {
                            run.task.as_deref() == Some(task.as_str()) && run.run_status.is_none()
                        })
                    })
                });
                let target = target.or_else(|| {
                    if named_task.is_none() {
                        current_run(&runs)
                    } else {
                        None
                    }
                });
                if let Some(index) = target {
                    let run = &mut runs[index];
                    run.refusal = string(body, "reason");
                    if run.agent.is_none() {
                        run.agent = string(body, "agent");
                    }
                    push_receipt(run, receipt);
                } else if body.get("task").and_then(Value::as_str).is_some() {
                    let mut run = RunReceipts {
                        task: string(body, "task"),
                        agent: string(body, "agent"),
                        refusal: string(body, "reason"),
                        ..RunReceipts::default()
                    };
                    push_receipt(&mut run, receipt);
                    runs.push(run);
                }
            }
            Some("gate_verdict") => {
                if let Some(index) = current_run(&runs) {
                    push_receipt(&mut runs[index], receipt);
                }
            }
            _ => {}
        }
    }

    runs.into_iter()
        .map(|run| classify(run, &verifies))
        .collect()
}

fn classify(run: RunReceipts, verifies: &impl Fn(&str) -> bool) -> StatusTask {
    let attestation_verifies = if run.attested {
        run.artifact.as_deref().map(verifies)
    } else {
        None
    };
    let (state, outcome) = if run.refusal.is_some() && run.work_landed {
        (
            TaskState::NeedsIteration,
            run.refusal
                .as_deref()
                .map(|reason| format!("refused after work landed: {reason}")),
        )
    } else if run.run_status.as_deref() == Some("ok") {
        if attestation_verifies == Some(true) {
            (
                TaskState::Done,
                Some("completed; attestation verified".to_string()),
            )
        } else {
            (
                TaskState::NeedsIteration,
                Some("completed; attestation does not verify".to_string()),
            )
        }
    } else if let Some(reason) = run.refusal.as_deref() {
        (TaskState::Failed, Some(format!("refused: {reason}")))
    } else if let Some(status) = run.run_status.as_deref() {
        (TaskState::Failed, Some(format!("run ended: {status}")))
    } else {
        (
            TaskState::Pending,
            Some("run started; no terminal receipt".to_string()),
        )
    };
    StatusTask {
        task: run.task,
        state,
        agent: run.agent,
        attestation_verifies,
        outcome,
        receipts: run.receipts,
    }
}

fn current_run(runs: &[RunReceipts]) -> Option<usize> {
    runs.iter()
        .rposition(|run| run.run_status.is_none() && run.refusal.is_none())
}

fn run_for_id(runs: &[RunReceipts], body: &Map<String, Value>) -> Option<usize> {
    let id = body.get("run_id").and_then(Value::as_str)?;
    runs.iter()
        .rposition(|run| run.run_id.as_deref() == Some(id))
}

fn run_for_artifact(runs: &[RunReceipts], body: &Map<String, Value>) -> Option<usize> {
    let artifact = body.get("artifact_id").and_then(Value::as_str)?;
    runs.iter()
        .rposition(|run| run.artifact.as_deref() == Some(artifact))
}

fn string(body: &Map<String, Value>, key: &str) -> Option<String> {
    body.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn push_receipt(run: &mut RunReceipts, receipt: Option<String>) {
    if let Some(receipt) = receipt {
        run.receipts.push(receipt);
    }
}

fn render_human(report: &StatusReport<'_>) -> Result<String, i32> {
    let width = terminal::size()
        .map(|(width, _)| width)
        .unwrap_or(100)
        .clamp(60, 100);
    let height = u16::try_from(
        report
            .groups
            .iter()
            .map(|group| 3usize.saturating_add(group.tasks.len().max(1)))
            .sum::<usize>(),
    )
    .map_err(|_| EXIT_INVARIANT)?;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).map_err(|_| EXIT_INVARIANT)?;
    terminal
        .draw(|frame| {
            let mut y = 0u16;
            for group in &report.groups {
                let group_height = u16::try_from(3usize.saturating_add(group.tasks.len().max(1)))
                    .unwrap_or(u16::MAX);
                let area = Rect::new(0, y, width, group_height);
                let rows = if group.tasks.is_empty() {
                    vec![Row::new(vec!["(no tasks)", "—", "—", "—"])]
                } else {
                    group
                        .tasks
                        .iter()
                        .map(|task| {
                            Row::new(vec![
                                Cell::from(display(task.task.as_deref())),
                                Cell::from(display(task.agent.as_deref())),
                                Cell::from(match task.attestation_verifies {
                                    Some(true) => "yes",
                                    Some(false) => "no",
                                    None => "—",
                                }),
                                Cell::from(display(task.outcome.as_deref())),
                            ])
                        })
                        .collect()
                };
                let table = Table::new(
                    rows,
                    [
                        Constraint::Length(22),
                        Constraint::Length(12),
                        Constraint::Length(6),
                        Constraint::Min(10),
                    ],
                )
                .header(
                    Row::new(vec!["TASK", "AGENT", "ATTEST", "OUTCOME"])
                        .style(Style::default().add_modifier(Modifier::BOLD)),
                )
                .block(Block::default().borders(Borders::ALL).title(format!(
                    " {} — {} of {} tasks ",
                    group.state.label(),
                    group.checked,
                    group.total
                )))
                .column_spacing(1);
                frame.render_widget(table, area);
                y = y.saturating_add(group_height);
            }
        })
        .map_err(|_| EXIT_INVARIANT)?;
    let buffer = terminal.backend().buffer();
    let mut output = if report.empty {
        "FLEET STATUS — empty store; 0 of 0 tasks\n".to_string()
    } else {
        format!(
            "FLEET STATUS — {} of {} tasks from receipts\n",
            report.checked, report.total
        )
    };
    for y in 0..buffer.area.height {
        let line = (0..buffer.area.width)
            .map(|x| buffer.cell((x, y)).map(|cell| cell.symbol()).unwrap_or(" "))
            .collect::<String>();
        output.push_str(line.trim_end());
        output.push('\n');
    }
    Ok(output)
}

fn display(value: Option<&str>) -> String {
    value.unwrap_or("—").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(seq: u64, event: &str, body: Value) -> Value {
        json!({
            "seq":seq,
            "event":event,
            "hash":format!("receipt-{seq}"),
            "body":body
        })
    }

    #[test]
    fn receipt_derivation_places_one_task_in_every_bucket() {
        let rows = vec![
            row(
                0,
                "run_start",
                json!({"run_id":"done","task":"ship","agent":"a"}),
            ),
            row(1, "artifact_frozen", json!({"artifact_id":"good"})),
            row(2, "attested", json!({"artifact_id":"good"})),
            row(
                3,
                "run_end",
                json!({"run_id":"done","artifact_id":"good","status":"ok"}),
            ),
            row(
                4,
                "run_start",
                json!({"run_id":"pending","task":"wait","agent":"b"}),
            ),
            row(
                5,
                "run_start",
                json!({"run_id":"failed","task":"break","agent":"c"}),
            ),
            row(
                6,
                "run_end",
                json!({"run_id":"failed","status":"agent_failed"}),
            ),
            row(
                7,
                "run_start",
                json!({"run_id":"iterate","task":"retry","agent":"d"}),
            ),
            row(8, "artifact_frozen", json!({"artifact_id":"bad"})),
            row(9, "attested", json!({"artifact_id":"bad"})),
            row(
                10,
                "run_end",
                json!({"run_id":"iterate","artifact_id":"bad","status":"ok"}),
            ),
        ];
        let tasks = derive_tasks(&rows, |artifact| artifact == "good");
        assert_eq!(tasks.len(), 4);
        for state in STATES {
            assert_eq!(tasks.iter().filter(|task| task.state == state).count(), 1);
        }
        let report = report(&tasks);
        assert!(report.groups.iter().all(|group| group.total == 4));
    }

    #[test]
    fn refusal_after_frozen_artifact_needs_iteration() {
        let rows = vec![
            row(
                0,
                "run_start",
                json!({"run_id":"r","task":"landed","agent":"a"}),
            ),
            row(1, "artifact_frozen", json!({"artifact_id":"x"})),
            row(2, "refusal", json!({"reason":"SELF_VERIFIED"})),
        ];
        let tasks = derive_tasks(&rows, |_| false);
        assert_eq!(tasks[0].state, TaskState::NeedsIteration);
        assert!(tasks[0]
            .outcome
            .as_deref()
            .unwrap()
            .contains("SELF_VERIFIED"));
    }

    #[test]
    fn named_refusal_does_not_overwrite_a_different_pending_task() {
        let rows = vec![
            row(
                0,
                "run_start",
                json!({"run_id":"r","task":"still running","agent":"a"}),
            ),
            row(
                1,
                "refusal",
                json!({"task":"other task","agent":"b","reason":"NO"}),
            ),
        ];
        let tasks = derive_tasks(&rows, |_| false);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].state, TaskState::Pending);
        assert_eq!(tasks[1].state, TaskState::Failed);
    }

    #[test]
    fn empty_store_is_explicit_with_zero_denominators() {
        let tasks = derive_tasks(&[], |_| false);
        let report = report(&tasks);
        assert!(report.empty);
        assert_eq!(report.checked, 0);
        assert_eq!(report.total, 0);
        assert!(report
            .groups
            .iter()
            .all(|group| group.checked == 0 && group.total == 0));
        let rendered = render_human(&report).expect("render empty status");
        assert!(rendered.contains("empty store; 0 of 0 tasks"));
        assert!(rendered.contains("NEEDS-ITERATION — 0 of 0 tasks"));
    }
}
