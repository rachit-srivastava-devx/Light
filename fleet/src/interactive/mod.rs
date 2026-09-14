//! Claude Code-inspired interactive developer experience for Fleet.

mod agent_adapter;
mod agent_loop;
mod agent_output;
mod approval;
mod banner;
mod commands;
mod effect_parse;
mod effect_plan;
mod effect_words;
mod fallback_read;
mod help;
mod input_prompt;
mod line_reader;
mod outcome_receipt;
mod path_check;
mod repl;
mod status_bar;
mod theme;
mod update_check;

#[cfg(test)]
mod commands_tests;
#[cfg(test)]
mod effect_plan_tests;
#[cfg(test)]
mod repl_tests;

pub use repl::run_loop as run;
