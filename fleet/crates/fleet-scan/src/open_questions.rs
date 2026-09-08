//! `OpenQuestions` and `Assessment`: the ADHD-safe verdict shape merge_questions produces.

use crate::probe::Question;

/// 1..=4 deduplicated, "why"-bearing questions, sorted by descending gap severity. The only way
/// to construct one is `merge_questions`, so an `Assessment::Open` can never carry 0 or >4 items.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct OpenQuestions(Vec<Question>);

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OpenQuestionsError {
    #[error("cannot construct OpenQuestions with 0 items -- use Assessment::Clear instead")]
    Empty,
    #[error("cannot construct OpenQuestions with more than 4 items ({0} given) -- caller must cap")]
    TooMany(usize),
}

impl OpenQuestions {
    pub fn new(items: Vec<Question>) -> Result<Self, OpenQuestionsError> {
        match items.len() {
            0 => Err(OpenQuestionsError::Empty),
            1..=4 => Ok(Self(items)),
            n => Err(OpenQuestionsError::TooMany(n)),
        }
    }

    pub fn as_slice(&self) -> &[Question] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<Question> {
        self.0
    }
}

/// The ADHD-safe verdict: either the requirement is clear enough to proceed, or here are at most
/// 4 ranked questions.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "verdict", rename_all = "lowercase")]
pub enum Assessment {
    Clear,
    Open(OpenQuestions),
}
