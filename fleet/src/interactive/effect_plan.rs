//! Deterministic effect discovery for the interactive approval boundary.

use super::effect_parse::classify;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Model,
    LocalWrite,
    LocalCommand,
    RemotePublish,
    DeferredSchedule,
}

pub fn discover(task: &str) -> Vec<Effect> {
    let mut effects = vec![Effect::Model];
    for raw in task.to_ascii_lowercase().split(['.', ',', ';', '\n']) {
        let signals = classify(raw);
        add_if(&mut effects, Effect::LocalWrite, signals.write);
        add_if(&mut effects, Effect::LocalCommand, signals.command);
        add_if(&mut effects, Effect::RemotePublish, signals.publish);
        add_if(&mut effects, Effect::DeferredSchedule, signals.schedule);
    }
    effects
}

pub fn label(effect: Effect) -> &'static str {
    match effect {
        Effect::Model => "send this task to the selected model provider; quota may be consumed",
        Effect::LocalWrite => "write or modify files in the current repository",
        Effect::LocalCommand => "run local build, test, or verification commands",
        Effect::RemotePublish => "push, publish, release, or create a remote pull request",
        Effect::DeferredSchedule => "create or change a deferred scheduled task",
    }
}

fn add_if(effects: &mut Vec<Effect>, effect: Effect, present: bool) {
    if present && !effects.contains(&effect) {
        effects.push(effect);
    }
}
