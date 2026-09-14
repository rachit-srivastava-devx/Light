//! Reedline-backed input with Fleet-specific mode and clear keybindings.

use super::fallback_read::fallback_read;
use super::input_prompt::InputPrompt;
use reedline::{
    default_emacs_keybindings, Emacs, KeyCode, KeyModifiers, Reedline, ReedlineEvent, Signal,
};

const TOGGLE_MODE: &str = "fleet:toggle-mode";
const CLEAR_SCREEN: &str = "fleet:clear-screen";

#[derive(Debug, PartialEq, Eq)]
pub enum ReadOutcome {
    Submit(String),
    ToggleMode,
    Clear,
    Exit,
}

pub struct LineReader {
    editor: Reedline,
}

impl LineReader {
    pub fn new() -> Self {
        let mut keys = default_emacs_keybindings();
        keys.add_binding(
            KeyModifiers::SHIFT,
            KeyCode::BackTab,
            ReedlineEvent::ExecuteHostCommand(TOGGLE_MODE.into()),
        );
        keys.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('l'),
            ReedlineEvent::ExecuteHostCommand(CLEAR_SCREEN.into()),
        );
        Self {
            editor: Reedline::create().with_edit_mode(Box::new(Emacs::new(keys))),
        }
    }

    pub fn read_input(&mut self, prompt: &str, color: bool) -> ReadOutcome {
        let prompt = InputPrompt::new(prompt, color);
        match self.editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => ReadOutcome::Submit(line),
            Ok(Signal::HostCommand(command)) if command == TOGGLE_MODE => ReadOutcome::ToggleMode,
            Ok(Signal::HostCommand(command)) if command == CLEAR_SCREEN => ReadOutcome::Clear,
            Ok(Signal::CtrlC | Signal::CtrlD) => ReadOutcome::Exit,
            Ok(_) => ReadOutcome::Exit,
            Err(_) => fallback_read(),
        }
    }
}
