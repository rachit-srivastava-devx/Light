#!/usr/bin/env bash
# PostToolUse hook (Edit|Write|MultiEdit): fast, fail-safe, per-file correctness feedback.
#
# Compiles/lints ONLY the file that just changed, using tools the repo already has. On failure it
# prints concise errors to stderr and exits 2 — a non-blocking PostToolUse error that feeds the
# message straight back to Claude (the edit already applied), so the model fixes it on the next turn
# without the user asking. This is the "test everything programmatically" lever from the L8 contract.
#
# It NO-OPS silently for non-code files, or when the relevant tool is absent, so it never adds noise
# to a docs/config/chat session. It also drops a marker so the Stop gate knows code changed this
# session. Disable per-session with SKIP_L8_VERIFY=1.
set -u
[ -n "${SKIP_L8_VERIFY:-}" ] && exit 0

INPUT="$(cat)"
FILE="$(printf '%s' "$INPUT" | jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' 2>/dev/null)"
[ -z "$FILE" ] && exit 0
[ -f "$FILE" ] || exit 0

# --- record that code changed this session (consumed by l8-stop-gate.sh) ---
mark_code() {
  local root="${CLAUDE_PROJECT_DIR:-$PWD}"
  mkdir -p "$root/.fleet" 2>/dev/null && : > "$root/.fleet/.session-touched-code" 2>/dev/null || true
}

# Walk up from $1 looking for a directory containing $2 (max 8 levels). Echoes the dir or nothing.
find_up() {
  local dir; dir="$(cd "$(dirname "$1")" 2>/dev/null && pwd)" || return 0
  local n=0
  while [ -n "$dir" ] && [ "$n" -lt 8 ]; do
    [ -e "$dir/$2" ] && { printf '%s\n' "$dir"; return 0; }
    [ "$dir" = "/" ] && break
    dir="$(dirname "$dir")"; n=$((n+1))
  done
}

fail() { echo "⟦L8 auto-verify⟧ $1" >&2; shift; printf '%s\n' "$@" | tail -15 >&2; exit 2; }

case "$FILE" in
  *.py)
    mark_code
    if ! out="$(python3 -m py_compile "$FILE" 2>&1)"; then
      fail "$FILE does not compile — fix before continuing:" "$out"
    fi
    if command -v ruff >/dev/null 2>&1; then
      if ! out="$(ruff check --quiet "$FILE" 2>&1)"; then
        fail "ruff found issues in $FILE:" "$out"
      fi
    fi
    ;;
  *.ts|*.tsx|*.js|*.jsx|*.mjs|*.cjs)
    mark_code
    # Only lint if the repo ships a local eslint — avoids false "not found" noise.
    repo="$(find_up "$FILE" node_modules)"
    if [ -n "$repo" ] && [ -x "$repo/node_modules/.bin/eslint" ]; then
      if ! out="$(cd "$repo" && timeout 25 ./node_modules/.bin/eslint "$FILE" 2>&1)"; then
        # eslint exits 1 on lint errors, 2 on config errors; both are worth surfacing.
        fail "eslint found issues in $FILE:" "$out"
      fi
    fi
    ;;
  *.go)
    mark_code
    if command -v gofmt >/dev/null 2>&1; then
      out="$(gofmt -l "$FILE" 2>&1)"
      [ -n "$out" ] && echo "⟦L8 auto-verify⟧ $FILE is not gofmt-clean — run: gofmt -w $FILE" >&2
    fi
    ;;
  *.sh|*.bash)
    if command -v shellcheck >/dev/null 2>&1; then
      if ! out="$(shellcheck -S error "$FILE" 2>&1)"; then
        fail "shellcheck found errors in $FILE:" "$out"
      fi
    fi
    ;;
  *)
    exit 0
    ;;
esac
exit 0
