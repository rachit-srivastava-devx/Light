//! The one side-effecting seam in the human render path: write a rendered `Event` to stderr.
//! Split from `renderer::render` (pure, unit-testable) so no test needs to capture a real stream
//! to assert on rendered text.

use super::render_event::Event;
use super::renderer::render;
use super::style::Style;

pub fn emit(event: &Event, style: &Style) {
    eprintln!("{}", render(event, style));
}
