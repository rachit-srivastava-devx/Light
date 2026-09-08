//! Pure shared vocabulary at the root of the fleet crate DAG.
//!
//! `fleet-types` imports nothing from any other `fleet-*` crate and is imported by nearly all of
//! them. It owns the plain data types, newtype identifiers, and typed errors that cross crate
//! boundaries. It performs no IO, spawns no process, reads no clock, and makes no decision --
//! every fn here is a pure, total value transformation (parse / format / validate / compare).

mod attest_predicate;
mod attest_stmt;
mod attest_subject;
mod exit_code;
mod gate_refusal;
mod ident;
mod lifecycle;
mod node_id;
mod prev_hash;
mod receipt;
mod receipt_event;
mod role;
mod tokens;

pub use attest_predicate::{AttestationElements, DeliveryPredicate};
pub use attest_stmt::{Attestation, AttestationBuilder, DeliveryAttestationV1, InTotoStatementV1};
pub use attest_subject::{AttestationDigest, AttestationSubject, BareBlake3Digest, DeliveryTier};
pub use exit_code::{ExitCode, UnknownExitCode};
pub use gate_refusal::GateRefusal;
pub use ident::{valid_artifact_id, EmptyIdentifier, LaneId, TaskId};
pub use lifecycle::LifecycleState;
pub use node_id::{BadNodeId, NodeId};
pub use prev_hash::PrevHash;
pub use receipt::{BadBlake3Hash, Blake3Hash};
pub use receipt_event::{Receipt, ReceiptEvent, SchemaV1};
pub use role::{Role, UnknownRole};
pub use tokens::{Tokens, TokensOverflow};
