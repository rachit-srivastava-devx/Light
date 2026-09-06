# `fleet` — the terminal UI

Type `fleet` in any directory. It opens a chat surface scoped to that directory, discovers the
skills / slash commands / MCP servers available there, and puts fleet's two harnesses on your task.

```bash
fleet
```

`fleet <verb>` still runs one verb non-interactively, and `fleet help` still prints the table.
Only the **bare** invocation opens the UI.

## Install

```bash
fleet tui --install
```

Symlinks `~/.local/bin/fleet` → this repo's `fleet` and registers a managed block in `~/.zshrc`.
Idempotent: re-running reports `already current (unchanged)` and makes no second backup. It
verifies by resolving `fleet` in a real login shell, not by asserting the file was written.

## Modes — shift+tab cycles

| mode | what happens |
|---|---|
| `solo` | one lead backend answers, streamed token by token, tool calls shown live |
| `fanout` | **both** harnesses on the same task, role-split: one builds, one verifies read-only |
| `fleet` | hands off to `fleet run` — intake, clarifying questions, SOW, role SDLC, receipts |

### Why fan-out is role-split rather than duplicated

Two agents with write access to one tree collide, and this repo has already paid for it: a
correctly-scoped dispatch was failed with exit 6 naming three files a *different* concurrent
dispatch had written in the same wall-clock window (`docs/reports/CYCLE-PROOF.md`, "the stage 5c
incident"). So the lanes take the builder/verifier split the role registry already declares. Both
models are on the task; only one holds the pen, and the verify lane is forced read-only even when
the turn is write-enabled.

If only one lane's backend is usable, the split **collapses to solo and says so** — the other lane
settles as `skipped` carrying its reason. A half fan-out never reads as a whole one.

## Keys

| key | |
|---|---|
| `/` | command popup — builtins, workspace commands, and all 38 fleet verbs |
| `@` | file completion from the workspace |
| `↑` `↓` | history (or move within the popup) |
| `shift+tab` | cycle mode |
| `esc` | close the popup · interrupt a running turn · clear the line |
| `ctrl+c` ×2, `ctrl+d` | exit |

## What it discovers, and from where

Nearest-first: **workspace** → **fleet repo** → **`$HOME`**. A shadowed definition is recorded and
shown by `/skills`, never silently dropped.

- **skills** — `<root>/.claude/skills/*/SKILL.md`. Matched against each message by trigger phrase
  and word overlap; the matches and their scores are printed before the turn, so a wrong match is
  visible rather than silently steering the run. Matched skill bodies are inlined into the prompt.
- **commands** — `<root>/.claude/commands/*.md`, run as `/<name>`.
- **verbs** — parsed out of the `fleet` entrypoint's own help text, so the list cannot drift.
- **MCP** — `.mcp.json`, `.claude/mcp.json`, `~/.claude.json`. Each is reported `available` /
  `absent` / `declared`, never assumed present.
- **house rules** — `PRINCIPLES.md` (this repo), plus the workspace's `CLAUDE.md` / `AGENTS.md`.

## Backends are probed, not assumed

On start, each harness gets a real one-word round trip. Until it answers, its status is `probing`;
it is only `ready` once it has actually replied. Two orchestration tools in this repo's failure
corpus were adopted and defended for a full day without either having ever run — a `command -v`
check would have called both of them ready.

Known states on this machine (measured 2026-08-24):

| harness | status | note |
|---|---|---|
| `codex` | ready | needs node ≥ 20 on PATH; the launcher guarantees it |
| `claude` | unavailable | `Your organization has disabled Claude subscription access for Claude Code` — set `ANTHROPIC_API_KEY`, or have the org admin re-enable it |

## The node problem this launcher exists to solve

`node` on this machine resolves to nvm's **v10.16.2**, which cannot parse ESM and dies with a bogus
`SyntaxError: Unexpected token {` on line 4 of a valid file. That is also what breaks the vendored
`codex` CLI. `tui.sh` resolves a node ≥ 20 (`$FLEET_NODE` → Homebrew → `/usr/local` → `PATH` →
newest nvm install) and **prepends it to PATH for every backend it spawns**. Check with:

```bash
fleet tui --print-node
```

## Layout

| file | |
|---|---|
| `tui.sh` | launcher: resolves node, installs to PATH, execs the runner |
| `main.mjs` | session loop, key handling, command dispatch, transcript |
| `ui.mjs` | rendering primitives: sticky region, box, popup, wrap, key tokenizer |
| `discover.mjs` | skills / commands / MCP / verbs — filesystem only, executes nothing it finds |
| `backends.mjs` | probe + spawn + stream for both harnesses |
| `session.mjs` | prompt assembly, role briefs, fan-out |

Node stdlib only, no dependencies — the same constraint the console data plane holds.

## Two invariants worth knowing before you change this

1. **Every sticky line is truncated to the terminal width.** A sticky line that wraps occupies two
   rows while the redraw accounting believes it occupies one, and from then on every repaint eats a
   row of transcript. Scrollback lines may wrap freely; they scroll away and are never repainted.
2. **A pty read is not a keystroke.** Typing a line and pressing return can arrive as one chunk
   (`"hello\r"`); treating a chunk as an atomic key means return never submits. Input is split into
   key tokens, and real pastes are delimited by bracketed-paste markers.

Both are regression-tested — the first by replaying this UI's own escape sequences through a
miniature terminal emulator, because the bug is invisible to any assertion on the strings alone.

## Tests

```bash
bats tests/small/tui.bats
```

Hermetic: no network, no real backend, temp dir only. Both harnesses are replaced by fixture stubs
(`FLEET_CODEX_BIN` / `FLEET_CLAUDE_BIN`, the idiom `dispatch.sh` already uses), so the real
spawn/stream/parse/timeout pipeline runs at zero token cost. The stub event shapes are copies of
real captures, not invented.

## What this does NOT do

- **No receipts.** Conversational turns are not receipts and nothing here writes
  `ledger/RECEIPTS.jsonl`. Use `fleet` mode to reach the gated, receipt-writing path.
- **No push, merge, or PR.** That gate stays with you.
- **No per-call USD.** The codex backend emits no cost field; `/cost` reports tokens and says why
  the dollar figure is absent rather than inventing one.
- **No multi-turn backend sessions.** Each turn is one-shot; conversation is replayed as context.
