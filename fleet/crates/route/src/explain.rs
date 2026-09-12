#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CandidateId(pub String);

#[derive(Clone, Debug)]
pub struct StageEvidence {
    pub stage: String,
    pub candidates_in: usize,
    pub candidates_out: usize,
}

#[derive(Clone, Debug)]
pub struct ReservationRequest {
    pub candidate_id: CandidateId,
    pub reserved_until: std::time::Instant,
}

#[derive(Clone, Debug)]
pub struct RouteDecision {
    pub selected: CandidateId,
    pub selected_cost: u64,
    pub stages: Vec<StageEvidence>,
    pub reservation: ReservationRequest,
    pub snapshot_digest: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RouteRefusal {
    #[error("no candidates after {stage}: {reason}")]
    NoCandidates { stage: String, reason: String, candidates_in: usize },
    #[error("empty catalog")]
    EmptyCatalog,
}

impl RouteRefusal {
    pub fn stages(&self) -> Vec<StageEvidence> {
        match self {
            RouteRefusal::NoCandidates { stage, candidates_in, .. } => {
                vec![StageEvidence {
                    stage: stage.clone(),
                    candidates_in: *candidates_in,
                    candidates_out: 0,
                }]
            }
            RouteRefusal::EmptyCatalog => vec![StageEvidence {
                stage: "catalog".to_string(),
                candidates_in: 0,
                candidates_out: 0,
            }],
        }
    }
}
