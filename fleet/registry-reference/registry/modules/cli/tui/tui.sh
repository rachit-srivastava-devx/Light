#!/usr/bin/env bash
# tui.sh — launcher for the fleet terminal UI.
#
# WHY A LAUNCHER AND NOT A BARE SHEBANG: `node` on this class of machine is whatever nvm last
# selected. Here it is v10.16.2, which cannot parse `import {x} from "y"` and dies with a bogus
# "SyntaxError: Unexpected token {" on line 4 of a perfectly valid ESM file. That is the exact
# failure the vendored `codex` CLI hits (measured 2026-08-24), so the resolved interpreter is also
# prepended to PATH for every backend this UI spawns. Resolving the runtime is not incidental
# setup; it is the difference between the tool working and the tool being adopted.
#
# Usage: tui.sh [--cwd DIR] [--install] [--print-node] [--help]
# Exit:  0 ok | 2 usage | 3 no usable node runtime found | 4 install could not be verified
set -u

D="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
MIN_MAJOR=20

usage() {
  printf '%s\n' \
    'fleet tui [--cwd DIR]' \
    '  Opens the fleet terminal UI scoped to DIR (default: the current directory).' \
    '  --install     put `fleet` on PATH and register it in ~/.zshrc (idempotent)' \
    '  --print-node  resolve and print the node runtime that would be used, then exit' \
    '  --help        this text'
}

# Report the major version of a candidate, or nothing if it is not a working node.
node_major() {
  local bin="$1" v
  [ -x "$bin" ] || return 1
  v="$("$bin" -v 2>/dev/null)" || return 1
  case "$v" in v[0-9]*) ;; *) return 1 ;; esac
  v="${v#v}"; printf '%s' "${v%%.*}"
}

resolve_node() {
  local cand major
  # An explicit override always wins, and is REFUSED loudly if unusable rather than silently
  # falling through to a different interpreter than the operator asked for.
  if [ -n "${FLEET_NODE:-}" ]; then
    major="$(node_major "$FLEET_NODE")" || {
      printf 'fleet tui: FLEET_NODE=%s is not a working node binary\n' "$FLEET_NODE" >&2; return 3; }
    [ "$major" -ge "$MIN_MAJOR" ] || {
      printf 'fleet tui: FLEET_NODE=%s is node %s; need >= %s\n' "$FLEET_NODE" "$major" "$MIN_MAJOR" >&2; return 3; }
    printf '%s' "$FLEET_NODE"; return 0
  fi
  for cand in /opt/homebrew/bin/node /usr/local/bin/node "$(command -v node 2>/dev/null)" /usr/bin/node; do
    [ -n "$cand" ] || continue
    major="$(node_major "$cand")" || continue
    [ "$major" -ge "$MIN_MAJOR" ] && { printf '%s' "$cand"; return 0; }
  done
  # nvm installs, newest first. `sort -V` is not portable to this bash, so sort numerically on the
  # version components extracted from the directory name.
  if [ -d "$HOME/.nvm/versions/node" ]; then
    while IFS= read -r cand; do
      [ -n "$cand" ] || continue
      major="$(node_major "$cand")" || continue
      [ "$major" -ge "$MIN_MAJOR" ] && { printf '%s' "$cand"; return 0; }
    done < <(ls -1 "$HOME/.nvm/versions/node" 2>/dev/null \
              | sed 's/^v//' | sort -t. -k1,1nr -k2,2nr -k3,3nr | sed "s|^|$HOME/.nvm/versions/node/v|; s|$|/bin/node|")
  fi
  return 3
}

# Install `fleet` as a bare command. Idempotent: re-running replaces the managed block rather than
# appending a second copy, which is how a shell rc accumulates six stale exports over a year.
do_install() {
  local bin="$HOME/.local/bin" rc="$HOME/.zshrc" stamp backup
  mkdir -p "$bin" || { printf 'fleet: cannot create %s\n' "$bin" >&2; return 4; }
  ln -sfn "$D/fleet" "$bin/fleet" || { printf 'fleet: cannot link %s/fleet\n' "$bin" >&2; return 4; }
  printf 'linked  %s -> %s\n' "$bin/fleet" "$D/fleet"

  # Build the intended file first and compare. Re-running an installer must not churn the rc or
  # drop another dated backup each time; three identical backups is noise pretending to be safety.
  local tmp="$rc.fleet.tmp.$$"
  if [ -f "$rc" ]; then
    # Strip the managed block AND the trailing blank lines it leaves behind. Without the second
    # half, each re-run removed 5 lines and appended 6, growing ~/.zshrc by one blank line forever.
    awk '/^# >>> fleet command \(managed by/{skip=1} !skip{print} /^# <<< fleet command <<</{skip=0}' "$rc" \
      | awk '{ if (NF) { for (i=0;i<blank;i++) print ""; blank=0; print } else blank++ }' >"$tmp" \
      || { rm -f "$tmp"; printf 'fleet: could not read %s\n' "$rc" >&2; return 4; }
  else : >"$tmp"; fi
  {
    printf '\n%s\n' '# >>> fleet command (managed by `fleet tui --install`) >>>'
    printf '%s\n' '# Bare `fleet` opens the terminal UI in the current directory.'
    printf '%s\n' '# `fleet <verb>` still runs one verb non-interactively; `fleet help` lists them.'
    printf '%s\n' 'export PATH="$HOME/.local/bin:$PATH"'
    printf '%s\n' '# <<< fleet command <<<'
  } >>"$tmp"
  if [ -f "$rc" ] && cmp -s "$tmp" "$rc"; then
    rm -f "$tmp"; printf 'zshrc   %s already current (unchanged)\n' "$rc"
  else
    if [ -f "$rc" ]; then
      stamp="$(date -u +%Y%m%dT%H%M%SZ)"; backup="$rc.fleet-backup.$stamp"
      cp "$rc" "$backup" || { rm -f "$tmp"; printf 'fleet: cannot back up %s\n' "$rc" >&2; return 4; }
      printf 'backup  %s\n' "$backup"
    fi
    mv "$tmp" "$rc" || { printf 'fleet: could not write %s\n' "$rc" >&2; return 4; }
    printf 'zshrc   %s (managed block added)\n' "$rc"
  fi

  # Verify through a real login shell, not by asserting the file was written. A block appended to a
  # rc file that a later line shadows is exactly the kind of thing that reads as installed and is not.
  local resolved
  # An interactive login shell also emits terminal-integration escape codes (iTerm2 OSC 1337) on
  # this machine; taking the raw tail printed those into the middle of the path. Extract the path.
  resolved="$(zsh -ilc 'command -v fleet' 2>/dev/null | tr -d '\r' | grep -oE '/[^[:cntrl:]]*/fleet$' | tail -1)"
  if [ -n "$resolved" ]; then
    printf 'verified `fleet` resolves to %s in a fresh login shell\n' "$resolved"
    printf '\nOpen a new terminal (or run: exec zsh) and type: fleet\n'
    return 0
  fi
  printf 'fleet: appended to %s but `fleet` still does not resolve in a login shell\n' "$rc" >&2
  printf '       check for a later PATH assignment in %s that drops ~/.local/bin\n' "$rc" >&2
  return 4
}

CWD="$PWD"
while [ $# -gt 0 ]; do
  case "$1" in
    --cwd) [ -n "${2-}" ] || { printf 'fleet tui: --cwd needs a directory\n' >&2; exit 2; }
           CWD="$(cd "$2" 2>/dev/null && pwd)" || { printf 'fleet tui: no such directory: %s\n' "$2" >&2; exit 2; }
           shift 2 ;;
    --install) DO_INSTALL=1; shift ;;
    --print-node) PRINT_NODE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'fleet tui: unknown argument %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

if [ "${DO_INSTALL:-0}" = 1 ]; then do_install; exit $?; fi

NODE="$(resolve_node)" || {
  printf '%s\n' \
    'fleet tui: no node >= 20 found.' \
    "  looked at: \$FLEET_NODE, /opt/homebrew/bin/node, /usr/local/bin/node, \$(command -v node), ~/.nvm/versions/node/*" \
    '  fix: brew install node   (or) nvm install 22   (or) FLEET_NODE=/path/to/node fleet' >&2
  exit 3
}

if [ "${PRINT_NODE:-0}" = 1 ]; then printf '%s %s\n' "$NODE" "$("$NODE" -v)"; exit 0; fi

# Every backend this UI spawns inherits this PATH. codex is a node ESM script whose shebang would
# otherwise pick up nvm's node 10 and abort before printing a single token.
export PATH="$(dirname "$NODE"):$PATH"
export FLEET_ROOT="$D"
export FLEET_TUI_CWD="$CWD"
exec "$NODE" "$D/registry/modules/cli/tui/main.mjs"
