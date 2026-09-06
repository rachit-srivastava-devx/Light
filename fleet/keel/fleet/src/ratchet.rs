use fs2::FileExt;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const EXIT_ENV: i32 = 3;
const EXIT_INVARIANT: i32 = 6;
const EXIT_REFUSAL: i32 = 7;
const EXIT_MISMATCH: i32 = 8;
const RATE_SCALE: u64 = 1_000_000;

pub fn wilson_95(k: u64, n: u64) -> Option<(f64, f64)> {
    if n == 0 {
        return None;
    }
    let z = 1.959963984540054;
    let p = k as f64 / n as f64;
    let denominator = 1.0 + z * z / n as f64;
    let center = (p + z * z / (2.0 * n as f64)) / denominator;
    let margin =
        z * ((p * (1.0 - p) / n as f64 + z * z / (4.0 * n as f64 * n as f64)).sqrt()) / denominator;
    Some((center - margin, center + margin))
}

pub fn adequacy_verdict(killed: u64, total: u64, mark_rate: f64, mark_count: u64) -> bool {
    if total == 0 || killed > total {
        return false;
    }
    let rate = killed as f64 / total as f64;
    rate >= mark_rate && total >= mark_count
}

#[derive(Clone, Copy)]
enum MetricKind {
    Rate,
    Count,
}

#[derive(Clone, Copy)]
struct MetricSpec {
    name: &'static str,
    kind: MetricKind,
    higher_is_better: bool,
}

const METRICS: [MetricSpec; 7] = [
    MetricSpec {
        name: "mutation_kill_rate",
        kind: MetricKind::Rate,
        higher_is_better: true,
    },
    MetricSpec {
        name: "mutant_count",
        kind: MetricKind::Count,
        higher_is_better: true,
    },
    MetricSpec {
        name: "acceptance_pass_rate",
        kind: MetricKind::Rate,
        higher_is_better: true,
    },
    MetricSpec {
        name: "arch_violations",
        kind: MetricKind::Count,
        higher_is_better: false,
    },
    MetricSpec {
        name: "p95_dispatch_latency_ms",
        kind: MetricKind::Count,
        higher_is_better: false,
    },
    MetricSpec {
        name: "change_failure_rate",
        kind: MetricKind::Rate,
        higher_is_better: false,
    },
    MetricSpec {
        name: "rework_rate",
        kind: MetricKind::Rate,
        higher_is_better: false,
    },
];

#[derive(Clone)]
struct MetricValue {
    kind: MetricKind,
    value: u64,
}

#[derive(Clone)]
struct Scorecard {
    change: String,
    checked: u64,
    total: u64,
    values: BTreeMap<String, MetricValue>,
}

#[derive(Clone)]
struct Mark {
    value: MetricValue,
    set_at: String,
    set_by_change: String,
}

struct RatchetError {
    code: i32,
    reason: String,
    denominator: Option<(u64, u64)>,
}

type Result<T> = std::result::Result<T, RatchetError>;

impl RatchetError {
    fn new(code: i32, reason: impl Into<String>) -> Self {
        Self {
            code,
            reason: reason.into(),
            denominator: None,
        }
    }

    fn with_denominator(mut self, checked: u64, total: u64) -> Self {
        self.denominator = Some((checked, total));
        self
    }
}

struct RatchetLock(File);

impl RatchetLock {
    fn acquire(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|_| RatchetError::new(EXIT_ENV, "unable to open ratchet lock"))?;
        file.lock_exclusive()
            .map_err(|_| RatchetError::new(EXIT_ENV, "unable to lock ratchet state"))?;
        Ok(Self(file))
    }
}

impl Drop for RatchetLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub(crate) fn command(args: &[String]) -> std::result::Result<(), i32> {
    let result = command_inner(args);
    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            eprintln!("{}", error.reason);
            if is_decision_command(args) {
                let (checked, total) = error.denominator.unwrap_or_default();
                let body = serde_json::json!({
                    "status": if error.code == EXIT_REFUSAL { "refused" } else { "failed" },
                    "reason": error.reason,
                    "checked": checked,
                    "total": total,
                });
                let recorded = crate::append_receipt(
                    "gate_verdict",
                    body,
                    "fleet-ratchet",
                    None,
                    Some(error.code),
                );
                recorded?;
                if error.code == EXIT_REFUSAL
                    && matches!(args.first().map(String::as_str), Some("check" | "submit"))
                {
                    if let Some(change) = args
                        .windows(2)
                        .find(|pair| pair[0] == "--change")
                        .map(|pair| pair[1].as_str())
                    {
                        let _ = crate::heal_ledger_artifact(change, "below_ratchet");
                    }
                }
            }
            Err(error.code)
        }
    }
}

fn is_decision_command(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("seed" | "check" | "submit" | "adequacy")
    )
}

fn command_inner(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("show") if args.len() == 1 => show(),
        Some("seed") => decision(args, false),
        Some("check" | "submit") if args.len() == 1 => Err(RatchetError::new(
            EXIT_REFUSAL,
            "USAGE\n  fleet ratchet check --metrics <SCORECARD_PATH> --change <ID>\n\nexample:\n  fleet ratchet check --metrics \"$FLEET_STATE/scorecards/change.json\" --change change-123",
        )),
        Some("check") | Some("submit") => decision(args, true),
        Some("adequacy") => adequacy_command(&args[1..]),
        Some("exception") => exception_command(&args[1..]),
        _ => Err(RatchetError::new(
            EXIT_REFUSAL,
            "ratchet command is required",
        )),
    }
}

/// Record the quality bar for an evidenced swarm dispatch. The first completed
/// dispatch seeds the bar; subsequent dispatches must meet it. Keeping this in
/// the ratchet module makes dispatch use the same locked, atomic marks store as
/// the public ratchet commands.
pub(crate) fn record_dispatch(
    state: &Path,
    change: &str,
    checked: u64,
    total: u64,
) -> std::result::Result<(), i32> {
    let result = record_dispatch_inner(state, change, checked, total);
    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            eprintln!("{}", error.reason);
            let (denominator_checked, denominator_total) =
                error.denominator.unwrap_or((checked, total));
            crate::append_receipt(
                "gate_verdict",
                serde_json::json!({
                    "kind":"dispatch_ratchet",
                    "status":if error.code == EXIT_REFUSAL { "refused" } else { "failed" },
                    "reason":error.reason,
                    "checked":denominator_checked,
                    "total":denominator_total,
                }),
                "fleet-ratchet",
                None,
                Some(error.code),
            )?;
            Err(error.code)
        }
    }
}

fn record_dispatch_inner(state: &Path, change: &str, checked: u64, total: u64) -> Result<()> {
    if change.trim().is_empty() || total == 0 || checked > total {
        return Err(
            RatchetError::new(EXIT_INVARIANT, "dispatch ratchet score is invalid")
                .with_denominator(checked, total),
        );
    }
    let scaled = checked
        .checked_mul(RATE_SCALE)
        .ok_or_else(|| RatchetError::new(EXIT_INVARIANT, "dispatch score overflowed"))?
        / total;
    let ratchet = state.join("ratchet");
    fs::create_dir_all(&ratchet)
        .map_err(|_| RatchetError::new(EXIT_ENV, "unable to create ratchet state"))?;
    let _lock = RatchetLock::acquire(&ratchet.join("ratchet.lock"))?;
    let mark_path = ratchet.join("dispatch-mark.json");
    let existing = read_dispatch_mark(&mark_path)?;
    let now = crate::now_rfc3339()
        .map_err(|code| RatchetError::new(code, "unable to timestamp dispatch mark"))?;
    let actual = MetricValue {
        kind: MetricKind::Rate,
        value: scaled,
    };
    if let Some(mark) = existing.as_ref() {
        if actual.value < mark.value.value {
            return Err(RatchetError::new(
                EXIT_REFUSAL,
                format!(
                    "REFUSED metric=dispatch_acceptance actual={} mark={}",
                    display_value(&actual),
                    display_value(&mark.value)
                ),
            )
            .with_denominator(checked, total));
        }
    }
    let next = match existing {
        Some(mark) if mark.value.value >= actual.value => mark,
        _ => Mark {
            value: actual.clone(),
            set_at: now,
            set_by_change: change.to_string(),
        },
    };
    write_dispatch_mark(&mark_path, &next)?;
    append_dispatch_scorecard(&ratchet, change, checked, total, &actual)?;
    Ok(())
}

fn read_dispatch_mark(path: &Path) -> Result<Option<Mark>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(RatchetError::new(EXIT_ENV, "unable to read dispatch mark")),
    };
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RatchetError::new(EXIT_MISMATCH, "dispatch mark is not valid JSON"))?;
    if value.get("schema_version").and_then(Value::as_str) != Some("1")
        || value.get("metric").and_then(Value::as_str) != Some("dispatch_acceptance")
    {
        return Err(RatchetError::new(
            EXIT_MISMATCH,
            "dispatch mark schema is invalid",
        ));
    }
    let rate = value
        .get("value")
        .and_then(Value::as_str)
        .and_then(parse_rate)
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "dispatch mark value is invalid"))?;
    let set_at = value
        .get("set_at")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "dispatch mark timestamp is missing"))?;
    let set_by_change = value
        .get("set_by_change")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "dispatch mark provenance is missing"))?;
    Ok(Some(Mark {
        value: MetricValue {
            kind: MetricKind::Rate,
            value: rate,
        },
        set_at: set_at.to_string(),
        set_by_change: set_by_change.to_string(),
    }))
}

fn write_dispatch_mark(path: &Path, mark: &Mark) -> Result<()> {
    let value = serde_json::json!({
        "schema_version":"1",
        "metric":"dispatch_acceptance",
        "value":display_value(&mark.value),
        "set_at":mark.set_at,
        "set_by_change":mark.set_by_change,
    });
    crate::write_json_atomic(path, &value)
        .map_err(|code| RatchetError::new(code, "unable to persist dispatch mark"))
}

fn append_dispatch_scorecard(
    ratchet: &Path,
    change: &str,
    checked: u64,
    total: u64,
    actual: &MetricValue,
) -> Result<()> {
    let line = serde_json::to_string(&serde_json::json!({
        "schema_version":"1",
        "change":change,
        "metric":"dispatch_acceptance",
        "value":display_value(actual),
        "checked":checked,
        "total":total,
        "status":"accepted",
    }))
    .map_err(|_| RatchetError::new(EXIT_INVARIANT, "unable to encode dispatch scorecard"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(ratchet.join("dispatch-scorecards.jsonl"))
        .map_err(|_| RatchetError::new(EXIT_ENV, "unable to open dispatch scorecards"))?;
    writeln!(file, "{line}")
        .map_err(|_| RatchetError::new(EXIT_ENV, "unable to append dispatch scorecard"))?;
    file.sync_all()
        .map_err(|_| RatchetError::new(EXIT_ENV, "unable to sync dispatch scorecard"))
}

fn adequacy_command(args: &[String]) -> Result<()> {
    let mut killed = None;
    let mut total = None;
    let mut index = 0usize;
    while index < args.len() {
        let key = args[index].as_str();
        let value = args
            .get(index + 1)
            .ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "adequacy option requires a value"))?
            .parse::<u64>()
            .map_err(|_| RatchetError::new(EXIT_REFUSAL, "adequacy values must be integers"))?;
        match key {
            "--killed" => killed = Some(value),
            "--total" => total = Some(value),
            _ => {
                return Err(RatchetError::new(
                    EXIT_REFUSAL,
                    format!("unknown adequacy option={}", key),
                ))
            }
        }
        index += 2;
    }
    let killed = killed.ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "--killed is required"))?;
    let total = total.ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "--total is required"))?;
    let state =
        crate::state_dir().map_err(|code| RatchetError::new(code, "FLEET_STATE is unavailable"))?;
    let marks = read_marks(&state.join("ratchet/marks.json"))?
        .ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "ratchet marks are not seeded"))?;
    let mark_rate = marks
        .get("mutation_kill_rate")
        .ok_or_else(|| {
            RatchetError::new(
                EXIT_MISMATCH,
                "ratchet mark is missing metric=mutation_kill_rate",
            )
        })?
        .value
        .value as f64
        / RATE_SCALE as f64;
    let mark_count = marks
        .get("mutant_count")
        .ok_or_else(|| {
            RatchetError::new(EXIT_MISMATCH, "ratchet mark is missing metric=mutant_count")
        })?
        .value
        .value;
    let interval = wilson_95(killed, total);
    let verdict = adequacy_verdict(killed, total, mark_rate, mark_count);
    let rate = if total == 0 {
        "unavailable".to_string()
    } else {
        format!("{:.4}", killed as f64 / total as f64)
    };
    let interval_text = interval
        .map(|(lower, upper)| format!("({:.4},{:.4})", lower, upper))
        .unwrap_or_else(|| "unavailable".to_string());
    println!(
        "rate={} interval={} count={} verdict={}",
        rate,
        interval_text,
        total,
        if verdict { "pass" } else { "fail" }
    );
    if total == 0 {
        return Err(
            RatchetError::new(EXIT_INVARIANT, "adequacy total must be greater than zero")
                .with_denominator(total, total),
        );
    }
    if killed > total {
        return Err(
            RatchetError::new(EXIT_INVARIANT, "adequacy killed cannot exceed total")
                .with_denominator(total, total),
        );
    }
    if !verdict {
        return Err(RatchetError::new(
            EXIT_REFUSAL,
            "adequacy refused: rate or mutant count regressed",
        )
        .with_denominator(total, total));
    }
    Ok(())
}

fn decision(args: &[String], compare: bool) -> Result<()> {
    let parsed = parse_decision_args(&args[1..])?;
    let state =
        crate::state_dir().map_err(|code| RatchetError::new(code, "FLEET_STATE is unavailable"))?;
    let ratchet = state.join("ratchet");
    fs::create_dir_all(&ratchet)
        .map_err(|_| RatchetError::new(EXIT_ENV, "unable to create ratchet state"))?;
    let _lock = RatchetLock::acquire(&ratchet.join("ratchet.lock"))?;
    let scorecard = load_scorecard(&parsed.metrics_path, &parsed.change)?;
    let marks_path = ratchet.join("marks.json");
    let existing = read_marks(&marks_path)?;
    if compare {
        let marks = existing
            .ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "ratchet marks are not seeded"))?;
        let waived = match parsed.exception.as_ref() {
            Some(path) => read_exception(path, &scorecard.change)?,
            None => BTreeSet::new(),
        };
        let mut regressions = Vec::new();
        for spec in METRICS {
            let actual = scorecard.values.get(spec.name).ok_or_else(|| {
                RatchetError::new(
                    EXIT_MISMATCH,
                    format!("scorecard is missing metric={}", spec.name),
                )
            })?;
            let mark = marks.get(spec.name).ok_or_else(|| {
                RatchetError::new(
                    EXIT_MISMATCH,
                    format!("ratchet mark is missing metric={}", spec.name),
                )
            })?;
            if is_regression(spec, actual, &mark.value) && !waived.contains(spec.name) {
                regressions.push(format!(
                    "metric={} actual={} mark={} set_at={} set_by_change={}",
                    spec.name,
                    display_value(actual),
                    display_value(&mark.value),
                    mark.set_at,
                    mark.set_by_change
                ));
            }
        }
        if let Some(regression) = regressions.first() {
            return Err(
                RatchetError::new(EXIT_REFUSAL, format!("REFUSED {}", regression))
                    .with_denominator(scorecard.checked, scorecard.total),
            );
        }
        let mut advanced = 0u64;
        let now = crate::now_rfc3339()
            .map_err(|code| RatchetError::new(code, "unable to timestamp ratchet mark"))?;
        let mut next = marks;
        for spec in METRICS {
            let actual = scorecard.values.get(spec.name).ok_or_else(|| {
                RatchetError::new(
                    EXIT_MISMATCH,
                    format!("scorecard is missing metric={}", spec.name),
                )
            })?;
            let mark = next.get(spec.name).ok_or_else(|| {
                RatchetError::new(
                    EXIT_MISMATCH,
                    format!("ratchet mark is missing metric={}", spec.name),
                )
            })?;
            if is_improvement(spec, actual, &mark.value) {
                next.insert(
                    spec.name.to_string(),
                    Mark {
                        value: actual.clone(),
                        set_at: now.clone(),
                        set_by_change: scorecard.change.clone(),
                    },
                );
                advanced += 1;
            }
        }
        write_marks(&marks_path, &next)?;
        append_scorecard(&ratchet, &scorecard, "accepted", advanced)?;
        append_verdict(&scorecard, "accepted", advanced, None)?;
        println!(
            "accepted checked={} total={} advanced={}",
            scorecard.checked, scorecard.total, advanced
        );
        Ok(())
    } else {
        if existing.is_some() {
            return Err(RatchetError::new(
                EXIT_REFUSAL,
                "ratchet marks already exist; seed is immutable",
            ));
        }
        let now = crate::now_rfc3339()
            .map_err(|code| RatchetError::new(code, "unable to timestamp ratchet mark"))?;
        let mut marks = BTreeMap::new();
        for spec in METRICS {
            let value = scorecard.values.get(spec.name).ok_or_else(|| {
                RatchetError::new(
                    EXIT_MISMATCH,
                    format!("scorecard is missing metric={}", spec.name),
                )
            })?;
            marks.insert(
                spec.name.to_string(),
                Mark {
                    value: value.clone(),
                    set_at: now.clone(),
                    set_by_change: scorecard.change.clone(),
                },
            );
        }
        write_marks(&marks_path, &marks)?;
        append_scorecard(&ratchet, &scorecard, "seeded", 7)?;
        append_verdict(&scorecard, "seeded", 7, None)?;
        println!(
            "seeded checked={} total={} marks=7",
            scorecard.checked, scorecard.total
        );
        Ok(())
    }
}

struct DecisionArgs {
    metrics_path: PathBuf,
    change: String,
    exception: Option<PathBuf>,
}

fn parse_decision_args(args: &[String]) -> Result<DecisionArgs> {
    let mut metrics_path = None;
    let mut change = None;
    let mut exception = None;
    let mut index = 0usize;
    while index < args.len() {
        let key = args[index].as_str();
        let value = args
            .get(index + 1)
            .ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "ratchet option requires a value"))?
            .clone();
        match key {
            "--metrics" => metrics_path = Some(PathBuf::from(value)),
            "--change" => change = Some(value),
            "--exception" => exception = Some(PathBuf::from(value)),
            _ => {
                return Err(RatchetError::new(
                    EXIT_REFUSAL,
                    format!("unknown ratchet option={}", key),
                ))
            }
        }
        index += 2;
    }
    let metrics_path =
        metrics_path.ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "--metrics is required"))?;
    let change = change.ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "--change is required"))?;
    if change.trim().is_empty() {
        return Err(RatchetError::new(EXIT_REFUSAL, "change id is empty"));
    }
    Ok(DecisionArgs {
        metrics_path,
        change,
        exception,
    })
}

fn load_scorecard(path: &Path, change: &str) -> Result<Scorecard> {
    let bytes = fs::read(path).map_err(|error| {
        let expected = crate::state_dir()
            .map(|state| state.join("scorecards").join(format!("{change}.json")))
            .unwrap_or_else(|_| PathBuf::from("$FLEET_STATE/scorecards").join(format!("{change}.json")));
        RatchetError::new(
            EXIT_ENV,
            format!(
                "unable to read scorecard PATH={} ({error}); --metrics expects a JSON scorecard path, conventionally {} under $FLEET_STATE. Create that file, then run `fleet ratchet check --metrics {} --change {change}`",
                path.display(),
                expected.display(),
                expected.display(),
            ),
        )
    })?;
    let root: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RatchetError::new(EXIT_MISMATCH, "scorecard is not valid JSON"))?;
    let object = root
        .as_object()
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "scorecard must be a JSON object"))?;
    let checked = object
        .get("checked")
        .and_then(Value::as_u64)
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "scorecard.checked must be an integer"))?;
    let total = object
        .get("total")
        .and_then(Value::as_u64)
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "scorecard.total must be an integer"))?;
    if checked == 0 || total == 0 || checked > total {
        return Err(RatchetError::new(
            EXIT_INVARIANT,
            format!(
                "scorecard denominator invalid checked={} total={}",
                checked, total
            ),
        ));
    }
    let embedded_change = object.get("change").and_then(Value::as_str);
    if let Some(embedded) = embedded_change {
        if embedded != change {
            return Err(RatchetError::new(
                EXIT_MISMATCH,
                "scorecard change does not match --change",
            ));
        }
    }
    let mut values = BTreeMap::new();
    for spec in METRICS {
        let value = object.get(spec.name).ok_or_else(|| {
            RatchetError::new(
                EXIT_MISMATCH,
                format!("scorecard is missing metric={}", spec.name),
            )
        })?;
        let parsed = match spec.kind {
            MetricKind::Rate => {
                let text = value.as_str().ok_or_else(|| {
                    RatchetError::new(
                        EXIT_MISMATCH,
                        format!("metric={} must be a fixed-point string", spec.name),
                    )
                })?;
                MetricValue {
                    kind: spec.kind,
                    value: parse_rate(text).ok_or_else(|| {
                        RatchetError::new(
                            EXIT_MISMATCH,
                            format!("metric={} is not a valid fixed-point rate", spec.name),
                        )
                    })?,
                }
            }
            MetricKind::Count => MetricValue {
                kind: spec.kind,
                value: value.as_u64().ok_or_else(|| {
                    RatchetError::new(
                        EXIT_MISMATCH,
                        format!("metric={} must be an integer", spec.name),
                    )
                })?,
            },
        };
        values.insert(spec.name.to_string(), parsed);
    }
    Ok(Scorecard {
        change: change.to_string(),
        checked,
        total,
        values,
    })
}

fn parse_rate(text: &str) -> Option<u64> {
    if text.is_empty()
        || text.trim() != text
        || text.starts_with('+')
        || text.starts_with('-')
        || text.contains('e')
        || text.contains('E')
    {
        return None;
    }
    let mut pieces = text.split('.');
    let whole = pieces.next()?;
    let fraction = pieces.next().unwrap_or("");
    if pieces.next().is_some()
        || whole.is_empty()
        || whole.bytes().any(|byte| !byte.is_ascii_digit())
        || fraction.len() > 6
        || fraction.bytes().any(|byte| !byte.is_ascii_digit())
    {
        return None;
    }
    let whole_value: u64 = whole.parse().ok()?;
    if whole_value > 1 {
        return None;
    }
    if whole_value == 1 && fraction.bytes().any(|byte| byte != b'0') {
        return None;
    }
    let mut scaled_fraction = fraction.to_string();
    while scaled_fraction.len() < 6 {
        scaled_fraction.push('0');
    }
    let fraction_value: u64 = if scaled_fraction.is_empty() {
        0
    } else {
        scaled_fraction.parse().ok()?
    };
    whole_value
        .checked_mul(RATE_SCALE)?
        .checked_add(fraction_value)
}

fn display_value(value: &MetricValue) -> String {
    match value.kind {
        MetricKind::Count => value.value.to_string(),
        MetricKind::Rate => {
            let whole = value.value / RATE_SCALE;
            let fraction = format!("{:06}", value.value % RATE_SCALE);
            let trimmed = fraction.trim_end_matches('0');
            if trimmed.is_empty() {
                whole.to_string()
            } else {
                format!("{}.{}", whole, trimmed)
            }
        }
    }
}

fn is_regression(spec: MetricSpec, actual: &MetricValue, mark: &MetricValue) -> bool {
    if spec.higher_is_better {
        actual.value < mark.value
    } else {
        actual.value > mark.value
    }
}

fn is_improvement(spec: MetricSpec, actual: &MetricValue, mark: &MetricValue) -> bool {
    if spec.higher_is_better {
        actual.value > mark.value
    } else {
        actual.value < mark.value
    }
}

fn read_marks(path: &Path) -> Result<Option<BTreeMap<String, Mark>>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(RatchetError::new(EXIT_ENV, "unable to read ratchet marks")),
    };
    let root: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RatchetError::new(EXIT_MISMATCH, "ratchet marks are not valid JSON"))?;
    let object = root
        .as_object()
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "ratchet marks must be a JSON object"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("1") {
        return Err(RatchetError::new(
            EXIT_MISMATCH,
            "ratchet marks schema_version must be 1",
        ));
    }
    let marks = object
        .get("marks")
        .and_then(Value::as_object)
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "ratchet marks.mark is required"))?;
    let mut parsed = BTreeMap::new();
    for spec in METRICS {
        let mark = marks
            .get(spec.name)
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RatchetError::new(
                    EXIT_MISMATCH,
                    format!("ratchet mark is missing metric={}", spec.name),
                )
            })?;
        let value = mark.get("value").ok_or_else(|| {
            RatchetError::new(
                EXIT_MISMATCH,
                format!("ratchet mark value is missing metric={}", spec.name),
            )
        })?;
        let parsed_value = match spec.kind {
            MetricKind::Rate => MetricValue {
                kind: spec.kind,
                value: parse_rate(value.as_str().ok_or_else(|| {
                    RatchetError::new(
                        EXIT_MISMATCH,
                        format!(
                            "ratchet mark metric={} must be a fixed-point string",
                            spec.name
                        ),
                    )
                })?)
                .ok_or_else(|| {
                    RatchetError::new(
                        EXIT_MISMATCH,
                        format!("ratchet mark metric={} has invalid rate", spec.name),
                    )
                })?,
            },
            MetricKind::Count => MetricValue {
                kind: spec.kind,
                value: value.as_u64().ok_or_else(|| {
                    RatchetError::new(
                        EXIT_MISMATCH,
                        format!("ratchet mark metric={} must be an integer", spec.name),
                    )
                })?,
            },
        };
        let set_at = mark
            .get("set_at")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RatchetError::new(
                    EXIT_MISMATCH,
                    format!("ratchet mark timestamp is missing metric={}", spec.name),
                )
            })?
            .to_string();
        let set_by_change = mark
            .get("set_by_change")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RatchetError::new(
                    EXIT_MISMATCH,
                    format!("ratchet mark provenance is missing metric={}", spec.name),
                )
            })?
            .to_string();
        if set_by_change.trim().is_empty() || set_at.trim().is_empty() {
            return Err(RatchetError::new(
                EXIT_MISMATCH,
                format!("ratchet mark provenance is empty metric={}", spec.name),
            ));
        }
        parsed.insert(
            spec.name.to_string(),
            Mark {
                value: parsed_value,
                set_at,
                set_by_change,
            },
        );
    }
    Ok(Some(parsed))
}

fn write_marks(path: &Path, marks: &BTreeMap<String, Mark>) -> Result<()> {
    let mut mark_values = Map::new();
    for spec in METRICS {
        let mark = marks.get(spec.name).ok_or_else(|| {
            RatchetError::new(
                EXIT_MISMATCH,
                format!("cannot write missing metric={}", spec.name),
            )
        })?;
        let mut object = Map::new();
        let value = match mark.value.kind {
            MetricKind::Rate => Value::String(display_value(&mark.value)),
            MetricKind::Count => Value::Number(mark.value.value.into()),
        };
        object.insert("value".to_string(), value);
        object.insert("set_at".to_string(), Value::String(mark.set_at.clone()));
        object.insert(
            "set_by_change".to_string(),
            Value::String(mark.set_by_change.clone()),
        );
        mark_values.insert(spec.name.to_string(), Value::Object(object));
    }
    let root = serde_json::json!({"schema_version":"1","marks":mark_values});
    crate::write_json_atomic(path, &root)
        .map_err(|code| RatchetError::new(code, "unable to persist ratchet marks"))
}

fn show() -> Result<()> {
    let state =
        crate::state_dir().map_err(|code| RatchetError::new(code, "FLEET_STATE is unavailable"))?;
    let path = state.join("ratchet/marks.json");
    let marks = read_marks(&path)?;
    let dispatch_mark = read_dispatch_mark(&state.join("ratchet/dispatch-mark.json"))?;
    if marks.is_none() && dispatch_mark.is_none() {
        return Err(RatchetError::new(EXIT_INVARIANT, "ratchet has no marks"));
    }
    if let Some(marks) = marks {
        for spec in METRICS {
            let mark = marks.get(spec.name).ok_or_else(|| {
                RatchetError::new(
                    EXIT_MISMATCH,
                    format!("ratchet mark is missing metric={}", spec.name),
                )
            })?;
            println!(
                "{} mark={} set_at={} set_by_change={}",
                spec.name,
                display_value(&mark.value),
                mark.set_at,
                mark.set_by_change
            );
        }
    }
    if let Some(mark) = dispatch_mark {
        println!(
            "dispatch_acceptance mark={} set_at={} set_by_change={}",
            display_value(&mark.value),
            mark.set_at,
            mark.set_by_change
        );
    }
    Ok(())
}

fn append_scorecard(
    ratchet: &Path,
    scorecard: &Scorecard,
    status: &str,
    advanced: u64,
) -> Result<()> {
    let path = ratchet.join("scorecards.jsonl");
    let mut object = Map::new();
    object.insert("schema_version".to_string(), Value::String("1".to_string()));
    object.insert(
        "change".to_string(),
        Value::String(scorecard.change.clone()),
    );
    object.insert(
        "checked".to_string(),
        Value::Number(scorecard.checked.into()),
    );
    object.insert("total".to_string(), Value::Number(scorecard.total.into()));
    object.insert("status".to_string(), Value::String(status.to_string()));
    object.insert("advanced".to_string(), Value::Number(advanced.into()));
    object.insert(
        "ts_wall".to_string(),
        Value::String(
            crate::now_rfc3339()
                .map_err(|code| RatchetError::new(code, "unable to timestamp scorecard"))?,
        ),
    );
    let mut values = Map::new();
    for spec in METRICS {
        let value = scorecard.values.get(spec.name).ok_or_else(|| {
            RatchetError::new(
                EXIT_MISMATCH,
                format!("scorecard is missing metric={}", spec.name),
            )
        })?;
        values.insert(
            spec.name.to_string(),
            match value.kind {
                MetricKind::Rate => Value::String(display_value(value)),
                MetricKind::Count => Value::Number(value.value.into()),
            },
        );
    }
    object.insert("metrics".to_string(), Value::Object(values));
    let line = serde_json::to_string(&Value::Object(object))
        .map_err(|_| RatchetError::new(EXIT_INVARIANT, "unable to encode scorecard"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|_| RatchetError::new(EXIT_ENV, "unable to append scorecard"))?;
    file.write_all(line.as_bytes())
        .map_err(|_| RatchetError::new(EXIT_ENV, "unable to append scorecard"))?;
    file.write_all(b"\n")
        .map_err(|_| RatchetError::new(EXIT_ENV, "unable to append scorecard"))?;
    file.sync_data()
        .map_err(|_| RatchetError::new(EXIT_ENV, "unable to sync scorecard"))
}

fn append_verdict(
    scorecard: &Scorecard,
    status: &str,
    advanced: u64,
    error_code: Option<i32>,
) -> Result<()> {
    let body = serde_json::json!({
        "status": status,
        "change": scorecard.change,
        "checked": scorecard.checked,
        "total": scorecard.total,
        "advanced": advanced,
    });
    crate::append_receipt("gate_verdict", body, "fleet-ratchet", None, error_code)
        .map(|_| ())
        .map_err(|code| RatchetError::new(code, "unable to append ratchet verdict"))
}

fn exception_command(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("create") => create_exception(&args[1..]),
        _ => Err(RatchetError::new(
            EXIT_REFUSAL,
            "ratchet exception create is required",
        )),
    }
}

fn create_exception(args: &[String]) -> Result<()> {
    let mut change = None;
    let mut expires_at = None;
    let mut reason = None;
    let mut signed_by = None;
    let mut output = None;
    let mut metrics = Vec::new();
    let mut index = 0usize;
    while index < args.len() {
        let key = args[index].as_str();
        let value = args
            .get(index + 1)
            .ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "exception option requires a value"))?
            .clone();
        match key {
            "--change" => change = Some(value),
            "--metric" => metrics.push(value),
            "--expires-at" => expires_at = Some(value),
            "--reason" => reason = Some(value),
            "--signed-by" => signed_by = Some(value),
            "--output" => output = Some(PathBuf::from(value)),
            _ => {
                return Err(RatchetError::new(
                    EXIT_REFUSAL,
                    format!("unknown exception option={}", key),
                ))
            }
        }
        index += 2;
    }
    let change =
        change.ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "exception --change is required"))?;
    let expires_at = expires_at
        .ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "exception --expires-at is required"))?;
    let reason =
        reason.ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "exception --reason is required"))?;
    let signed_by = signed_by
        .ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "exception --signed-by is required"))?;
    let output =
        output.ok_or_else(|| RatchetError::new(EXIT_REFUSAL, "exception --output is required"))?;
    validate_exception_scope(&change, &metrics, &expires_at, &reason, &signed_by)?;
    let mut scope = Map::new();
    scope.insert("change".to_string(), Value::String(change));
    scope.insert(
        "metrics".to_string(),
        Value::Array(metrics.into_iter().map(Value::String).collect()),
    );
    let mut unsigned = Map::new();
    unsigned.insert("schema_version".to_string(), Value::String("1".to_string()));
    unsigned.insert("scope".to_string(), Value::Object(scope));
    unsigned.insert("expires_at".to_string(), Value::String(expires_at));
    unsigned.insert("reason".to_string(), Value::String(reason));
    unsigned.insert("signed_by".to_string(), Value::String(signed_by));
    let signature = sign_object(&Value::Object(unsigned.clone()));
    unsigned.insert("signature".to_string(), Value::String(signature));
    crate::write_json_atomic(&output, &Value::Object(unsigned))
        .map_err(|code| RatchetError::new(code, "unable to write exception"))?;
    println!("exception={}", output.display());
    Ok(())
}

fn read_exception(path: &Path, change: &str) -> Result<BTreeSet<String>> {
    let bytes =
        fs::read(path).map_err(|_| RatchetError::new(EXIT_ENV, "unable to read exception"))?;
    let root: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RatchetError::new(EXIT_MISMATCH, "exception is not valid JSON"))?;
    let object = root
        .as_object()
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "exception must be a JSON object"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("1") {
        return Err(RatchetError::new(
            EXIT_MISMATCH,
            "exception schema_version must be 1",
        ));
    }
    let scope = object
        .get("scope")
        .and_then(Value::as_object)
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "exception scope is required"))?;
    if scope.len() != 2
        || object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "schema_version" | "scope" | "expires_at" | "reason" | "signed_by" | "signature"
            )
        })
    {
        return Err(RatchetError::new(
            EXIT_MISMATCH,
            "exception contains fields outside its signed scope",
        ));
    }
    let scoped_change = scope
        .get("change")
        .and_then(Value::as_str)
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "exception scope.change is required"))?;
    if scoped_change != change {
        return Err(RatchetError::new(
            EXIT_MISMATCH,
            "exception scope does not match change",
        ));
    }
    let metric_values = scope
        .get("metrics")
        .and_then(Value::as_array)
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "exception scope.metrics is required"))?;
    let mut metrics = BTreeSet::new();
    for value in metric_values {
        let metric = value
            .as_str()
            .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "exception metric must be a string"))?;
        if !METRICS.iter().any(|spec| spec.name == metric)
            || metric == "*"
            || !metrics.insert(metric.to_string())
        {
            return Err(RatchetError::new(
                EXIT_MISMATCH,
                "exception scope widens beyond known metrics",
            ));
        }
    }
    if metrics.is_empty() || metrics.len() >= METRICS.len() {
        return Err(RatchetError::new(
            EXIT_MISMATCH,
            "exception scope must narrow the metric set",
        ));
    }
    let expires_at = object
        .get("expires_at")
        .and_then(Value::as_str)
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "exception expires_at is required"))?;
    let reason = object
        .get("reason")
        .and_then(Value::as_str)
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "exception reason is required"))?;
    let signed_by = object
        .get("signed_by")
        .and_then(Value::as_str)
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "exception signed_by is required"))?;
    validate_exception_scope(
        scoped_change,
        &metrics.iter().cloned().collect::<Vec<_>>(),
        expires_at,
        reason,
        signed_by,
    )?;
    if expires_at
        <= crate::now_rfc3339()
            .map_err(|code| RatchetError::new(code, "unable to read current time"))?
            .as_str()
    {
        return Err(RatchetError::new(
            EXIT_INVARIANT,
            format!("expired exception expires_at={}", expires_at),
        ));
    }
    let signature = object
        .get("signature")
        .and_then(Value::as_str)
        .ok_or_else(|| RatchetError::new(EXIT_MISMATCH, "exception signature is required"))?;
    let mut unsigned = object.clone();
    unsigned.remove("signature");
    if signature != sign_object(&Value::Object(unsigned)) {
        return Err(RatchetError::new(
            EXIT_MISMATCH,
            "exception signature does not verify",
        ));
    }
    Ok(metrics)
}

fn validate_exception_scope(
    change: &str,
    metrics: &[String],
    expires_at: &str,
    reason: &str,
    signed_by: &str,
) -> Result<()> {
    if change.trim().is_empty()
        || metrics.is_empty()
        || metrics.len() >= METRICS.len()
        || expires_at.trim().is_empty()
        || reason.trim().is_empty()
        || signed_by.trim().is_empty()
    {
        return Err(RatchetError::new(
            EXIT_REFUSAL,
            "exception must be signed, expiring, and narrowing",
        ));
    }
    let mut unique = BTreeSet::new();
    for metric in metrics {
        if metric == "*"
            || !METRICS.iter().any(|spec| spec.name == metric)
            || !unique.insert(metric)
        {
            return Err(RatchetError::new(
                EXIT_REFUSAL,
                "exception scope widens beyond known metrics",
            ));
        }
    }
    if expires_at.len() < 20
        || !expires_at.ends_with('Z')
        || expires_at.as_bytes().get(10) != Some(&b'T')
    {
        return Err(RatchetError::new(
            EXIT_REFUSAL,
            "exception expires_at must be UTC RFC3339",
        ));
    }
    Ok(())
}

fn sign_object(value: &Value) -> String {
    format!("blake3:{}", crate::blake3_hex(&canonical(value)))
}

fn canonical(value: &Value) -> Vec<u8> {
    serde_json::to_vec(&canonical_value(value)).expect("JSON values are serializable")
}

fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut sorted = Map::new();
            let mut keys: Vec<_> = object.keys().collect();
            keys.sort();
            for key in keys {
                sorted.insert(key.clone(), canonical_value(&object[key]));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_value).collect()),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        adequacy_command, adequacy_verdict, is_improvement, parse_decision_args, parse_rate,
        read_exception, record_dispatch_inner, sign_object, validate_exception_scope, wilson_95,
        write_marks, Mark, MetricKind, MetricSpec, MetricValue, RatchetError, EXIT_MISMATCH,
        EXIT_REFUSAL, METRICS, RATE_SCALE,
    };
    use serde_json::{Map, Value};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn wilson_interval_for_26_of_44() {
        let (lower, upper) = wilson_95(26, 44).expect("non-empty measurement");
        assert!((lower - 0.4441).abs() < 0.0001);
        assert!((upper - 0.7231).abs() < 0.0001);
    }

    #[test]
    fn adequacy_requires_both_rate_and_count() {
        assert!(adequacy_verdict(26, 44, 0.5, 44));
        assert!(!adequacy_verdict(26, 44, 0.6, 44));
        assert!(!adequacy_verdict(26, 44, 0.5, 45));
        assert!(!adequacy_verdict(0, 0, 0.0, 0));
    }

    #[test]
    fn parse_rate_rejects_each_independent_lexical_violation() {
        for invalid in ["", " 0", "+0", "-0", "0e0", "0E0"] {
            assert_eq!(parse_rate(invalid), None, "accepted {invalid:?}");
        }
    }

    #[test]
    fn parse_rate_rejects_each_independent_component_violation() {
        for invalid in ["0.0.0", ".0", "a", "0.0000000", "0.a"] {
            assert_eq!(parse_rate(invalid), None, "accepted {invalid:?}");
        }
    }

    #[test]
    fn parse_rate_enforces_closed_unit_interval() {
        assert_eq!(parse_rate("0"), Some(0));
        assert_eq!(parse_rate("1"), Some(1_000_000));
        assert_eq!(parse_rate("2"), None);
        assert_eq!(parse_rate("1.0"), Some(1_000_000));
        assert_eq!(parse_rate("0.1"), Some(100_000));
        assert_eq!(parse_rate("1.1"), None);
    }

    #[test]
    fn parse_rate_scales_short_fractions_to_six_places() {
        assert_eq!(parse_rate("0.1"), Some(100_000));
        assert_eq!(parse_rate("0.12345"), Some(123_450));
        assert_eq!(parse_rate("0.123456"), Some(123_456));
    }

    fn improvement(higher_is_better: bool, actual: u64, mark: u64) -> bool {
        let spec = MetricSpec {
            name: "test_metric",
            kind: MetricKind::Count,
            higher_is_better,
        };
        let actual = MetricValue {
            kind: MetricKind::Count,
            value: actual,
        };
        let mark = MetricValue {
            kind: MetricKind::Count,
            value: mark,
        };
        is_improvement(spec, &actual, &mark)
    }

    #[test]
    fn improvement_is_strict_when_higher_is_better() {
        assert!(improvement(true, 11, 10));
        assert!(!improvement(true, 10, 10));
        assert!(!improvement(true, 9, 10));
    }

    #[test]
    fn improvement_is_strict_when_lower_is_better() {
        assert!(improvement(false, 9, 10));
        assert!(!improvement(false, 10, 10));
        assert!(!improvement(false, 11, 10));
    }

    // ---- from lane k-B ----

    const VALID_CHANGE: &str = "change-123";
    const VALID_EXPIRY: &str = "9999-12-31T23:59:59Z";
    const VALID_REASON: &str = "temporary regression waiver";
    const VALID_SIGNER: &str = "principal-engineer";

    fn metric_names(count: usize) -> Vec<String> {
        METRICS[..count]
            .iter()
            .map(|spec| spec.name.to_string())
            .collect()
    }

    fn assert_error(error: RatchetError, code: i32, reason: &str) {
        assert_eq!(error.code, code);
        assert_eq!(error.reason, reason);
    }

    fn valid_exception(metrics: &[String]) -> Value {
        let mut scope = Map::new();
        scope.insert(
            "change".to_string(),
            Value::String(VALID_CHANGE.to_string()),
        );
        scope.insert(
            "metrics".to_string(),
            Value::Array(metrics.iter().cloned().map(Value::String).collect()),
        );
        let mut object = Map::new();
        object.insert("schema_version".to_string(), Value::String("1".to_string()));
        object.insert("scope".to_string(), Value::Object(scope));
        object.insert(
            "expires_at".to_string(),
            Value::String(VALID_EXPIRY.to_string()),
        );
        object.insert(
            "reason".to_string(),
            Value::String(VALID_REASON.to_string()),
        );
        object.insert(
            "signed_by".to_string(),
            Value::String(VALID_SIGNER.to_string()),
        );
        let signature = sign_object(&Value::Object(object.clone()));
        object.insert("signature".to_string(), Value::String(signature));
        Value::Object(object)
    }

    fn write_exception(value: &Value) -> PathBuf {
        static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fleet-ratchet-test-{}-{}.json",
            std::process::id(),
            sequence
        ));
        fs::write(
            &path,
            serde_json::to_vec(value).expect("serialize exception"),
        )
        .expect("write exception fixture");
        path
    }

    fn read_fixture(
        value: &Value,
        change: &str,
    ) -> super::Result<std::collections::BTreeSet<String>> {
        let path = write_exception(value);
        let result = read_exception(&path, change);
        fs::remove_file(path).expect("remove exception fixture");
        result
    }

    #[test]
    fn exception_scope_accepts_each_valid_boundary() {
        let one_metric = metric_names(1);
        if let Err(error) = validate_exception_scope(
            VALID_CHANGE,
            &one_metric,
            "2100-01-01T00:00:00Z",
            VALID_REASON,
            VALID_SIGNER,
        ) {
            panic!(
                "valid scope rejected: code={} reason={}",
                error.code, error.reason
            );
        }
    }

    #[test]
    fn exception_scope_requires_every_top_level_field() {
        let metrics = metric_names(1);
        let cases = [
            (
                "",
                metrics.as_slice(),
                VALID_EXPIRY,
                VALID_REASON,
                VALID_SIGNER,
            ),
            (
                VALID_CHANGE,
                &[][..],
                VALID_EXPIRY,
                VALID_REASON,
                VALID_SIGNER,
            ),
            (
                VALID_CHANGE,
                metrics.as_slice(),
                "",
                VALID_REASON,
                VALID_SIGNER,
            ),
            (
                VALID_CHANGE,
                metrics.as_slice(),
                VALID_EXPIRY,
                "",
                VALID_SIGNER,
            ),
            (
                VALID_CHANGE,
                metrics.as_slice(),
                VALID_EXPIRY,
                VALID_REASON,
                "",
            ),
        ];
        for (change, metrics, expires_at, reason, signed_by) in cases {
            let error = validate_exception_scope(change, metrics, expires_at, reason, signed_by)
                .expect_err("each required field must be independently enforced");
            assert_error(
                error,
                EXIT_REFUSAL,
                "exception must be signed, expiring, and narrowing",
            );
        }

        let all_metrics = metric_names(METRICS.len());
        let error = validate_exception_scope(
            VALID_CHANGE,
            &all_metrics,
            VALID_EXPIRY,
            VALID_REASON,
            VALID_SIGNER,
        )
        .expect_err("a waiver cannot cover every metric");
        assert_error(
            error,
            EXIT_REFUSAL,
            "exception must be signed, expiring, and narrowing",
        );
    }

    #[test]
    fn exception_scope_rejects_unknown_wildcard_and_duplicate_metrics() {
        for metrics in [
            vec!["not_a_metric".to_string()],
            vec!["*".to_string()],
            vec![METRICS[0].name.to_string(), METRICS[0].name.to_string()],
        ] {
            let error = validate_exception_scope(
                VALID_CHANGE,
                &metrics,
                VALID_EXPIRY,
                VALID_REASON,
                VALID_SIGNER,
            )
            .expect_err("metric scope must contain distinct known metrics only");
            assert_error(
                error,
                EXIT_REFUSAL,
                "exception scope widens beyond known metrics",
            );
        }
    }

    #[test]
    fn exception_scope_checks_each_timestamp_constraint_independently() {
        let metrics = metric_names(1);
        for expires_at in [
            "2100-01-01T00:00:0Z",
            "2100-01-01T00:00:00X",
            "2100-01-01X00:00:00Z",
        ] {
            let error = validate_exception_scope(
                VALID_CHANGE,
                &metrics,
                expires_at,
                VALID_REASON,
                VALID_SIGNER,
            )
            .expect_err("all UTC RFC3339 shape constraints are required");
            assert_error(
                error,
                EXIT_REFUSAL,
                "exception expires_at must be UTC RFC3339",
            );
        }
    }

    #[test]
    fn read_exception_accepts_a_valid_signed_narrow_scope() {
        let metrics = metric_names(1);
        let parsed = match read_fixture(&valid_exception(&metrics), VALID_CHANGE) {
            Ok(parsed) => parsed,
            Err(error) => panic!(
                "valid signed exception rejected: code={} reason={}",
                error.code, error.reason
            ),
        };
        assert_eq!(parsed, metrics.into_iter().collect());
    }

    #[test]
    fn read_exception_rejects_schema_scope_change_expiry_and_signature_mutations() {
        let metrics = metric_names(1);

        let mut wrong_schema = valid_exception(&metrics);
        wrong_schema["schema_version"] = Value::String("2".to_string());
        assert_error(
            read_fixture(&wrong_schema, VALID_CHANGE).expect_err("schema must be version 1"),
            EXIT_MISMATCH,
            "exception schema_version must be 1",
        );

        let mut wrong_change = valid_exception(&metrics);
        wrong_change["scope"]["change"] = Value::String("another-change".to_string());
        assert_error(
            read_fixture(&wrong_change, VALID_CHANGE).expect_err("change must match"),
            EXIT_MISMATCH,
            "exception scope does not match change",
        );

        let mut expired = valid_exception(&metrics);
        expired["expires_at"] = Value::String("2000-01-01T00:00:00Z".to_string());
        let error = read_fixture(&expired, VALID_CHANGE).expect_err("past exception must expire");
        assert_eq!(error.code, super::EXIT_INVARIANT);
        assert!(error.reason.starts_with("expired exception expires_at="));

        let mut bad_signature = valid_exception(&metrics);
        bad_signature["signature"] = Value::String("blake3:invalid".to_string());
        assert_error(
            read_fixture(&bad_signature, VALID_CHANGE).expect_err("signature must verify"),
            EXIT_MISMATCH,
            "exception signature does not verify",
        );
    }

    #[test]
    fn read_exception_rejects_each_out_of_scope_shape() {
        let metrics = metric_names(1);

        let mut short_scope = valid_exception(&metrics);
        short_scope["scope"]
            .as_object_mut()
            .expect("scope object")
            .remove("change");
        assert_error(
            read_fixture(&short_scope, VALID_CHANGE).expect_err("scope needs exactly two fields"),
            EXIT_MISMATCH,
            "exception contains fields outside its signed scope",
        );

        let mut extra_field = valid_exception(&metrics);
        extra_field["unsigned_note"] = Value::String("outside signature".to_string());
        assert_error(
            read_fixture(&extra_field, VALID_CHANGE).expect_err("extra fields are forbidden"),
            EXIT_MISMATCH,
            "exception contains fields outside its signed scope",
        );
    }

    #[test]
    fn read_exception_rejects_metric_scope_before_generic_validation() {
        for metrics in [
            vec!["not_a_metric".to_string()],
            vec!["*".to_string()],
            vec![METRICS[0].name.to_string(), METRICS[0].name.to_string()],
        ] {
            assert_error(
                read_fixture(&valid_exception(&metrics), VALID_CHANGE)
                    .expect_err("invalid parsed metric scope must be a mismatch"),
                EXIT_MISMATCH,
                "exception scope widens beyond known metrics",
            );
        }
    }

    #[test]
    fn read_exception_requires_a_nonempty_proper_subset() {
        for metrics in [metric_names(0), metric_names(METRICS.len())] {
            assert_error(
                read_fixture(&valid_exception(&metrics), VALID_CHANGE)
                    .expect_err("parsed metric scope must narrow the full set"),
                EXIT_MISMATCH,
                "exception scope must narrow the metric set",
            );
        }
    }

    // ---- from lane k-C ----

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let output = Command::new("mktemp")
                .arg("-d")
                .output()
                .expect("mktemp must be available");
            assert!(output.status.success(), "mktemp -d failed");
            let path = String::from_utf8(output.stdout).expect("mktemp path must be UTF-8");
            Self(PathBuf::from(path.trim()))
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("temporary ratchet state must be removable");
        }
    }

    fn run_adequacy(killed: u64, total: u64) -> String {
        let state = TempDir::new();
        let output = Command::new(std::env::current_exe().expect("current test executable"))
            .arg("--exact")
            .arg("ratchet::tests::adequacy_subprocess_helper")
            .arg("--ignored")
            .arg("--nocapture")
            .env("FLEET_STATE", state.path())
            .env("RATCHET_TEST_KILLED", killed.to_string())
            .env("RATCHET_TEST_TOTAL", total.to_string())
            .output()
            .expect("adequacy subprocess must run");
        assert!(output.status.success(), "subprocess harness failed");
        String::from_utf8(output.stdout).expect("adequacy output must be UTF-8")
    }

    fn test_marks() -> BTreeMap<String, Mark> {
        METRICS
            .iter()
            .map(|spec| {
                let value = match spec.name {
                    "mutation_kill_rate" => RATE_SCALE / 2,
                    "mutant_count" => 10,
                    _ => 0,
                };
                (
                    spec.name.to_string(),
                    Mark {
                        value: MetricValue {
                            kind: match spec.kind {
                                MetricKind::Rate => MetricKind::Rate,
                                MetricKind::Count => MetricKind::Count,
                            },
                            value,
                        },
                        set_at: "2026-01-01T00:00:00Z".to_string(),
                        set_by_change: "ratchet-test".to_string(),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn decision_args_parse_all_options_and_advance_by_pairs() {
        let args = vec![
            "--metrics".to_string(),
            "scorecard.json".to_string(),
            "--change".to_string(),
            "change-42".to_string(),
            "--exception".to_string(),
            "waiver.json".to_string(),
        ];

        let parsed = parse_decision_args(&args)
            .unwrap_or_else(|error| panic!("unexpected parse error: {}", error.reason));
        assert_eq!(parsed.metrics_path, PathBuf::from("scorecard.json"));
        assert_eq!(parsed.change, "change-42");
        assert_eq!(parsed.exception, Some(PathBuf::from("waiver.json")));
    }

    #[test]
    fn dispatch_ratchet_refuses_a_score_below_the_mark_and_names_both() {
        let state = TempDir::new();
        record_dispatch_inner(state.path(), "first-artifact", 9, 10)
            .unwrap_or_else(|error| panic!("first dispatch must seed its mark: {}", error.reason));
        let error = record_dispatch_inner(state.path(), "later-artifact", 8, 10)
            .expect_err("later dispatch below the rising bar must refuse");
        assert_eq!(error.code, EXIT_REFUSAL);
        assert!(error.reason.contains("actual=0.8"), "{}", error.reason);
        assert!(error.reason.contains("mark=0.9"), "{}", error.reason);
    }

    #[test]
    fn adequacy_parses_both_options_and_prints_fractional_rate() {
        let output = run_adequacy(9, 10);
        assert!(
            output.contains("rate=0.9000 interval="),
            "unexpected output: {output}"
        );
        assert!(output.contains("count=10 verdict=pass"), "{output}");
        assert!(output.contains("ADEQUACY_RESULT=ok"), "{output}");
    }

    #[test]
    fn adequacy_accepts_killed_equal_to_total() {
        let output = run_adequacy(10, 10);
        assert!(output.contains("rate=1.0000 interval="), "{output}");
        assert!(output.contains("count=10 verdict=pass"), "{output}");
        assert!(output.contains("ADEQUACY_RESULT=ok"), "{output}");
    }

    #[test]
    fn adequacy_rejects_zero_total_as_invariant() {
        let output = run_adequacy(0, 0);
        assert!(
            output.contains("rate=unavailable interval=unavailable"),
            "{output}"
        );
        assert!(output.contains("ADEQUACY_RESULT=err:6"), "{output}");
    }

    #[test]
    fn adequacy_rejects_a_valid_but_regressed_measurement() {
        let output = run_adequacy(4, 10);
        assert!(output.contains("count=10 verdict=fail"), "{output}");
        assert!(output.contains("ADEQUACY_RESULT=err:7"), "{output}");
    }

    #[test]
    #[ignore = "subprocess entry point used by adequacy tests"]
    fn adequacy_subprocess_helper() {
        let state = PathBuf::from(std::env::var("FLEET_STATE").expect("FLEET_STATE"));
        let ratchet = state.join("ratchet");
        fs::create_dir_all(&ratchet).expect("ratchet state directory");
        write_marks(&ratchet.join("marks.json"), &test_marks())
            .unwrap_or_else(|error| panic!("unable to write test marks: {}", error.reason));
        let args = vec![
            "--killed".to_string(),
            std::env::var("RATCHET_TEST_KILLED").expect("killed value"),
            "--total".to_string(),
            std::env::var("RATCHET_TEST_TOTAL").expect("total value"),
        ];
        match adequacy_command(&args) {
            Ok(()) => println!("ADEQUACY_RESULT=ok"),
            Err(error) => println!("ADEQUACY_RESULT=err:{}", error.code),
        }
    }
}
