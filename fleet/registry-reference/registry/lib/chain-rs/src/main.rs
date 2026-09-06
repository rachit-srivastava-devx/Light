use fleet_ledger::{
    append_receipt, default_ledger_path, invariant_i1, invariant_i2, invariant_i3, verify_chain,
    LedgerError, ReceiptInput,
};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = run(&args);
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(exit_code(&error) as i32);
    }
}

fn run(args: &[String]) -> Result<(), LedgerError> {
    let command = args
        .first()
        .map(String::as_str)
        .ok_or_else(|| LedgerError::Usage(usage()))?;
    match command {
        "verify" => run_verify(&args[1..]),
        "append" => run_append(&args[1..]),
        "invariants" => run_invariants(&args[1..]),
        "help" | "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        _ => Err(LedgerError::Usage(format!(
            "unknown command '{command}'\n\n{}",
            usage()
        ))),
    }
}

fn take_option(args: &[String], name: &str) -> Result<(Option<String>, Vec<String>), LedgerError> {
    let mut value = None;
    let mut remaining = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] == name {
            index += 1;
            let next = args
                .get(index)
                .ok_or_else(|| LedgerError::Usage(format!("{name} requires a value")))?;
            value = Some(next.clone());
        } else {
            remaining.push(args[index].clone());
        }
        index += 1;
    }
    Ok((value, remaining))
}

fn run_verify(args: &[String]) -> Result<(), LedgerError> {
    let incremental = args.iter().any(|arg| arg == "--incremental");
    if args
        .iter()
        .any(|arg| arg == "--incremental" || arg == "--ledger" || arg == "--checkpoint")
    {
        let without_incremental: Vec<String> = args
            .iter()
            .filter(|arg| arg.as_str() != "--incremental")
            .cloned()
            .collect();
        let (ledger, without_ledger) = take_option(&without_incremental, "--ledger")?;
        let (checkpoint, rest) = take_option(&without_ledger, "--checkpoint")?;
        if rest.iter().any(|arg| arg.starts_with('-')) || rest.len() > 1 {
            return Err(LedgerError::Usage(
                "verify accepts --ledger PATH, --checkpoint PATH, and --incremental".to_owned(),
            ));
        }
        return print_verify(
            ledger
                .map(PathBuf::from)
                .unwrap_or_else(default_ledger_path),
            incremental,
            checkpoint.map(PathBuf::from),
        );
    }
    print_verify(default_ledger_path(), false, None)
}

fn print_verify(
    path: PathBuf,
    incremental: bool,
    checkpoint: Option<PathBuf>,
) -> Result<(), LedgerError> {
    let report = verify_chain(&path, incremental, checkpoint.as_deref())?;
    if report.lines == 0 {
        println!("chain: empty ledger (vacuously intact)");
    } else if let Some(line) = report.verified_from {
        println!(
            "chain: intact ({} lines), verified from line {}",
            report.lines,
            line + 1
        );
    } else {
        println!("chain: intact ({} lines)", report.lines);
    }
    Ok(())
}

fn run_append(args: &[String]) -> Result<(), LedgerError> {
    let (ledger, positional) = take_option(args, "--ledger")?;
    if positional.len() != 10 || positional.iter().any(|arg| arg.starts_with("--")) {
        return Err(LedgerError::Usage("append [--ledger PATH] TS EVENT COMPONENT TASK_ID GENERATOR VERIFIER EXIT_CODE TOKENS_IN TOKENS_OUT COST_MICRO_USD".to_owned()));
    }
    let hash = append_receipt(
        &ledger
            .map(PathBuf::from)
            .unwrap_or_else(default_ledger_path),
        ReceiptInput {
            ts: &positional[0],
            event: &positional[1],
            component: &positional[2],
            task_id: &positional[3],
            generator_model: &positional[4],
            verifier_model: &positional[5],
            exit_code: &positional[6],
            tokens_in: &positional[7],
            tokens_out: &positional[8],
            cost_micro_usd: &positional[9],
        },
    )?;
    println!("{hash}");
    Ok(())
}

fn run_invariants(args: &[String]) -> Result<(), LedgerError> {
    match args.first().map(String::as_str) {
        Some("i1") if args.len() == 3 => invariant_i1(&args[1], &args[2])?,
        Some("i2") if args.len() == 3 => invariant_i2(&args[1], &args[2])?,
        Some("i3") if args.len() >= 2 => {
            invariant_i3(&args[1..].iter().map(String::as_str).collect::<Vec<_>>())?
        }
        _ => {
            return Err(LedgerError::Usage(
                "invariants i1 GENERATOR VERIFIER | i2 PROJECTED REMAINING | i3 INTEGER..."
                    .to_owned(),
            ))
        }
    }
    println!("invariants: intact");
    Ok(())
}

fn exit_code(error: &LedgerError) -> u8 {
    match error {
        LedgerError::ChainBroken { .. } => 8,
        LedgerError::Invariant { .. } => 6,
        LedgerError::Usage(_) => 2,
        LedgerError::Io(_) | LedgerError::Json(_) | LedgerError::InvalidInteger(_) => 4,
    }
}

fn usage() -> String {
    "fleet-ledger verify [--ledger PATH] [--checkpoint PATH] [--incremental]\nfleet-ledger append [--ledger PATH] TS EVENT COMPONENT TASK_ID GENERATOR VERIFIER EXIT_CODE TOKENS_IN TOKENS_OUT COST_MICRO_USD\nfleet-ledger invariants i1 GENERATOR VERIFIER\nfleet-ledger invariants i2 PROJECTED REMAINING\nfleet-ledger invariants i3 INTEGER...".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn temp_path(label: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fleet-ledger-cli-{label}-{}-{stamp}.jsonl",
            std::process::id()
        ))
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(format!("{}.ckpt", path.display()));
        let _ = fs::remove_file(format!("{}.tmp", path.display()));
        let _ = fs::remove_dir_all(format!("{}.lock", path.display()));
    }

    #[test]
    fn usage_names_every_command_and_option() {
        let text = usage();
        for expected in [
            "fleet-ledger verify",
            "--ledger PATH",
            "--checkpoint PATH",
            "--incremental",
            "fleet-ledger append",
            "fleet-ledger invariants i1",
            "fleet-ledger invariants i2",
            "fleet-ledger invariants i3",
        ] {
            assert!(text.contains(expected), "usage missing {expected:?}");
        }
    }

    #[test]
    fn run_dispatches_each_command_and_rejects_unknown() {
        let ledger = temp_path("dispatch");
        assert!(run(&strings(&["verify", "--ledger", ledger.to_str().unwrap()])).is_ok());
        assert!(run(&strings(&[
            "append",
            "--ledger",
            ledger.to_str().unwrap(),
            "ts",
            "event",
            "component",
            "task",
            "sonnet",
            "opus",
            "0",
            "1",
            "2",
            "3",
        ]))
        .is_ok());
        assert!(run(&strings(&["invariants", "i3", "1"])).is_ok());
        for command in ["help", "--help", "-h"] {
            assert!(run(&strings(&[command])).is_ok());
        }
        assert!(matches!(
            run(&strings(&["unknown"])),
            Err(LedgerError::Usage(_))
        ));
        cleanup(&ledger);
    }

    #[test]
    fn take_option_handles_values_positionals_repeats_and_missing_values() {
        let (value, remaining) =
            take_option(&strings(&["before", "--ledger", "a", "after"]), "--ledger").unwrap();
        assert_eq!(value.as_deref(), Some("a"));
        assert_eq!(remaining, strings(&["before", "after"]));

        let (value, remaining) =
            take_option(&strings(&["--ledger", "a", "--ledger", "b"]), "--ledger").unwrap();
        assert_eq!(value.as_deref(), Some("b"));
        assert!(remaining.is_empty());

        let (value, remaining) = take_option(&strings(&["positional"]), "--ledger").unwrap();
        assert_eq!(value, None);
        assert_eq!(remaining, strings(&["positional"]));
        assert!(matches!(
            take_option(&strings(&["--ledger"]), "--ledger"),
            Err(LedgerError::Usage(message)) if message == "--ledger requires a value"
        ));
    }

    #[test]
    fn verify_and_append_cover_valid_and_invalid_cli_shapes() {
        let ledger = temp_path("verify-append");
        let checkpoint = ledger.with_extension("checkpoint");
        let path = ledger.to_str().unwrap();
        assert!(run_verify(&strings(&["--ledger", path])).is_ok());

        let mut append = vec![
            "--ledger",
            path,
            "ts",
            "event",
            "component",
            "task",
            "sonnet",
            "opus",
            "0",
            "1",
            "2",
            "3",
        ];
        assert!(run_append(&strings(&append)).is_ok());
        assert!(run_verify(&strings(&[
            "--ledger",
            path,
            "--checkpoint",
            checkpoint.to_str().unwrap(),
            "--incremental",
        ]))
        .is_ok());

        append.pop();
        assert!(matches!(
            run_append(&strings(&append)),
            Err(LedgerError::Usage(_))
        ));
        append.push("--unexpected");
        assert!(matches!(
            run_append(&strings(&append)),
            Err(LedgerError::Usage(_))
        ));
        assert!(matches!(
            run_verify(&strings(&["--ledger", path, "extra", "another"])),
            Err(LedgerError::Usage(_))
        ));
        assert!(matches!(
            run_verify(&strings(&["--ledger", path, "-unexpected"])),
            Err(LedgerError::Usage(_))
        ));

        fs::write(&ledger, "not json\n").unwrap();
        assert!(print_verify(ledger.clone(), false, None).is_err());
        cleanup(&ledger);
        cleanup(&checkpoint);
    }

    #[test]
    fn invariants_enforce_every_arity_boundary() {
        let cases = [
            (&["i1"][..], true),
            (&["i1", "opus"][..], true),
            (&["i1", "opus", "sonnet"][..], false),
            (&["i1", "opus", "sonnet", "extra"][..], true),
            (&["i2"][..], true),
            (&["i2", "1"][..], true),
            (&["i2", "1", "2"][..], false),
            (&["i2", "1", "2", "extra"][..], true),
            (&["i3"][..], true),
            (&["i3", "1"][..], false),
            (&["i3", "1", "2"][..], false),
            (&["i3", "1", "2", "3"][..], false),
        ];
        for (arguments, should_fail) in cases {
            let result = run_invariants(&strings(arguments));
            assert_eq!(result.is_err(), should_fail, "arguments: {arguments:?}");
        }
    }

    #[test]
    fn exit_codes_match_fleet_contract() {
        let json_error = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let cases = [
            (
                LedgerError::ChainBroken {
                    line: 1,
                    reason: "x".into(),
                },
                8,
            ),
            (
                LedgerError::Invariant {
                    code: 5,
                    message: "x".into(),
                },
                6,
            ),
            (
                LedgerError::Invariant {
                    code: 6,
                    message: "x".into(),
                },
                6,
            ),
            (LedgerError::Usage("x".into()), 2),
            (LedgerError::InvalidInteger("x".into()), 4),
            (LedgerError::Io(std::io::Error::other("x")), 4),
            (LedgerError::Json(json_error), 4),
        ];
        for (error, expected) in cases {
            assert_eq!(exit_code(&error), expected);
        }
    }
}
