/// The tier a role maps to. `role_allows` (in `allow.rs`) is the only place tier <-> role is
/// decided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tier {
    Lead,
    Worker,
    Cheap,
}

/// Coarse task classification the safety-policy stage (stage 2) reasons over.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskClass {
    General,
    Implementation,
    HumanOnly,
}
