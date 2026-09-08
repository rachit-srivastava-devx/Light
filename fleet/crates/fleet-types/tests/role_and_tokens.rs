//! Unit-level coverage for `Role` and `Tokens` (BLUEPRINT.md §9), run as integration tests so
//! `src/role.rs`/`src/tokens.rs` stay thin per the 80-line rule.

use fleet_types::{Role, Tokens, TokensOverflow, UnknownRole};

#[test]
fn role_parse_matches_all_five_and_only_five() {
    for role in Role::ALL {
        assert_eq!(Role::parse(role.name()), Ok(role));
    }
    for bad in ["", "Lead", "LEAD", "lead ", "unknown"] {
        assert_eq!(Role::parse(bad), Err(UnknownRole(bad.to_string())));
    }
}

#[test]
fn checked_add_overflows_instead_of_wrapping() {
    assert_eq!(Tokens::ZERO.checked_add(Tokens::ZERO), Ok(Tokens::ZERO));
    assert_eq!(Tokens::new(u64::MAX).checked_add(Tokens::new(1)), Err(TokensOverflow));
}

#[test]
fn checked_sub_underflows_instead_of_wrapping() {
    assert_eq!(Tokens::ZERO.checked_sub(Tokens::new(1)), Err(TokensOverflow));
    assert_eq!(Tokens::new(5).checked_sub(Tokens::new(2)), Ok(Tokens::new(3)));
}
