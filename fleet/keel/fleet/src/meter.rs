//! Exact, persistent token accounting and measured-basis scheduling.
//!
//! The state file intentionally has a tiny, inspectable format.  Empty numeric
//! fields mean unknown; they are never decoded as zero.

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const EXIT_ENVIRONMENT: i32 = 3;
const EXIT_INVARIANT: i32 = 6;
const STATE_FILE: &str = "meter-v1.tsv";

#[derive(Clone, Debug, PartialEq, Eq)]
struct Lane {
    window: Option<u64>,
    used: Option<u64>,
    reservations: Vec<u64>,
    resolved_model: Option<String>,
    unknown_observed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Meter {
    tokenizer_generation: Option<String>,
    lanes: BTreeMap<String, Lane>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlanningSnapshot {
    pub(crate) estimated_task_tokens: Option<u64>,
    pub(crate) checked: usize,
    pub(crate) total: usize,
    pub(crate) remaining: BTreeMap<String, Option<u64>>,
}

impl Meter {
    fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let mut lines = text.lines();
        let header = lines
            .next()
            .ok_or_else(|| "empty meter state".to_string())?;
        let generation = header
            .strip_prefix("fleet-meter-v1\ttokenizer=")
            .ok_or_else(|| "unsupported meter state generation".to_string())?;
        let tokenizer_generation = if generation.is_empty() {
            None
        } else {
            Some(generation.to_owned())
        };
        let mut lanes = BTreeMap::new();
        for (index, line) in lines.enumerate() {
            let fields: Vec<_> = line.split('\t').collect();
            if !matches!(fields.len(), 4 | 6) || fields[0].is_empty() {
                return Err(format!("invalid meter state line {}", index + 2));
            }
            let parse_optional = |raw: &str| -> Result<Option<u64>, String> {
                if raw.is_empty() {
                    Ok(None)
                } else {
                    raw.parse()
                        .map(Some)
                        .map_err(|_| format!("invalid count on line {}", index + 2))
                }
            };
            let reservations = if fields[3].is_empty() {
                Vec::new()
            } else {
                fields[3]
                    .split(',')
                    .map(|v| {
                        v.parse::<u64>()
                            .map_err(|_| format!("invalid reservation on line {}", index + 2))
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };
            lanes.insert(
                fields[0].to_owned(),
                Lane {
                    window: parse_optional(fields[1])?,
                    used: parse_optional(fields[2])?,
                    reservations,
                    resolved_model: fields
                        .get(4)
                        .filter(|value| !value.is_empty())
                        .map(|value| (*value).to_owned()),
                    unknown_observed: fields.get(5).is_some_and(|value| *value == "unknown"),
                },
            );
        }
        Ok(Self {
            tokenizer_generation,
            lanes,
        })
    }

    fn save(&self, path: &Path) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| "meter state has no parent".to_string())?;
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
        let mut body = format!(
            "fleet-meter-v1\ttokenizer={}\n",
            self.tokenizer_generation.as_deref().unwrap_or("")
        );
        for (name, lane) in &self.lanes {
            let window = lane.window.map(|v| v.to_string()).unwrap_or_default();
            let used = lane.used.map(|v| v.to_string()).unwrap_or_default();
            let reservations = lane
                .reservations
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            body.push_str(&format!(
                "{name}\t{window}\t{used}\t{reservations}\t{}\t{}\n",
                lane.resolved_model.as_deref().unwrap_or(""),
                if lane.unknown_observed { "unknown" } else { "" }
            ));
        }
        let pid = std::process::id();
        let tmp = parent.join(format!(".meter-v1.{pid}.tmp"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| format!("create {}: {e}", tmp.display()))?;
        file.write_all(body.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        fs::rename(&tmp, path).map_err(|e| format!("publish {}: {e}", path.display()))
    }

    fn reserve(&mut self, lane_name: &str, tokens: u64) -> Result<(), String> {
        let lane = self
            .lanes
            .get_mut(lane_name)
            .ok_or_else(|| format!("unknown lane {lane_name}"))?;
        let window = lane
            .window
            .ok_or_else(|| format!("lane {lane_name} window is unknown"))?;
        let used = lane
            .used
            .ok_or_else(|| format!("lane {lane_name} usage is unknown"))?;
        let remaining = window
            .checked_sub(used)
            .ok_or_else(|| format!("lane {lane_name} usage exceeds its window"))?;
        if tokens > remaining {
            return Err(format!(
                "reservation {tokens} exceeds remaining {remaining} on lane {lane_name}"
            ));
        }
        lane.used = used.checked_add(tokens);
        lane.reservations.push(tokens);
        Ok(())
    }

    fn measured_lanes(&self) -> usize {
        self.lanes
            .values()
            .filter(|lane| lane.window.is_some() && lane.used.is_some())
            .count()
    }

    fn observed_lanes(&self) -> usize {
        self.lanes
            .values()
            .filter(|lane| lane.used.is_some())
            .count()
    }

    fn measured_task_tokens(&self) -> Option<u64> {
        let values: Vec<u64> = self
            .lanes
            .values()
            .flat_map(|lane| lane.reservations.iter().copied())
            .collect();
        if values.is_empty() {
            return None;
        }
        let sum = values
            .iter()
            .try_fold(0_u64, |acc, value| acc.checked_add(*value))?;
        let count = u64::try_from(values.len()).ok()?;
        // Ceiling keeps the plan inside the measured capacity.
        sum.checked_add(count.checked_sub(1)?)?.checked_div(count)
    }
}

fn state_path() -> Result<PathBuf, i32> {
    let root = env::var_os("FLEET_STATE")
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            eprintln!("fleet meter: FLEET_STATE is required");
            EXIT_ENVIRONMENT
        })?;
    Ok(PathBuf::from(root).join(STATE_FILE))
}

fn tokenizer_generation() -> Option<String> {
    if let Some(value) = env::var_os("CODEX_TOKENIZER_GENERATION").filter(|v| !v.is_empty()) {
        return Some(value.to_string_lossy().into_owned());
    }
    let output = Command::new("codex").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn initial_meter() -> Result<Meter, String> {
    let specification = env::var("FLEET_METER_WINDOWS")
        .map_err(|_| concat!("no persisted lanes and FLEET_METER_WINDOWS is unset.", "\n", "  The meter reports MEASURED usage per lane and will not invent a window it was never given.", "\n", "  Either declare the windows, or run a task and it records its own:", "\n", "", "\n", "    export FLEET_METER_WINDOWS=\"codex=200000,claude=100000\"", "\n", "    fleet swarm dispatch --task \"<T>\" --repo <P>", "\n", "", "\n", "  A lane whose window is genuinely unknown reports null, never 0.").to_string())?;
    let mut lanes = BTreeMap::new();
    for item in specification.split(',').filter(|item| !item.is_empty()) {
        let (name, raw_window) = item
            .split_once('=')
            .ok_or_else(|| format!("invalid lane window {item}"))?;
        if name.is_empty() || name.contains(['\t', '\n']) {
            return Err(format!("invalid lane name {name:?}"));
        }
        let window = if raw_window == "null" {
            None
        } else {
            Some(
                raw_window
                    .parse::<u64>()
                    .map_err(|_| format!("invalid window {raw_window}"))?,
            )
        };
        lanes.insert(
            name.to_owned(),
            Lane {
                window,
                used: window.map(|_| 0),
                reservations: Vec::new(),
                resolved_model: None,
                unknown_observed: false,
            },
        );
    }
    Ok(Meter {
        tokenizer_generation: tokenizer_generation(),
        lanes,
    })
}

fn load_or_initialize(path: &Path) -> Result<Meter, String> {
    if path.exists() {
        Meter::load(path)
    } else {
        initial_meter()
    }
}

/// Read the same measured state used by `fleet meter` without reserving tokens
/// or persisting an initialized meter. Planning must have no side effects.
pub(crate) fn planning_snapshot_at(state_root: &Path) -> Result<PlanningSnapshot, i32> {
    let path = state_root.join(STATE_FILE);
    let meter = if path.exists() {
        Meter::load(&path).map_err(|reason| {
            eprintln!("fleet meter: invalid persisted planning state: {reason}");
            EXIT_INVARIANT
        })?
    } else {
        initial_meter().map_err(|reason| {
            eprintln!("fleet meter: {reason}");
            EXIT_ENVIRONMENT
        })?
    };
    let remaining = meter
        .lanes
        .iter()
        .map(|(name, lane)| {
            let value = lane
                .window
                .zip(lane.used)
                .and_then(|(window, used)| window.checked_sub(used));
            (name.clone(), value)
        })
        .collect();
    Ok(PlanningSnapshot {
        estimated_task_tokens: meter.measured_task_tokens(),
        checked: meter.measured_lanes(),
        total: meter.lanes.len(),
        remaining,
    })
}

fn null_or(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn show(meter: &Meter) -> Result<(), i32> {
    // `show` reports observations, not quota configuration. A completed run with unavailable
    // token metadata is still an observed input and is printed as null with checked=0. It must
    // not be turned into a fabricated zero or hidden behind a refusal.
    let checked = meter.observed_lanes();
    let total = meter.lanes.len();
    println!("LANE\tUSED\tREMAINING\tCHECKED\tTOTAL\tTOKENIZER_GENERATION\tRESOLVED_MODEL");
    for (name, lane) in &meter.lanes {
        let remaining = lane
            .window
            .zip(lane.used)
            .and_then(|(window, used)| window.checked_sub(used));
        println!(
            "{name}\t{}\t{}\t{checked}\t{total}\t{}\t{}",
            null_or(lane.used),
            null_or(remaining),
            meter.tokenizer_generation.as_deref().unwrap_or("null"),
            lane.resolved_model.as_deref().unwrap_or("null")
        );
    }
    if total == 0 {
        eprintln!("fleet meter REFUSE: observed zero lanes (checked=0,total=0)");
        Err(EXIT_INVARIANT)
    } else {
        Ok(())
    }
}

/// Persist usage emitted by a real worker invocation. Unknown metadata remains absent; in
/// particular, a completed stub run records `used=null`, never zero. Once an unknown observation
/// exists for a lane, later values cannot make the aggregate exact, so the aggregate stays null.
pub(crate) fn record_observation_at(
    state_root: &Path,
    lane_name: &str,
    tokens: Option<u64>,
    resolved_model: Option<&str>,
) -> Result<(), i32> {
    if lane_name.is_empty() || lane_name.contains(['\t', '\n']) {
        eprintln!("fleet meter: invalid lane name {lane_name:?}");
        return Err(EXIT_INVARIANT);
    }
    let path = state_root.join(STATE_FILE);
    let mut meter = if path.exists() {
        Meter::load(&path).map_err(|reason| {
            eprintln!("fleet meter: {reason}");
            EXIT_INVARIANT
        })?
    } else {
        match initial_meter() {
            Ok(meter) => meter,
            Err(_) => Meter {
                // A completed run does not prove which tokenizer was used. Adapter metadata may
                // populate this in a future schema; until then absence is evidence, not zero or
                // the version of an unrelated CLI found on PATH.
                tokenizer_generation: None,
                lanes: BTreeMap::new(),
            },
        }
    };
    match meter.lanes.get_mut(lane_name) {
        Some(lane) => {
            if !lane.unknown_observed {
                lane.used = match (lane.used, tokens) {
                    (Some(used), Some(tokens)) => used.checked_add(tokens),
                    // `None` before any observation means an unknown configured window, not an
                    // unmeasured run. The first measured run establishes the used value.
                    (None, Some(tokens)) => Some(tokens),
                    (_, None) => {
                        lane.unknown_observed = true;
                        None
                    }
                };
            }
            if let Some(model) = resolved_model {
                lane.resolved_model = Some(model.to_owned());
            }
        }
        None => {
            meter.lanes.insert(
                lane_name.to_owned(),
                Lane {
                    window: None,
                    used: tokens,
                    reservations: Vec::new(),
                    resolved_model: resolved_model.map(str::to_owned),
                    unknown_observed: tokens.is_none(),
                },
            );
        }
    }
    meter.save(&path).map_err(|reason| {
        eprintln!("fleet meter: {reason}");
        EXIT_ENVIRONMENT
    })
}

fn parse_positive(raw: Option<&String>, label: &str) -> Result<u64, i32> {
    let value = raw
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| {
            eprintln!("fleet meter REFUSE: {label} must be a positive integer");
            EXIT_INVARIANT
        })?;
    Ok(value)
}

fn option_value<'a>(args: &'a [String], name: &str) -> Option<&'a String> {
    args.iter()
        .position(|v| v == name)
        .and_then(|index| args.get(index + 1))
}

fn plan(meter: &Meter, tasks: u64) -> Result<(), i32> {
    let checked = meter.measured_lanes();
    let total = meter.lanes.len();
    let per_task = meter.measured_task_tokens();
    println!(
        "requested={tasks} measured_task_tokens={} checked={checked} total={total}",
        null_or(per_task)
    );
    let per_task = per_task.ok_or_else(|| {
        eprintln!("fleet meter REFUSE: task size is unmeasured; make an exact reservation first");
        EXIT_INVARIANT
    })?;
    if checked == 0 {
        return Err(EXIT_INVARIANT);
    }
    let mut capacity: BTreeMap<String, u64> = meter
        .lanes
        .iter()
        .filter_map(|(name, lane)| {
            lane.window
                .zip(lane.used)
                .and_then(|(window, used)| window.checked_sub(used))
                .map(|remaining| (name.clone(), remaining / per_task))
        })
        .collect();
    let mut assignments = Vec::new();
    for task in 1..=tasks {
        let lane = if capacity.get("codex").copied().unwrap_or(0) > 0 {
            Some("codex".to_string())
        } else {
            capacity
                .iter()
                .filter(|(_, slots)| **slots > 0)
                .max_by_key(|(name, slots)| (**slots, std::cmp::Reverse((*name).clone())))
                .map(|(name, _)| name.clone())
        };
        let Some(lane) = lane else { break };
        if let Some(slots) = capacity.get_mut(&lane) {
            *slots -= 1;
        }
        assignments.push((task, lane));
    }
    println!(
        "fit={} requested={tasks} checked={checked} total={total}",
        assignments.len()
    );
    for (task, lane) in assignments {
        println!("task={task}\tlane={lane}");
    }
    Ok(())
}

fn command_inner(args: &[String], path: &Path) -> Result<(), i32> {
    let mut meter = load_or_initialize(path).map_err(|reason| {
        eprintln!("fleet meter REFUSE: {reason}");
        EXIT_INVARIANT
    })?;
    match args.first().map(String::as_str) {
        Some("show") if args.len() == 1 => show(&meter),
        Some("reserve") => {
            let tokens = parse_positive(option_value(args, "--tokens"), "--tokens")?;
            let lane = option_value(args, "--lane")
                .filter(|v| !v.is_empty())
                .ok_or_else(|| {
                    eprintln!("fleet meter REFUSE: --lane is required");
                    EXIT_INVARIANT
                })?;
            meter.reserve(lane, tokens).map_err(|reason| {
                eprintln!("fleet meter REFUSE: {reason}");
                EXIT_INVARIANT
            })?;
            meter.save(path).map_err(|reason| {
                eprintln!("fleet meter: {reason}");
                EXIT_ENVIRONMENT
            })?;
            let measured = meter.measured_lanes();
            println!(
                "accepted tokens={tokens} lane={lane} checked={measured} total={}",
                meter.lanes.len()
            );
            Ok(())
        }
        Some("plan") => plan(
            &meter,
            parse_positive(option_value(args, "--tasks"), "--tasks")?,
        ),
        _ => {
            eprintln!("fleet meter <show|reserve --tokens N --lane L|plan --tasks N>");
            Err(EXIT_INVARIANT)
        }
    }
}

fn refusal_receipt(state_path: &Path) -> Result<(), String> {
    let state_root = state_path
        .parent()
        .ok_or_else(|| "meter state has no parent".to_string())?;
    let receipt_dir = state_root.join("receipts");
    fs::create_dir_all(&receipt_dir)
        .map_err(|e| format!("create {}: {e}", receipt_dir.display()))?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("system clock: {e}"))?
        .as_millis();
    let receipt = receipt_dir.join(format!(
        "meter-refusal-{}-{timestamp}.json",
        std::process::id()
    ));
    let body = format!("{{\"kind\":\"meter_refusal\",\"checked\":null,\"total\":null,\"timestamp_ms\":{timestamp}}}\n");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&receipt)
        .map_err(|e| format!("create {}: {e}", receipt.display()))?;
    file.write_all(body.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("write {}: {e}", receipt.display()))
}

pub fn command(args: &[String]) -> Result<(), i32> {
    let path = state_path()?;
    match command_inner(args, &path) {
        Err(EXIT_INVARIANT) => {
            refusal_receipt(&path).map_err(|reason| {
                eprintln!("fleet meter: could not persist refusal receipt: {reason}");
                EXIT_ENVIRONMENT
            })?;
            Err(EXIT_INVARIANT)
        }
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn meter(window: Option<u64>, used: Option<u64>) -> Meter {
        Meter {
            tokenizer_generation: Some("codex-test-generation".into()),
            lanes: BTreeMap::from([(
                "codex".into(),
                Lane {
                    window,
                    used,
                    reservations: vec![],
                    resolved_model: None,
                    unknown_observed: false,
                },
            )]),
        }
    }

    #[test]
    fn over_reservation_is_refused() {
        assert!(meter(Some(100), Some(90)).reserve("codex", 11).is_err());
    }

    #[test]
    fn fitting_reservation_is_accepted() {
        let mut value = meter(Some(100), Some(90));
        assert_eq!(value.reserve("codex", 10), Ok(()));
        assert_eq!(value.lanes["codex"].used, Some(100));
    }

    #[test]
    fn persistence_across_two_invocations() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            env::temp_dir().join(format!("fleet-meter-{}-{unique}", std::process::id()));
        let path = directory.join(STATE_FILE);
        let mut first = meter(Some(100), Some(0));
        first.reserve("codex", 37).unwrap();
        first.save(&path).unwrap();
        let mut second = Meter::load(&path).unwrap();
        second.reserve("codex", 5).unwrap();
        second.save(&path).unwrap();
        assert_eq!(Meter::load(&path).unwrap().lanes["codex"].used, Some(42));
        fs::remove_file(path).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn unknown_serializes_as_null_not_zero() {
        assert_eq!(null_or(None), "null");
        let value = meter(None, None);
        assert_eq!(value.lanes["codex"].used, None);
    }

    #[test]
    fn unknown_observation_is_persisted_as_absent_with_a_denominator() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "fleet-meter-observation-{}-{unique}",
            std::process::id()
        ));
        record_observation_at(&directory, "stub", None, None).unwrap();
        let persisted = Meter::load(&directory.join(STATE_FILE)).unwrap();
        assert_eq!(persisted.lanes.len(), 1);
        assert_eq!(persisted.lanes["stub"].used, None);
        fs::remove_file(directory.join(STATE_FILE)).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn measured_observation_populates_unknown_configured_lane_and_model() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "fleet-meter-measured-observation-{}-{unique}",
            std::process::id()
        ));
        let path = directory.join(STATE_FILE);
        let configured = Meter {
            tokenizer_generation: None,
            lanes: BTreeMap::from([(
                "freelane".to_string(),
                Lane {
                    window: None,
                    used: None,
                    reservations: Vec::new(),
                    resolved_model: None,
                    unknown_observed: false,
                },
            )]),
        };
        configured.save(&path).unwrap();

        record_observation_at(&directory, "freelane", Some(35), Some("served-model")).unwrap();

        let persisted = Meter::load(&path).unwrap();
        assert_eq!(persisted.lanes["freelane"].used, Some(35));
        assert_eq!(
            persisted.lanes["freelane"].resolved_model.as_deref(),
            Some("served-model")
        );
        assert_eq!(persisted.observed_lanes(), 1);
        fs::remove_file(path).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn unknown_observation_cannot_later_be_rewritten_as_measured_zero() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "fleet-meter-unknown-aggregate-{}-{unique}",
            std::process::id()
        ));
        record_observation_at(&directory, "freelane", None, Some("first-model")).unwrap();
        record_observation_at(&directory, "freelane", Some(0), Some("second-model")).unwrap();

        let persisted = Meter::load(&directory.join(STATE_FILE)).unwrap();
        assert_eq!(persisted.lanes["freelane"].used, None);
        assert!(persisted.lanes["freelane"].unknown_observed);
        fs::remove_file(directory.join(STATE_FILE)).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
