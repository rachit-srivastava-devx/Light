//! Approval — narrow, expiring, single-use publication capability.
//!
//! Only the operator/parent boundary may mint or revoke an approval.
//! A worker result, model statement, or lifecycle state cannot imply approval.

mod check;
mod grant;
mod port;

pub use check::{approve, consume};
pub use grant::{ApprovalError, ApprovalGrant, ApprovalRequest};
pub use port::ApprovalStore;
