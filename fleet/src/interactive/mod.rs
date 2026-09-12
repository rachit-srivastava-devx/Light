//! Claude Code-inspired interactive developer experience for Fleet.

mod agent_loop;
mod banner;
mod commands;
mod help;
mod keys;
mod line_reader;
mod path_check;
mod raw_terminal;
mod repl;
mod status_bar;
mod theme;
mod update_check;

#[cfg(test)]
mod commands_tests;

pub use repl::run_loop as run;
