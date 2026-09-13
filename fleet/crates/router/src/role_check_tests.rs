use super::*;

#[test]
fn lead_code_gate_covers_both_directions() {
    let wrote_code = RoleCheck {
        role: Role::Lead,
        diff_adds_code: true,
        builder_model: None,
        verifier_model: None,
    };
    assert_eq!(
        evaluate_role_check(&wrote_code),
        Err(RoleRefusal::LeadWroteCode)
    );

    let no_code = RoleCheck {
        role: Role::Lead,
        diff_adds_code: false,
        builder_model: None,
        verifier_model: None,
    };
    assert_eq!(evaluate_role_check(&no_code), Ok(()));
}

#[test]
fn verifier_self_check_covers_both_directions() {
    let same = RoleCheck {
        role: Role::Verifier,
        diff_adds_code: false,
        builder_model: Some("sonnet"),
        verifier_model: Some("sonnet"),
    };
    assert_eq!(evaluate_role_check(&same), Err(RoleRefusal::SelfVerified));

    let distinct = RoleCheck {
        role: Role::Verifier,
        diff_adds_code: false,
        builder_model: Some("sonnet"),
        verifier_model: Some("codex"),
    };
    assert_eq!(evaluate_role_check(&distinct), Ok(()));
}
