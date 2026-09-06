use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
pub const FIELDS: [&str; 10] = [
    "ts",
    "event",
    "component",
    "task_id",
    "generator_model",
    "verifier_model",
    "exit_code",
    "tokens_in",
    "tokens_out",
    "cost_micro_usd",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Receipt {
    pub ts: String,
    pub event: String,
    pub component: String,
    pub task_id: String,
    pub generator_model: String,
    pub verifier_model: String,
    pub exit_code: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cost_micro_usd: i64,
    pub prev_hash: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    pub lines: usize,
    pub verified_from: Option<usize>,
}

#[derive(Debug)]
pub enum LedgerError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidInteger(String),
    ChainBroken { line: usize, reason: String },
    Invariant { code: u8, message: String },
    Usage(String),
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Json(err) => write!(f, "{err}"),
            Self::InvalidInteger(value) => write!(f, "'{value}' is not an integer"),
            Self::ChainBroken { line, reason } => {
                write!(f, "chain: BROKEN at line {line} - {reason}")
            }
            Self::Invariant { message, .. } => write!(f, "{message}"),
            Self::Usage(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<std::io::Error> for LedgerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for LedgerError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub fn default_ledger_path() -> PathBuf {
    if let Ok(path) = std::env::var("FLEET_LEDGER") {
        return PathBuf::from(path);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("ledger/RECEIPTS.jsonl")
}

pub fn hash_for(prev_hash: &str, canonical: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(b"\n");
    hasher.update(canonical.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn canonical_for_receipt(receipt: &Receipt) -> String {
    [
        receipt.ts.as_str(),
        receipt.event.as_str(),
        receipt.component.as_str(),
        receipt.task_id.as_str(),
        receipt.generator_model.as_str(),
        receipt.verifier_model.as_str(),
        &receipt.exit_code.to_string(),
        &receipt.tokens_in.to_string(),
        &receipt.tokens_out.to_string(),
        &receipt.cost_micro_usd.to_string(),
    ]
    .join("|")
}

fn json_scalar_as_shell_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::Null => "None".to_owned(),
        other => other.to_string(),
    }
}

pub fn canonical_for_json(value: &Value) -> String {
    FIELDS
        .iter()
        .map(|field| {
            value
                .get(*field)
                .map_or_else(String::new, json_scalar_as_shell_string)
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn parse_integer(value: &str) -> Result<i64, LedgerError> {
    if value.is_empty()
        || (value.starts_with('-') && value.len() == 1)
        || (!value.starts_with('-') && !value.chars().all(|c| c.is_ascii_digit()))
        || (value.starts_with('-') && !value[1..].chars().all(|c| c.is_ascii_digit()))
    {
        return Err(LedgerError::InvalidInteger(value.to_owned()));
    }
    value
        .parse::<i64>()
        .map_err(|_| LedgerError::InvalidInteger(value.to_owned()))
}

fn lock_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.lock", path.display()))
}

struct LedgerLock {
    path: PathBuf,
}

impl LedgerLock {
    fn acquire(path: &Path) -> Result<Self, LedgerError> {
        let lock = lock_path(path);
        fs::create_dir(&lock)?;
        let pid_path = lock.join("pid");
        let mut pid = File::create(pid_path)?;
        writeln!(pid, "{}", std::process::id())?;
        Ok(Self { path: lock })
    }
}

impl Drop for LedgerLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn append_receipt(path: &Path, input: ReceiptInput<'_>) -> Result<String, LedgerError> {
    for value in [
        input.exit_code,
        input.tokens_in,
        input.tokens_out,
        input.cost_micro_usd,
    ] {
        parse_integer(value)?;
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let _lock = LedgerLock::acquire(path)?;
    let prev_hash = last_hash(path)?;
    let receipt = Receipt {
        ts: input.ts.to_owned(),
        event: input.event.to_owned(),
        component: input.component.to_owned(),
        task_id: input.task_id.to_owned(),
        generator_model: input.generator_model.to_owned(),
        verifier_model: input.verifier_model.to_owned(),
        exit_code: parse_integer(input.exit_code)?,
        tokens_in: parse_integer(input.tokens_in)?,
        tokens_out: parse_integer(input.tokens_out)?,
        cost_micro_usd: parse_integer(input.cost_micro_usd)?,
        prev_hash,
        hash: String::new(),
    };
    let mut receipt = receipt;
    receipt.hash = hash_for(&receipt.prev_hash, &canonical_for_receipt(&receipt));
    let encoded = serde_json::to_string(&receipt)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{encoded}")?;
    Ok(receipt.hash)
}

#[derive(Debug, Clone, Copy)]
pub struct ReceiptInput<'a> {
    pub ts: &'a str,
    pub event: &'a str,
    pub component: &'a str,
    pub task_id: &'a str,
    pub generator_model: &'a str,
    pub verifier_model: &'a str,
    pub exit_code: &'a str,
    pub tokens_in: &'a str,
    pub tokens_out: &'a str,
    pub cost_micro_usd: &'a str,
}

fn last_hash(path: &Path) -> Result<String, LedgerError> {
    if !path.is_file() {
        return Ok(GENESIS.to_owned());
    }
    let file = File::open(path)?;
    let mut last = String::new();
    for line in BufReader::new(file).lines() {
        last = line?;
    }
    if last.is_empty() {
        return Ok(GENESIS.to_owned());
    }
    Ok(serde_json::from_str::<Value>(&last)
        .ok()
        .and_then(|value| {
            value
                .get("hash")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .filter(|hash| !hash.is_empty())
        .unwrap_or_else(|| GENESIS.to_owned()))
}

#[derive(Debug, Deserialize)]
struct Checkpoint {
    line: usize,
    hash: String,
}

fn checkpoint_start(path: &Path, checkpoint_path: &Path) -> Result<(usize, String), LedgerError> {
    let checkpoint = match fs::read_to_string(checkpoint_path) {
        Ok(text) => serde_json::from_str::<Checkpoint>(&text),
        Err(_) => return Ok((0, GENESIS.to_owned())),
    };
    let checkpoint = match checkpoint {
        Ok(value) => value,
        Err(_) => return Ok((0, GENESIS.to_owned())),
    };
    let file = File::open(path)?;
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line_number = index + 1;
        if line_number == checkpoint.line {
            let value: Value = match serde_json::from_str(&line?) {
                Ok(value) => value,
                Err(_) => return Ok((0, GENESIS.to_owned())),
            };
            if value.get("hash").and_then(Value::as_str) == Some(checkpoint.hash.as_str()) {
                return Ok((checkpoint.line, checkpoint.hash));
            }
            return Ok((0, GENESIS.to_owned()));
        }
    }
    Ok((0, GENESIS.to_owned()))
}

pub fn verify_chain(
    path: &Path,
    incremental: bool,
    checkpoint_path: Option<&Path>,
) -> Result<VerifyReport, LedgerError> {
    if !path.is_file() || fs::metadata(path)?.len() == 0 {
        return Ok(VerifyReport {
            lines: 0,
            verified_from: None,
        });
    }
    let selected_checkpoint = checkpoint_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(format!("{}.ckpt", path.display())));
    let (start_line, mut previous) = if incremental {
        checkpoint_start(path, &selected_checkpoint)?
    } else {
        (0, GENESIS.to_owned())
    };
    let file = File::open(path)?;
    let mut line_count = 0;
    for line_result in BufReader::new(file).lines() {
        line_count += 1;
        if line_count <= start_line {
            continue;
        }
        let line = line_result?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(trimmed).map_err(|_| LedgerError::ChainBroken {
            line: line_count,
            reason: "is not valid JSON".to_owned(),
        })?;
        let found_previous = value.get("prev_hash").and_then(Value::as_str).unwrap_or("");
        if found_previous != previous {
            return Err(LedgerError::ChainBroken {
                line: line_count,
                reason: format!("prev_hash mismatch (expected {previous}, found {found_previous})"),
            });
        }
        let calculated = hash_for(&previous, &canonical_for_json(&value));
        let found_hash = value.get("hash").and_then(Value::as_str).unwrap_or("");
        if calculated != found_hash {
            return Err(LedgerError::ChainBroken {
                line: line_count,
                reason: "content edited (hash mismatch)".to_owned(),
            });
        }
        previous = found_hash.to_owned();
    }
    if checkpoint_path.is_some() || incremental {
        let temp_path = PathBuf::from(format!("{}.tmp", selected_checkpoint.display()));
        let checkpoint = serde_json::json!({"line": line_count, "hash": previous});
        fs::write(&temp_path, serde_json::to_string(&checkpoint)?)?;
        fs::rename(temp_path, selected_checkpoint)?;
    }
    Ok(VerifyReport {
        lines: line_count,
        verified_from: (start_line > 0).then_some(start_line),
    })
}

pub fn i1_looks_like_model(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.len() >= 2 && lower.starts_with('o') && lower.as_bytes()[1].is_ascii_digit() {
        return true;
    }
    [
        "claude",
        "opus",
        "sonnet",
        "haiku",
        "fable",
        "codex",
        "gpt",
        "gemini",
        "grok",
        "llama",
        "qwen",
        "deepseek",
        "mistral",
        "kimi",
        "pi",
        "pi-signed",
        "opencode",
        "cursor",
        "rovodev",
        "copilot",
        "antigravity",
        "acp",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn i1_is_sentinel(value: &str) -> bool {
    matches!(value, "human" | "none" | "not-applicable")
}

pub fn invariant_i1(generator: &str, verifier: &str) -> Result<(), LedgerError> {
    if generator.is_empty() || verifier.is_empty() {
        return Err(LedgerError::Invariant {
            code: 6,
            message: "I1: both fields must be set (use 'not-applicable' when no model is involved)"
                .to_owned(),
        });
    }
    if i1_is_sentinel(generator) && i1_is_sentinel(verifier) {
        return Ok(());
    }
    if i1_looks_like_model(generator) && !i1_looks_like_model(verifier) && !i1_is_sentinel(verifier)
    {
        return Err(LedgerError::Invariant { code: 6, message: format!("I1 VIOLATED: verifier '{verifier}' is not a model or a sentinel — '{generator}' is unverified") });
    }
    if i1_looks_like_model(verifier)
        && !i1_looks_like_model(generator)
        && !i1_is_sentinel(generator)
    {
        return Err(LedgerError::Invariant {
            code: 6,
            message: format!("I1 VIOLATED: generator '{generator}' is not a model or a sentinel"),
        });
    }
    if !i1_looks_like_model(generator) && !i1_looks_like_model(verifier) {
        return Err(LedgerError::Invariant { code: 6, message: format!("I1 VIOLATED: neither '{generator}' nor '{verifier}' is a model; use 'not-applicable' for both when no model is involved") });
    }
    if generator == verifier {
        return Err(LedgerError::Invariant {
            code: 6,
            message: format!("I1 VIOLATED: generator == verifier ({generator})"),
        });
    }
    Ok(())
}

pub fn invariant_i2(projected: &str, remaining: &str) -> Result<(), LedgerError> {
    let projected = parse_integer(projected).map_err(|_| LedgerError::Invariant {
        code: 6,
        message: "I2: non-integer input".to_owned(),
    })?;
    let remaining = parse_integer(remaining).map_err(|_| LedgerError::Invariant {
        code: 6,
        message: "I2: non-integer input".to_owned(),
    })?;
    if projected > remaining {
        return Err(LedgerError::Invariant {
            code: 5,
            message: format!("I2 VIOLATED: projected {projected} > remaining {remaining}"),
        });
    }
    Ok(())
}

pub fn invariant_i3(values: &[&str]) -> Result<(), LedgerError> {
    for value in values {
        if parse_integer(value).is_err() {
            return Err(LedgerError::Invariant {
                code: 6,
                message: format!("I3 VIOLATED: '{value}' is not an integer"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn canonical_hash_matches_reference_shape() {
        let receipt = Receipt {
            ts: "2026-01-01T00:00:00Z".into(),
            event: "dispatch".into(),
            component: "meter".into(),
            task_id: "t1".into(),
            generator_model: "sonnet".into(),
            verifier_model: "opus".into(),
            exit_code: 0,
            tokens_in: 10,
            tokens_out: 20,
            cost_micro_usd: 1500,
            prev_hash: GENESIS.into(),
            hash: String::new(),
        };
        assert_eq!(
            canonical_for_receipt(&receipt),
            "2026-01-01T00:00:00Z|dispatch|meter|t1|sonnet|opus|0|10|20|1500"
        );
        assert_eq!(
            hash_for(GENESIS, &canonical_for_receipt(&receipt)),
            "5b2377ad211b954543ff4a45dffe8ddb9299f3b73e05c6fc085c6f23c6833edc"
        );
    }

    #[test]
    fn invariants_match_exit_codes_and_boundary() {
        assert!(invariant_i1("human", "none").is_ok());
        assert!(i1_looks_like_model("o1-mini"));
        assert!(!i1_looks_like_model("oracle"));
        assert!(matches!(
            invariant_i1("opus", "opus"),
            Err(LedgerError::Invariant { code: 6, .. })
        ));
        assert!(invariant_i2("500", "500").is_ok());
        assert!(matches!(
            invariant_i2("501", "500"),
            Err(LedgerError::Invariant { code: 5, .. })
        ));
        assert!(invariant_i3(&["0", "12", "-3"]).is_ok());
        assert!(matches!(
            invariant_i3(&["1.5"]),
            Err(LedgerError::Invariant { code: 6, .. })
        ));
    }

    #[test]
    fn empty_ledger_is_intact() {
        let path = unique_test_path("empty");
        let report = verify_chain(&path, false, None).unwrap();
        assert_eq!(report.lines, 0);
    }

    #[test]
    fn checkpoint_verifies_only_new_lines() {
        let path = unique_test_path("checkpoint");
        let checkpoint = path.with_extension("ckpt");
        let input = ReceiptInput {
            ts: "t",
            event: "e",
            component: "c",
            task_id: "id",
            generator_model: "sonnet",
            verifier_model: "opus",
            exit_code: "0",
            tokens_in: "1",
            tokens_out: "2",
            cost_micro_usd: "3",
        };
        append_receipt(&path, input).unwrap();
        verify_chain(&path, false, Some(&checkpoint)).unwrap();
        append_receipt(&path, ReceiptInput { ts: "t2", ..input }).unwrap();
        let report = verify_chain(&path, true, Some(&checkpoint)).unwrap();
        assert_eq!(report.verified_from, Some(1));
        cleanup(&path, &checkpoint);
    }

    #[test]
    fn display_renders_every_error_kind() {
        let json_error = serde_json::from_str::<Value>("not json").unwrap_err();
        let errors = [
            LedgerError::Io(std::io::Error::other("io failure")),
            LedgerError::Json(json_error),
            LedgerError::InvalidInteger("abc".into()),
            LedgerError::ChainBroken {
                line: 7,
                reason: "edited".into(),
            },
            LedgerError::Invariant {
                code: 6,
                message: "invariant failure".into(),
            },
            LedgerError::Usage("usage text".into()),
        ];
        for error in errors {
            assert!(!error.to_string().is_empty(), "empty display for {error:?}");
        }
    }

    #[test]
    fn default_path_uses_contract_location_without_override() {
        let prior = std::env::var_os("FLEET_LEDGER");
        std::env::remove_var("FLEET_LEDGER");
        let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("ledger/RECEIPTS.jsonl");
        assert_eq!(default_ledger_path(), expected);
        if let Some(value) = prior {
            std::env::set_var("FLEET_LEDGER", value);
        }
    }

    #[test]
    fn parse_integer_rejects_each_invalid_shape() {
        for value in ["", "-", "12x", "-x"] {
            assert!(
                parse_integer(value).is_err(),
                "accepted invalid integer {value:?}"
            );
        }
        for value in ["0", "12", "-12"] {
            assert!(
                parse_integer(value).is_ok(),
                "rejected valid integer {value:?}"
            );
        }
    }

    #[test]
    fn full_verification_reports_no_checkpoint_start() {
        let path = unique_test_path("full-checkpoint");
        let checkpoint = path.with_extension("ckpt");
        append_receipt(
            &path,
            ReceiptInput {
                ts: "t",
                event: "e",
                component: "c",
                task_id: "id",
                generator_model: "sonnet",
                verifier_model: "opus",
                exit_code: "0",
                tokens_in: "1",
                tokens_out: "2",
                cost_micro_usd: "3",
            },
        )
        .unwrap();
        let report = verify_chain(&path, false, Some(&checkpoint)).unwrap();
        assert_eq!(report.verified_from, None);
        cleanup(&path, &checkpoint);
    }

    #[test]
    fn invariant_i1_covers_sentinel_model_and_non_model_combinations() {
        assert!(invariant_i1("human", "none").is_ok());
        assert!(invariant_i1("opus", "sonnet").is_ok());
        assert!(invariant_i1("opus", "human").is_ok());
        assert!(invariant_i1("human", "opus").is_ok());
        for (generator, verifier) in [
            ("", "opus"),
            ("opus", ""),
            ("human", "oracle"),
            ("opus", "oracle"),
            ("oracle", "opus"),
            ("oracle", "other"),
            ("opus", "opus"),
        ] {
            assert!(
                invariant_i1(generator, verifier).is_err(),
                "accepted {generator:?}, {verifier:?}"
            );
        }
    }

    fn unique_test_path(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fleet-ledger-{label}-{}-{stamp}.jsonl",
            std::process::id()
        ))
    }

    fn cleanup(path: &Path, checkpoint: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(checkpoint);
        let _ = fs::remove_dir_all(lock_path(path));
    }
}
