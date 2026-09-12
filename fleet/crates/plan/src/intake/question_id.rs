//! The 15-id catalog `intake.sh`'s `valid_id()` (`intake.sh:55-60`) hard-codes.

/// One of the 15 known intake rubric question ids.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum QuestionId {
    Scope,
    Success,
    CliFlag,
    SchemaMigration,
    UiView,
    ApiEndpoint,
    Deletion,
    Scale,
    Tenancy,
    Precision,
    Auth,
    Ownership,
    Failure,
    Unhappy,
    RenameRefactor,
}

/// `id` did not match any of the 15 known question ids.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0:?} is not a known intake question id")]
pub struct UnknownQuestionId(pub String);

const ALL: [(QuestionId, &str); 15] = {
    use QuestionId::*;
    [
        (Scope, "scope"),
        (Success, "success"),
        (CliFlag, "cli_flag"),
        (SchemaMigration, "schema_migration"),
        (UiView, "ui_view"),
        (ApiEndpoint, "api_endpoint"),
        (Deletion, "deletion"),
        (Scale, "scale"),
        (Tenancy, "tenancy"),
        (Precision, "precision"),
        (Auth, "auth"),
        (Ownership, "ownership"),
        (Failure, "failure"),
        (Unhappy, "unhappy"),
        (RenameRefactor, "rename_refactor"),
    ]
};

impl QuestionId {
    /// Mirrors `valid_id`'s case arms (`intake.sh:56-59`) plus `emit_core`'s two ids.
    pub fn parse(id: &str) -> Result<Self, UnknownQuestionId> {
        ALL.iter()
            .find(|(_, name)| *name == id)
            .map(|(q, _)| *q)
            .ok_or_else(|| UnknownQuestionId(id.to_string()))
    }

    pub fn as_str(self) -> &'static str {
        ALL.iter().find(|(q, _)| *q == self).map(|(_, n)| *n).expect("total")
    }
}
