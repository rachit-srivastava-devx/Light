# OrbMac App Shell — visual spec (matches reference screenshot exactly)

Reference: a Codex-desktop-style 3-pane macOS app. This spec is the source of truth for pixel/color/
spacing matching — read it before writing any shell UI code. Do not approximate; if a value can't be
read precisely from the screenshot, pick the closest system-standard value and say so in your report
rather than guessing silently.

## Layout — 3 panes, left and right independently collapsible

```
┌─────────────┬──────────────────────────────────────┬─────────────────┐
│  Sidebar     │  Chat area                            │  Artifacts      │
│  (history)   │  (streaming transcript + input bar)    │  panel          │
│  collapsible │                                        │  collapsible    │
└─────────────┴──────────────────────────────────────┴─────────────────┘
```

### Left sidebar (collapsible, ~260-280pt wide)
- Header row: sidebar-toggle icon, app name ("OrbMac" in place of "Codex") with a small dropdown
  chevron, then search icon and a notification-bell icon right-aligned.
- "New chat" row below: pencil/edit icon + label, with a "+" icon at the far right of the row.
- A scrollable list of folder groups (grey folder icon + name, e.g. project/workspace names),
  each expandable to show indented chat/session items underneath. Some items carry a small trailing
  icon (branch/fork glyph) hinting at a git-linked session — omit that specific affordance for v1,
  a plain chat-history row is enough.
- A muted "Show more" text link when a folder's item list is truncated.
- A "Recents" section pinned near the bottom, showing the most recent session(s); the active/selected
  one is visually highlighted (subtle lighter background, rounded rect) and can show a small live
  waveform/activity icon + a refresh icon when it's the current voice session.
- Footer row, pinned to the very bottom: circular user-avatar (initials), display name, and a
  small download/sync icon at the far right.
- All text in the sidebar is small (~13pt), secondary items in a muted grey, active/primary items in
  off-white.

### Middle: chat area
- Top bar: back/forward chevrons (top-left, only meaningful once there's real navigation history —
  fine to render disabled/inert for v1), the current session's title (e.g. "New voice chat"),
  a "…" more-options icon next to the title. Top-right of this bar: a share/export icon, a list/
  outline icon, and two small window-layout icons that toggle the left/right panels (this is the
  actual collapse control for the two side panels — wire these, don't just decorate them).
- Below the top bar: a transient error/status banner can appear centered (e.g. a rounded pill with a
  warning icon, message text, and a dismiss "×" — used here for a connection-failed message; reuse
  this same banister for any transport error the protocol client surfaces, not just voice-specific
  ones).
- Transcript, scrollable, growing downward:
  - User turns: right-aligned, inside a rounded pill/bubble with a slightly lighter background than
    the canvas.
  - Assistant turns: left-aligned, plain text (no bubble), directly on the canvas background.
  - A turn still streaming in shows a lightweight "Thinking" label with an animated trailing cursor/
    caret before real content starts arriving — replace it with real streamed text token-by-token as
    it comes in (both user voice transcript AND assistant reply must stream, not just appear
    complete).
- A dismissible promo/status banner can sit just above the input bar (in the reference it's a credits
  promo — for v1, repurpose this row for something functionally useful like connection status or omit
  it if there's nothing to say; don't fabricate a fake promo).
- Input bar, pinned to the bottom of the chat area:
  - A "+" attach icon, left-most.
  - A large borderless text field with placeholder text ("Do anything" in the reference — pick
    OrbMac's own placeholder), which is exactly where live voice-transcribed text gets filled in as
    the user talks to the orb — see "Orb trigger" below.
  - A small permission/status pill (e.g. "Full access" with a warning triangle in the reference) —
    repurpose for whatever access/session state is meaningful here, or omit if not applicable.
  - A model/mode selector dropdown.
  - A microphone icon.
  - The **orb** is NOT an inline element inside the input bar's icon row. Per a corrected follow-up
    screenshot, it floats ON TOP of the text box — overlapping/hovering above the input field
    (roughly centered horizontally over it, overlapping its top edge), the way a floating action
    button sits above rather than within a toolbar. Render it as its own layer above the input bar
    (SwiftUI `ZStack`/`.overlay` anchored to the input bar's top edge), not as a row item alongside
    the "+", mic, and model-selector icons — those stay as the reference shows them, unaffected.
    Size ~28-36pt diameter circle. At rest/idle it renders as a plain, mostly-white/light circle (no
    visible gradient motion needed when idle) — the full breathing/gradient/color animation from the
    M4 `OrbView` becomes visible once a state other than idle is active (listening/thinking/etc), same
    preset colors, scaled down to fit this small floating footprint. Clicking it starts a voice
    session with the relay (start_listening); while listening, streamed transcript text fills the
    input field live; the orb's visual state (from the M4 unit) reflects listening/thinking/speaking
    exactly as designed — this is Rule 4's "plugged in" requirement: the orb component must accept an
    external `OrbVisualState` binding and render correctly at this small size without redesigning its
    core drawing logic (if `OrbView`'s current implementation assumes a large canvas and looks wrong/
    broken at ~30pt, that's a real gap to fix in M4, not to route around here).

### Right panel: artifacts (collapsible, ~280-320pt wide)
- "Outputs" section header with a small "+" button; empty-state placeholder text below it
  ("Create a file or site" in the reference — keep the same *shape* of empty state, wording can be
  OrbMac-appropriate).
- A divider, then "Sources" section header with its own "+" button and empty-state placeholder
  ("Attach files or connect apps").
- As the conversation produces artifacts (files, generated content), they render here — for v1 this
  panel can start empty/placeholder-only; wiring real artifact generation is a later unit, but the
  panel's presence, header styling, and collapse behavior must be built now.

## Color palette (dark theme, match closely)

- Canvas / main background: near-black, ~#0A0A0C to #101012.
- Sidebar and right-panel background: one step lighter than canvas, ~#161618 to #1A1A1C.
- Selected/hover row background: a subtle lighter overlay on the panel background, ~#232326, rounded
  corners (~8pt radius).
- Primary text: off-white, ~#EDEDEF.
- Secondary/muted text (placeholders, folder names, timestamps): mid-grey, ~#8A8A90.
- Borders/dividers: very subtle, ~#232326, 1pt hairline.
- Error/warning banner: warm red-orange, background ~#3A1F1C with ~#E8785A text/icon (not a harsh
  alert-red — matches this app's warm error tone elsewhere, e.g. the orb's own error preset uses
  `#c87552` per the orb visual spec — reuse that same warm red-orange family for consistency).
- User-message bubble background: a soft mid-grey-blue, ~#2A2A2E, rounded ~14pt corners.
- Accent/interactive elements (buttons, active icons): off-white on hover/press, muted grey at rest.

## Typography

- System font (SF Pro via the default macOS font — do not embed a custom font).
- Sidebar items / secondary text: ~13pt regular.
- Chat transcript body text: ~14-15pt regular, generous line-height (~1.5).
- Section headers ("Outputs", "Sources", "Recents"): ~12pt, slightly letter-spaced, muted grey,
  uppercase or small-caps styling optional — match the reference's understated weight, not bold.
- Title bar text (session title): ~14pt medium weight.

## Spacing

- Sidebar/panel internal padding: ~16pt horizontal, ~12pt vertical between rows.
- Row height (chat list items): ~32-36pt.
- Input bar: ~52-60pt tall, ~12pt internal padding, ~10pt gaps between its icon elements.
- Panel corner radius where panels float over canvas (if using a floating/inset panel style rather
  than edge-to-edge): ~12pt.

## Behavioral requirements (not just visual)

1. Both side panels collapse/expand independently (via the top-bar toggle icons AND/OR a keyboard
   shortcut) — collapsing must not destroy that panel's state, just hide it.
2. Chat transcript updates via streaming for BOTH the user's live voice transcript (as it arrives
   from the relay's `transcript` frames, partial then final) and the assistant's reply (as `speak`
   text streams back, or as TTS audio implies progress — coordinate with whatever the M2 protocol
   client unit exposes for streaming state).
3. Clicking the orb starts a voice turn; the live partial transcript fills the input text field in
   real time (not just on `is_final: true`) so the user sees what's being heard.
4. The orb component (from unit M4, `OrbView`/`OrbVisualState`) must be usable at this small
   input-bar size without modification to its core drawing logic — if the current implementation
   assumes a large canvas, that's a gap to flag and fix, not silently work around with a scaled-down
   wrapper that changes its actual visual character.

## What this spec does NOT cover (explicitly out of scope for the shell unit)

- Real artifact generation/rendering in the right panel (empty-state only for now).
- The specific git-branch icon affordance on some sidebar rows.
- Pixel-perfect font metrics — "closely matching system-standard equivalents" is the bar, not
  extracting exact point sizes from the screenshot via a color-picker-style tool no one has here.
