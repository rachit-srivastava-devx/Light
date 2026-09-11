//! Blueprint streaming for parallel module execution.
//!
//! This module provides types and functions for streaming module blueprints back to the user
//! as an L8 engineer would - teaching per module with explanations, questions, and guidance.

use fleet_types::Blueprint;
use std::fmt;

/// Update event for blueprint streaming.
/// Sent to the user as the L8 engineer progresses through module planning.
#[derive(Clone, Debug, PartialEq)]
pub enum BlueprintUpdate {
    /// Planning started for a module
    Started { module_id: String },
    /// Planning progress update
    Progress { module_id: String, step: String, percentage: f32 },
    /// A question from the L8 engineer
    Question { module_id: String, question: String },
    /// An explanation from the L8 engineer
    Explanation { module_id: String, text: String },
    /// A blueprint is ready for review
    Ready { module_id: String, blueprint: Blueprint },
    /// All modules have been processed
    Done,
}

impl fmt::Display for BlueprintUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlueprintUpdate::Started { module_id } => {
                write!(f, "Starting blueprint for module: {}", module_id)
            }
            BlueprintUpdate::Progress { module_id, step, percentage } => {
                write!(f, "[{}] {}: {}%", module_id, step, (percentage * 100.0) as u8)
            }
            BlueprintUpdate::Question { module_id, question } => {
                write!(f, "[{}] Question: {}", module_id, question)
            }
            BlueprintUpdate::Explanation { module_id, text } => {
                write!(f, "[{}] {}", module_id, text)
            }
            BlueprintUpdate::Ready { module_id, blueprint } => {
                write!(f, "[{}] Blueprint ready", module_id)?;
                writeln!(f, "  Plan: {}", blueprint.plan)?;
                writeln!(f, "  Acceptance: {:?}", blueprint.acceptance)?;
                Ok(())
            }
            BlueprintUpdate::Done => {
                write!(f, "All blueprints completed")
            }
        }
    }
}

/// Blueprint stream that sends updates to a channel.
#[derive(Clone, Debug)]
pub struct BlueprintStream {
    /// Maximum number of modules to process in parallel
    pub concurrency: usize,
}

impl BlueprintStream {
    /// Create a new blueprint stream with default concurrency.
    pub fn new() -> Self {
        Self { concurrency: 3 }
    }

    /// Create a new blueprint stream with custom concurrency.
    pub fn with_concurrency(concurrency: usize) -> Self {
        Self { concurrency }
    }
}

impl Default for BlueprintStream {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blueprint_update_started() {
        let update = BlueprintUpdate::Started { module_id: "module-1".to_string() };
        assert_eq!(format!("{}", update), "Starting blueprint for module: module-1");
    }

    #[test]
    fn test_blueprint_update_progress() {
        let update = BlueprintUpdate::Progress {
            module_id: "module-1".to_string(),
            step: "Analysis".to_string(),
            percentage: 0.5,
        };
        assert_eq!(format!("{}", update), "[module-1] Analysis: 50%");
    }

    #[test]
    fn test_blueprint_update_question() {
        let update = BlueprintUpdate::Question {
            module_id: "module-1".to_string(),
            question: "What is the expected behavior?".to_string(),
        };
        assert_eq!(format!("{}", update), "[module-1] Question: What is the expected behavior?");
    }

    #[test]
    fn test_blueprint_update_explanation() {
        let update = BlueprintUpdate::Explanation {
            module_id: "module-1".to_string(),
            text: "This module handles user authentication.".to_string(),
        };
        assert_eq!(format!("{}", update), "[module-1] This module handles user authentication.");
    }

    #[test]
    fn test_blueprint_update_done() {
        let update = BlueprintUpdate::Done;
        assert_eq!(format!("{}", update), "All blueprints completed");
    }

    #[test]
    fn test_blueprint_stream_default_concurrency() {
        let stream = BlueprintStream::new();
        assert_eq!(stream.concurrency, 3);
    }

    #[test]
    fn test_blueprint_stream_custom_concurrency() {
        let stream = BlueprintStream::with_concurrency(5);
        assert_eq!(stream.concurrency, 5);
    }
}
