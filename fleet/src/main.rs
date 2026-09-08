//! fleet-cli composition root. Parses args, builds the runtime, wires the pipeline graph, and
//! calls into fleet-* crates. Contains no decision logic, no hashing, no subprocess business
//! rules (BLUEPRINT §2). **Restate deferred** -- see `pipeline/graph.rs`'s doc comment.

mod cli;
mod dispatch;
mod pipeline;
mod print;
mod runtime;

use clap::Parser;
use std::path::PathBuf;

fn main() {
    let cli = cli::Cli::parse();
    let config = runtime::load_config().unwrap_or_else(|e| {
        eprintln!("config load failed: {e}");
        std::process::exit(fleet_types::ExitCode::Env.as_i32());
    });
    let state_dir = PathBuf::from(&config.state_dir);
    let cap = runtime::ConcurrencyCap::from_env(config.ram_lanes.unwrap_or(usize::MAX), config.review_cap);
    let tokio_rt = runtime::tokio_rt::build(cap).expect("tokio runtime builds");
    let rayon_pool = runtime::rayon_pool::build(cap).expect("rayon pool builds");

    let outcome = rayon_pool.install(|| tokio_rt.block_on(async { dispatch::run(cli.command, &state_dir) }));
    match outcome {
        Ok(()) => std::process::exit(fleet_types::ExitCode::Ok.as_i32()),
        Err(e) => {
            eprintln!("fleet: {e}");
            std::process::exit(e.exit_code().as_i32());
        }
    }
}
