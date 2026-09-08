//! Human-formatted line helpers shared across `dispatch/*` -- printing only, no decisions.

pub fn line(label: &str, value: impl std::fmt::Display) {
    println!("{label}: {value}");
}

pub fn ok(msg: impl std::fmt::Display) {
    println!("ok: {msg}");
}

pub fn refused(msg: impl std::fmt::Display) {
    eprintln!("refused: {msg}");
}
