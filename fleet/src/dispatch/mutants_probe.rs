//! `cargo-mutants` availability, split out of `verify_ports.rs` to keep that file under the
//! 80-line cap. D27: the mutants gate must stay opt-in even when `cargo-mutants` happens to be
//! on `$PATH` -- but "not opted in" and "not installed" used to collapse into one boolean, so a
//! machine that genuinely has `cargo-mutants` on `PATH` printed "unavailable" (which reads like
//! "go install this"), when the real reason was that `FLEET_MUTANTS=1` was not set.

use std::process::Command;

pub enum Availability {
    Yes,
    NotOptedIn,
    NotOnPath,
}

pub fn probe() -> Availability {
    if std::env::var("FLEET_MUTANTS").as_deref() != Ok("1") {
        return Availability::NotOptedIn;
    }
    if on_path("cargo-mutants") {
        Availability::Yes
    } else {
        Availability::NotOnPath
    }
}

pub fn on_path(name: &str) -> bool {
    Command::new("which").arg(name).output().map(|o| o.status.success()).unwrap_or(false)
}

pub fn reason(a: Availability) -> String {
    match a {
        Availability::Yes => "unavailable".to_string(),
        Availability::NotOptedIn => "skipped: set FLEET_MUTANTS=1 to run it".to_string(),
        Availability::NotOnPath => "skipped: cargo-mutants not found on PATH".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_names_the_real_cause_for_each_availability() {
        assert_eq!(reason(Availability::NotOptedIn), "skipped: set FLEET_MUTANTS=1 to run it");
        assert_eq!(reason(Availability::NotOnPath), "skipped: cargo-mutants not found on PATH");
    }
}
