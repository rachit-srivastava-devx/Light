// ui.mjs — terminal rendering: a scrollback transcript above a sticky, redrawn input region.
//
// SHAPE, taken from a capture of `claude`'s own TUI at 100x34 (see README): a welcome box printed
// once into scrollback, transcript lines appended below it, then a pinned block at the bottom made
// of an optional completion popup, a right-aligned hint, a rule, the prompt line, a rule, and a
// mode footer.
//
// THE ONE INVARIANT THAT MAKES IT NOT CORRUPT: every line of the sticky block is truncated to the
// terminal width before it is written. A sticky line that wraps occupies two rows while the redraw
// accounting believes it occupies one, and from then on every repaint eats a row of transcript.
// Scrollback lines are free to wrap — they scroll away and are never repainted.

const RE_ANSI = /\x1b\[[0-9;]*m/g;

export const hasColor = () => process.stdout.isTTY && !process.env.NO_COLOR && process.env.TERM !== 'dumb';

const C = (open, close) => t => (hasColor() ? `\x1b[${open}m${t}\x1b[${close}m` : String(t));
export const c = {
  dim: C(2, 22), bold: C(1, 22), italic: C(3, 23), inverse: C(7, 27),
  red: C(31, 39), green: C(32, 39), yellow: C(33, 39), blue: C(34, 39),
  magenta: C(35, 39), cyan: C(36, 39), white: C(37, 39), grey: C(90, 39),
};

/** Visible width, ignoring SGR sequences. Wide (CJK/emoji) glyphs count as two columns. */
export function width(s) {
  let n = 0;
  for (const ch of String(s).replace(RE_ANSI, '')) {
    const cp = ch.codePointAt(0);
    if (cp === 0x200d || (cp >= 0xfe00 && cp <= 0xfe0f)) continue;      // ZWJ / variation selectors
    n += (cp >= 0x1100 && (cp <= 0x115f || cp === 0x2329 || cp === 0x232a ||
      (cp >= 0x2e80 && cp <= 0xa4cf && cp !== 0x303f) || (cp >= 0xac00 && cp <= 0xd7a3) ||
      (cp >= 0xf900 && cp <= 0xfaff) || (cp >= 0xfe30 && cp <= 0xfe6f) ||
      (cp >= 0xff00 && cp <= 0xff60) || (cp >= 0xffe0 && cp <= 0xffe6) ||
      (cp >= 0x1f300 && cp <= 0x1f64f) || (cp >= 0x1f900 && cp <= 0x1f9ff))) ? 2 : 1;
  }
  return n;
}

/** Truncate to `max` visible columns, preserving SGR sequences and appending an ellipsis. */
export function truncate(s, max) {
  if (max <= 0) return '';
  if (width(s) <= max) return s;
  let out = '', n = 0, i = 0; const str = String(s);
  while (i < str.length) {
    if (str[i] === '\x1b') { const m = /^\x1b\[[0-9;]*m/.exec(str.slice(i)); if (m) { out += m[0]; i += m[0].length; continue; } }
    const ch = String.fromCodePoint(str.codePointAt(i));
    const w = width(ch);
    if (n + w > max - 1) break;
    out += ch; n += w; i += ch.length;
  }
  return out + (hasColor() ? '\x1b[0m…' : '…');
}

export const pad = (s, n) => s + ' '.repeat(Math.max(0, n - width(s)));

/** Soft-wrap a paragraph to `max` columns on word boundaries. */
export function wrap(text, max) {
  const lines = [];
  for (const para of String(text).split('\n')) {
    if (!para) { lines.push(''); continue; }
    let line = '';
    for (const word of para.split(/(\s+)/)) {
      if (!word) continue;
      if (width(line) + width(word) > max && line.trim()) { lines.push(line.replace(/\s+$/, '')); line = word.replace(/^\s+/, ''); }
      else line += word;
      while (width(line) > max) { lines.push(truncate(line, max)); line = ''; }
    }
    if (line.trim() || !lines.length) lines.push(line.replace(/\s+$/, ''));
  }
  return lines;
}

/**
 * The pinned bottom region plus a scrollback writer.
 * `setSticky(lines)` replaces the pinned block; `emit(text)` appends above it.
 */
export class Screen {
  constructor(out = process.stdout) {
    this.out = out;
    this.sticky = [];
    this.height = 0;          // rows the sticky block currently occupies on screen
    // Which row of the block the terminal cursor is actually sitting on. #paint() moves the cursor
    // UP to the caret, so the next #clear() cannot assume it is on the last row — assuming that
    // over-rewinds by (height - 1 - caretRow) rows and wipes that much transcript on every repaint.
    this.cursorRow = 0;
    this.caret = null;        // {line, col} within the sticky block, or null to park at the end
    this.closed = false;
  }
  get cols() { return Math.max(20, this.out.columns || 80); }
  get rows() { return Math.max(8, this.out.rows || 24); }

  #clear() {
    if (!this.height) return;
    // Rewind from wherever the cursor actually is to column 0 of the block's first row, then wipe
    // everything below it.
    const up = this.cursorRow;
    this.out.write(`${up > 0 ? `\x1b[${up}A` : ''}\r\x1b[0J`);
    this.height = 0; this.cursorRow = 0;
  }

  #paint() {
    if (this.closed || !this.sticky.length) { this.height = 0; this.cursorRow = 0; return; }
    const lines = this.sticky.map(l => truncate(l, this.cols));
    this.out.write('\x1b[?25l' + lines.join('\n'));
    this.height = lines.length;
    this.cursorRow = this.height - 1;
    if (this.caret) {
      const row = Math.min(this.caret.line, this.height - 1);
      const up = this.height - 1 - row;
      this.out.write(`${up > 0 ? `\x1b[${up}A` : ''}\r${this.caret.col > 0 ? `\x1b[${this.caret.col}C` : ''}\x1b[?25h`);
      this.cursorRow = row;
    } else this.out.write('\x1b[?25h');
  }

  /** Forget the on-screen block without touching it — after the caller has cleared the screen. */
  reset() { this.height = 0; this.cursorRow = 0; }

  setSticky(lines, caret = null) { this.#clear(); this.sticky = lines; this.caret = caret; this.#paint(); }
  refresh() { this.#clear(); this.#paint(); }

  /** Append to scrollback above the sticky block. Accepts a string or array of lines. */
  emit(text) {
    const body = Array.isArray(text) ? text.join('\n') : String(text);
    this.#clear();
    this.out.write(body + '\n');
    this.#paint();
  }

  /** Release the terminal: drop the sticky block and show the cursor. */
  close() { this.#clear(); this.closed = true; this.sticky = []; this.out.write('\x1b[?25h'); }
}

/** A rounded box with a title, as the welcome banner. Returns lines. */
export function box(title, rows, cols) {
  const inner = cols - 2;
  const head = `╭─── ${title} ` + '─'.repeat(Math.max(0, inner - width(title) - 5)) + '╮';
  const foot = '╰' + '─'.repeat(inner) + '╯';
  return [head, ...rows.map(r => '│' + pad(truncate(r, inner), inner) + '│'), foot];
}

/** Two-column completion popup: name, then description, selected row highlighted. */
export function popup(items, selected, cols, max = 6) {
  if (!items.length) return [];
  // Keep the selected row on screen when the list is longer than the window.
  const start = Math.max(0, Math.min(selected - Math.floor(max / 2), items.length - max));
  const win = items.slice(Math.max(0, start), Math.max(0, start) + max);
  const nameW = Math.min(30, Math.max(12, ...win.map(i => width(i.name) + 2)));
  const descW = Math.max(10, cols - nameW - 4);
  const lines = win.map((it, i) => {
    const on = Math.max(0, start) + i === selected;
    const name = pad(truncate(it.name, nameW), nameW);
    const desc = truncate(it.description || '', descW);
    const row = `  ${name}  ${desc}`;
    return on ? c.inverse(pad(row, cols - 1)) : c.dim(row);
  });
  if (items.length > win.length) lines.push(c.grey(`  ${Math.max(0, start) + win.length}/${items.length} — ↑↓ to scroll`));
  return lines;
}

/**
 * Split a raw pty chunk into individual key tokens, keeping CSI (`ESC [ … final`) and SS3
 * (`ESC O letter`) escape sequences whole.
 *
 * A pty read is not a keystroke: typing a line and pressing return can arrive as ONE chunk
 * ("hello\r"). Treating the chunk as an atomic key means return never submits — measured against
 * this UI, not theorised.
 */
export function tokenize(chunk) {
  const keys = [];
  for (let i = 0; i < chunk.length;) {
    if (chunk[i] === '\x1b') {
      const m = /^\x1b(?:\[[0-9;?]*[ -\/]*[@-~]|O[A-Za-z]|.)/.exec(chunk.slice(i));
      if (m) { keys.push(m[0]); i += m[0].length; continue; }
      keys.push('\x1b'); i += 1; continue;
    }
    const ch = String.fromCodePoint(chunk.codePointAt(i));
    keys.push(ch); i += ch.length;
  }
  return keys;
}

export const SPINNER = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
