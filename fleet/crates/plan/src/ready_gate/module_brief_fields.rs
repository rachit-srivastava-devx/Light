//! Allowed/forbidden field-name tables. Verbatim from `lld.rs:129-178`.

pub(crate) const ALLOWED_MODULE_BRIEF_FIELDS: &[&str] = &[
    "schema_version", "node_id", "grain", "purpose", "owner", "owner_path", "interface",
    "data_owned", "deps", "registry", "acceptance", "non_goals", "open_questions", "guarantees",
    "alternatives", "failure_story",
];
pub(crate) const FORBIDDEN_MODULE_BRIEF_FIELDS: &[&str] =
    &["content_hash", "freeze_id", "depth_evidence", "depth_score", "stamped_by", "state", "version"];
pub(crate) const ALLOWED_FREEZE_FIELDS: &[&str] = &[
    "schema_version", "freeze_id", "version", "node_id", "content_hash", "decision", "why",
    "killed_alternatives", "accepts_when", "owner", "depth_evidence", "supersedes", "stamped_by",
];
pub(crate) const ALLOWED_SOW_SEED_FIELDS: &[&str] =
    &["restatement", "blind_suite_seed", "blast_radius", "owner", "registry_verdict"];
pub(crate) const ALLOWED_LLD_V1_FIELDS: &[&str] = &["schema_version", "module_brief", "freeze", "sow_seed"];
