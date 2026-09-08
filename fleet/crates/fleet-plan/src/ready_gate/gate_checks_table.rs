//! The 14-entry `CHECKS` table pairing each check id with its fn. Verbatim from
//! `lld_ready.rs:85-100`.

use super::gate_checks_a::{check_c1_open, check_c2_owner, check_c3_acc_ground, check_c3_acc_nontaut, check_c3_acc_parse, check_r17_deriv};
use super::gate_checks_b::{check_c12_store, check_r19_absolute, check_r21_alts, check_r21_fail};
use super::gate_checks_c::{check_c12_deps, check_iface, check_reg_verdict, check_shape};
use super::gate_types::Check;

pub const CHECKS: &[Check; 14] = &[
    ("C1-OPEN", check_c1_open),
    ("C2-OWNER", check_c2_owner),
    ("C3-ACC-PARSE", check_c3_acc_parse),
    ("C3-ACC-GROUND", check_c3_acc_ground),
    ("C3-ACC-NONTAUT", check_c3_acc_nontaut),
    ("R17-DERIV", check_r17_deriv),
    ("R19-ABSOLUTE", check_r19_absolute),
    ("R21-ALTS", check_r21_alts),
    ("R21-FAIL", check_r21_fail),
    ("C12-STORE", check_c12_store),
    ("C12-DEPS", check_c12_deps),
    ("REG-VERDICT", check_reg_verdict),
    ("IFACE", check_iface),
    ("SHAPE", check_shape),
];
