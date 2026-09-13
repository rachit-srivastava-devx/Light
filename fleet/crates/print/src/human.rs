//! Human-formatted line helpers shared across `dispatch/*` for the ~15 subcommands that report a
//! flat set of facts rather than a multi-gate run (`status`, `doctor`, `meter`, `route`, `roles`,
//! `role-check`, `ledger`, `graph`, `impact`, `lifecycle`, `agents`, `rollback`, `sow`). `line` is
//! deliberately unstyled and quiet -- these are one-shot answers, not a `-- summary --` block, and
//! wrapping a one-line answer in gate/summary framing would be noise (`fleet version` does not
//! need a verdict badge). `ok`/`refused` DO reuse `print::style::Style` so a severity word looks
//! the same everywhere: colour only on a real TTY, honouring `NO_COLOR`/`TERM=dumb`/`--no-color`,
//! exactly like the structured `run`/`gate`/`swarm` render path -- no separate colour logic here.

use super::style::{self, Style};

pub fn line(label: &str, value: impl std::fmt::Display) {
    println!("{label}: {value}");
}

pub fn ok(msg: impl std::fmt::Display) {
    let style = Style::detect();
    println!("{} {msg}", style.paint(style::GREEN, "ok:"));
}

pub fn refused(msg: impl std::fmt::Display) {
    let style = Style::detect();
    eprintln!("{} {msg}", style.paint(style::RED, "REFUSED:"));
}
