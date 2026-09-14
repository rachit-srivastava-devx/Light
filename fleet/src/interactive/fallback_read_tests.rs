use super::*;
use std::io::Cursor;

fn read(bytes: &[u8]) -> ReadOutcome {
    read_fallback_line(&mut Cursor::new(bytes))
}

#[test]
fn empty_stream_is_eof() {
    assert_eq!(read(b""), ReadOutcome::Exit);
}

#[test]
fn ctrl_d_on_an_empty_line_exits() {
    assert_eq!(read(&[0x04]), ReadOutcome::Exit);
}

#[test]
fn plain_line_submits_trimmed_of_newline() {
    assert_eq!(read(b"hello\n"), ReadOutcome::Submit("hello".into()));
}

#[test]
fn crlf_line_strips_the_carriage_return_too() {
    assert_eq!(read(b"hello\r\n"), ReadOutcome::Submit("hello".into()));
}

#[test]
fn a_line_with_no_trailing_newline_before_eof_still_submits() {
    // The underlying stream just ends (Ok(0)) before a '\n' ever arrives -- e.g. a pipe closed
    // mid-line. Whatever was typed should not be silently dropped.
    assert_eq!(read(b"partial"), ReadOutcome::Submit("partial".into()));
}

#[test]
fn ctrl_d_mid_line_is_treated_as_a_literal_byte_not_an_exit() {
    // Only an empty line treats Ctrl-D as "the user wants to quit" (matching readline/bash);
    // once something has been typed, a stray 0x04 byte is just data.
    assert_eq!(read(b"ab\x04cd\n"), ReadOutcome::Submit("ab\u{4}cd".into()));
}

#[test]
fn invalid_utf8_exits_instead_of_panicking() {
    assert_eq!(read(&[0xFF, 0xFE, b'\n']), ReadOutcome::Exit);
}

#[test]
fn a_runaway_line_past_the_cap_exits_instead_of_growing_forever() {
    let mut input = vec![b'a'; MAX_LINE_BYTES + 10];
    input.push(b'\n');
    assert_eq!(read(&input), ReadOutcome::Exit);
}

#[test]
fn multiple_lines_only_the_first_is_consumed() {
    // read_fallback_line is called once per prompt; a second call would read the next line.
    let mut cursor = Cursor::new(b"first\nsecond\n".to_vec());
    assert_eq!(
        read_fallback_line(&mut cursor),
        ReadOutcome::Submit("first".into())
    );
    assert_eq!(
        read_fallback_line(&mut cursor),
        ReadOutcome::Submit("second".into())
    );
}
