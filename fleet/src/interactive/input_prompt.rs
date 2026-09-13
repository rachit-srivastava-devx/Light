//! Fleet-owned Reedline prompt without Reedline's extra default indicator.

use reedline::{Color, Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus};
use std::borrow::Cow;

pub struct InputPrompt {
    left: String,
    color: bool,
}

impl InputPrompt {
    pub fn new(left: &str, color: bool) -> Self {
        Self {
            left: left.into(),
            color,
        }
    }
}

impl Prompt for InputPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.left)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("  ")
    }

    fn render_prompt_history_search_indicator(&self, search: PromptHistorySearch) -> Cow<'_, str> {
        let state = match search.status {
            PromptHistorySearchStatus::Passing => "search",
            PromptHistorySearchStatus::Failing => "no match",
        };
        Cow::Owned(format!("({state}: {}) ", search.term))
    }

    fn get_prompt_color(&self) -> Color {
        if self.color {
            Color::LightGreen
        } else {
            Color::Default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fleet_prompt_has_no_duplicate_editor_indicator() {
        let prompt = InputPrompt::new("› ", false);
        assert_eq!(prompt.render_prompt_left(), "› ");
        assert_eq!(prompt.render_prompt_indicator(PromptEditMode::Emacs), "");
        assert_eq!(prompt.render_prompt_right(), "");
    }
}
