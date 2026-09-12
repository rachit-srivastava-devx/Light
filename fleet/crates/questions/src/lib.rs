pub mod merge;
pub mod types;

pub use merge::{merge, merge_probes};
pub use types::{MergeError, Question, QuestionSet};
