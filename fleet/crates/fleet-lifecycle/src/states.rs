//! The closed set of 16 lifecycle states. Ported verbatim from
//! `fleet/keel/fleet/src/lifecycle.rs:35-93`.

mod sealed {
    pub trait Sealed {}
}

/// Marker implemented only by lifecycle states declared in this crate. Sealed: no
/// downstream crate can invent a 17th state and have it accepted by `Task<S>`.
pub trait State: sealed::Sealed {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Intake;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Specified;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reviewed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Decomposed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contracted;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Briefed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Leased;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Building;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Built;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Verifying;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Verified;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attested;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Accepted;
/// The change has been pushed and a real pull request opened. Not `Merged`/`Landed` -- the
/// change is proposed for human integration, never self-merged (author != integrator).
/// Terminal for an agent: there is no `merge` edge and never will be
/// (`merge_is_not_a_lifecycle_edge` compile-fail test enforces this).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Proposed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Observed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refused;

macro_rules! seal_states {
    ($($state:ident),+ $(,)?) => {
        $(
            impl sealed::Sealed for $state {}
            impl State for $state {}
        )+
    };
}

seal_states!(
    Intake, Specified, Reviewed, Decomposed, Contracted, Briefed, Leased, Building, Built,
    Verifying, Verified, Attested, Accepted, Proposed, Observed, Refused,
);
