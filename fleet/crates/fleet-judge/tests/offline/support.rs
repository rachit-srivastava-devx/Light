//! Shared fixtures for the offline pure-core tests: a fake `JudgeModel` that returns a canned
//! `RawVerdict`, a model that always fails, and a fixed `Criteria`/`Candidate` pair.

use fleet_judge::{Candidate, Criteria, JudgeModel, ModelError, RawVerdict};

pub struct FakeModel(pub RawVerdict);

impl JudgeModel for FakeModel {
    fn call(&self, _: &Criteria, _: &Candidate) -> Result<RawVerdict, ModelError> {
        Ok(self.0.clone())
    }
}

pub struct FailingModel;

impl JudgeModel for FailingModel {
    fn call(&self, _: &Criteria, _: &Candidate) -> Result<RawVerdict, ModelError> {
        Err(ModelError::new("connection refused"))
    }
}

pub fn criteria() -> Criteria {
    Criteria {
        instructions: "pick the best UI".into(),
        labels: vec!["cli".into(), "form".into()],
    }
}

pub fn candidate() -> Candidate {
    Candidate {
        input: "delete every file older than 30 days".into(),
    }
}
