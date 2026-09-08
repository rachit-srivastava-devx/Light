//! `Style::detect` must never emit ANSI when `NO_COLOR` is set, regardless of what a TTY probe
//! would otherwise say -- this is the property the brief calls out explicitly ("assert NO ANSI
//! bytes are emitted when NO_COLOR=1 is set"). `Style::new` (the pure constructor) is exercised
//! by `renderer_tests.rs`; this file is only about the impure `detect` env-reading path.

use super::Style;

#[test]
fn no_color_env_forces_plain_regardless_of_flag() {
    // SAFETY: test-only, single-threaded within this process's env mutation window.
    std::env::set_var("NO_COLOR", "1");
    let style = Style::detect();
    std::env::remove_var("NO_COLOR");
    assert!(!style.color, "NO_COLOR=1 must force plain output even if stderr were a TTY");
}

#[test]
fn term_dumb_forces_plain() {
    std::env::set_var("TERM", "dumb");
    let style = Style::detect();
    std::env::remove_var("TERM");
    assert!(!style.color, "TERM=dumb must force plain output");
}

#[test]
fn plain_style_paint_emits_no_ansi_bytes() {
    let style = Style::new(false);
    let painted = style.paint(super::RED, "boom");
    assert_eq!(painted, "boom");
    assert!(!painted.contains('\x1b'), "plain Style must never emit an ESC byte: {painted:?}");
}
