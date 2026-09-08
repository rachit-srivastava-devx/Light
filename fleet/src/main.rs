//! fleet-cli composition root. Parses args, builds the runtime, wires the pipeline graph, and
//! calls into fleet-* crates. Contains no decision logic, no hashing, no subprocess business
//! rules (BLUEPRINT §2). **Restate deferred** -- see `pipeline/graph.rs`'s doc comment.

mod cli;
mod dispatch;
mod pipeline;
mod print;
mod runtime;

use clap::{CommandFactory, FromArgMatches};
use std::path::PathBuf;

fn main() {
    // Builder-side help-text augmentation (`cli::help_text`) instead of doc comments on the
    // `Commands` enum, which would push `cli/root.rs` past its 80-line cap.
    let augmented = cli::help_text::with_descriptions(cli::Cli::command());
    let matches = augmented.get_matches();
    let cli = cli::Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    print::style::set_no_color_flag(cli.no_color);
    let config = runtime::load_config().unwrap_or_else(|e| {
        eprintln!("config load failed: {e}");
        std::process::exit(fleet_types::ExitCode::Env.as_i32());
    });
    let state_dir = PathBuf::from(&config.state_dir);
    // Only work-spawning commands may be refused for capacity (see `cli::capacity_scope`):
    // gating `completions`/`doctor`/`__agent` broke the installer and killed spawned children.
    let preflight_cfg = runtime::capacity::PreflightConfig::from_env(config.review_cap, config.ram_lanes);
    let measured = runtime::capacity::preflight(&runtime::capacity::StdCapacityProbe, &preflight_cfg);
    let cap = match (measured, cli.command.is_capacity_gated()) {
        (Ok(cap), _) => cap,
        (Err(refusal), true) => {
            // Route through the same structured render path every other refusal/verdict line
            // uses, instead of a one-off `eprintln!`, so `REFUSED ...` looks identical here and
            // in `fleet gate`'s output (colour/NO_COLOR honoured the same way too).
            let event = print::render_event::Event::Refusal {
                source: "fleet".into(),
                reason: format!("system capacity check failed: {refusal}"),
            };
            print::human_stream::emit(&event, &print::style::Style::detect());
            std::process::exit(fleet_types::ExitCode::Refusal.as_i32());
        }
        // Not gated: answer anyway, single-lane. Introspection must work on a loaded machine.
        (Err(_), false) => runtime::ConcurrencyCap::minimum(),
    };
    let tokio_rt = runtime::tokio_rt::build(cap).expect("tokio runtime builds");
    let rayon_pool = runtime::rayon_pool::build(cap).expect("rayon pool builds");

    let outcome =
        rayon_pool.install(|| tokio_rt.block_on(async { dispatch::run(cli.command, &state_dir, cap) }));
    match outcome {
        Ok(()) => std::process::exit(fleet_types::ExitCode::Ok.as_i32()),
        Err(e) => {
            eprintln!("fleet: {e}");
            std::process::exit(e.exit_code().as_i32());
        }
    }
}
