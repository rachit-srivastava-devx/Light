use super::indent;

#[test]
fn streamed_lines_keep_the_assistant_response_visually_grouped() {
    assert_eq!(indent("one\ntwo"), "one\n  two");
}
