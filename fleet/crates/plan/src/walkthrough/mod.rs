//! Ask #6 -- "teach the user on the implementation suggested, walk him through it": a
//! deterministic, structured outline of a plan built purely from data this crate already
//! validates. No model call lives here; a worker may later turn this into prose.

mod build;
mod error;
pub(crate) mod extract;
mod sections;
mod types;
mod work_order;

pub use build::build_walkthrough;
pub use error::WalkthroughError;
pub use types::{AcceptancePreviewItem, BuildItem, DecisionItem, RiskItem, Walkthrough, WorkOrderItem};
