#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    Lead,
    Builder,
    Verifier,
    Designer,
    Meter,
}

impl Role {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "lead" => Some(Self::Lead),
            "builder" => Some(Self::Builder),
            "verifier" => Some(Self::Verifier),
            "designer" => Some(Self::Designer),
            "meter" => Some(Self::Meter),
            _ => None,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Lead => "lead",
            Self::Builder => "builder",
            Self::Verifier => "verifier",
            Self::Designer => "designer",
            Self::Meter => "meter",
        }
    }

    pub(crate) fn bandwidth(self) -> u64 {
        match self {
            Self::Lead => 2,
            Self::Builder => 4,
            Self::Verifier => 3,
            Self::Designer => 2,
            Self::Meter => 2,
        }
    }

    pub(crate) fn allocation_fitness(self) -> u64 {
        match self {
            Self::Lead => 3,
            Self::Builder => 5,
            Self::Verifier => 4,
            Self::Designer => 3,
            Self::Meter => 2,
        }
    }

    fn owned_gate(self) -> &'static str {
        match self {
            Self::Lead => "contract",
            Self::Builder => "implementation",
            Self::Verifier => "independent-verification",
            Self::Designer => "design-a11y",
            Self::Meter => "budget",
        }
    }

    fn may_write_code(self) -> bool {
        matches!(self, Self::Builder)
    }
}

pub(crate) const ALL: [Role; 5] = [
    Role::Lead,
    Role::Builder,
    Role::Verifier,
    Role::Designer,
    Role::Meter,
];

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Check<'a> {
    pub(crate) role: Role,
    pub(crate) diff_adds_code: bool,
    pub(crate) builder_model: Option<&'a str>,
    pub(crate) verifier_model: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    LeadWroteCode,
    SelfVerified,
}

impl Refusal {
    pub(crate) fn reason(self) -> &'static str {
        match self {
            Self::LeadWroteCode => "LEAD_WROTE_CODE",
            Self::SelfVerified => "SELF_VERIFIED",
        }
    }
}

pub(crate) fn evaluate(check: &Check<'_>) -> Result<(), Refusal> {
    if check.role == Role::Lead && check.diff_adds_code {
        return Err(Refusal::LeadWroteCode);
    }
    if check.role == Role::Verifier {
        if let (Some(builder), Some(verifier)) = (check.builder_model, check.verifier_model) {
            if builder == verifier {
                return Err(Refusal::SelfVerified);
            }
        }
    }
    Ok(())
}

pub(crate) fn print_roles() {
    println!("ROLE      OWNS GATE                 MAY WRITE IMPLEMENTATION CODE");
    for role in ALL {
        println!(
            "{:<9} {:<25} {}",
            role.name(),
            role.owned_gate(),
            if role.may_write_code() { "yes" } else { "no" }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{evaluate, Check, Refusal, Role};

    fn check(role: Role, diff_adds_code: bool) -> Check<'static> {
        Check {
            role,
            diff_adds_code,
            builder_model: None,
            verifier_model: None,
        }
    }

    #[test]
    fn lead_code_gate_covers_both_directions() {
        assert_eq!(
            evaluate(&check(Role::Lead, true)),
            Err(Refusal::LeadWroteCode)
        );
        assert_eq!(evaluate(&check(Role::Lead, false)), Ok(()));
    }

    #[test]
    fn builder_code_gate_allows_both_diff_shapes() {
        assert_eq!(evaluate(&check(Role::Builder, true)), Ok(()));
        assert_eq!(evaluate(&check(Role::Builder, false)), Ok(()));
    }

    #[test]
    fn verifier_model_gate_covers_both_directions() {
        let same = Check {
            role: Role::Verifier,
            diff_adds_code: false,
            builder_model: Some("m1"),
            verifier_model: Some("m1"),
        };
        assert_eq!(evaluate(&same), Err(Refusal::SelfVerified));

        let distinct = Check {
            verifier_model: Some("m2"),
            ..same
        };
        assert_eq!(evaluate(&distinct), Ok(()));
    }
}
