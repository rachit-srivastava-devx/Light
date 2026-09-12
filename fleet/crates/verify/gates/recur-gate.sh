#!/usr/bin/env bash
# keel-gate::recur — evaluates promoted lesson signatures against a DIFF, not the whole tree
# (11-THREE-MEMORY-LAYERS.md §4.2/§4.4: tree-wide would fail every pre-existing instance on every
# change and get muted within a week; a diff-scoped gate stays alive).
#
# Exit: 0 clean · 3 environment fault (no git / no diff producible) · 6 a promoted signature matched
#       (refuse, name lesson id + file:line) · 7 refusal (bad invocation).
#
# Usage: bin/recur-gate.sh [--diff <file>|-]     # default: git diff --cached, or git diff HEAD if
#                                                 # nothing staged
#        bin/recur-gate.sh --selftest            # proves both directions on synthetic fixtures
#
# Portable on purpose: no PCRE, no lookbehind, no GNU-only grep flags. This is invoked by
# .githooks/pre-commit on whatever `grep`/`awk` is actually on the operator's PATH — not the
# harness's own ugrep-wrapped Bash tool used to develop it. Procedural line-by-line logic in awk,
# not a single clever regex, so it behaves the same under BSD and GNU userlands.
# NOTE (fleet-verify relocation, S1 fix): this copy lives directly at the gates root, but the
# `git diff` it needs must run against the TARGET repo (`--repo`), never the gates root -- it used
# to unconditionally `cd "$(dirname "$0")"` (the gates root) before running any `git` command,
# which only worked when a gates-root override happened to point at a real fleet/ checkout.
# `REPO` is the cwd this script was invoked with (fleet's runner sets it to the `--repo` target);
# `cd` into it explicitly so every bare `git diff`/`git rev-parse` below is unambiguous.
set -u
REPO="${FLEET_TARGET_REPO:-$(pwd)}"
cd "$REPO" || { echo "recur-gate: repo $REPO not accessible"; exit 3; }

LESSONS_DIR="memory/lessons"

check_diff() {
  # $1 = path to a unified diff (or - for stdin)
  #
  # KNOWN OPEN GAPS (2nd independent review, 2026-08-28, of the hardened version below — found,
  # not fixed, disclosed rather than rushed): (1) a real pipe earlier on the SAME physical line as
  # a trailing case-pattern (`echo hi | tee log; case "$x" in pass|warn)`) is wrongly suppressed —
  # is_case_pattern()'s whole-line check blinds is_pipe() to the genuine pipe before it. (2) an
  # escaped quote (`grep "a\"b" file | tail -1`) desyncs strip_quotes()'s naive state tracking and
  # can swallow a real trailing pipe. Both are narrow, uncommon shell idioms, not the common case
  # this gate was hardened against; fixing (2) properly needs real shell-escape parsing, which is a
  # materially bigger undertaking than this gate's naive-lexer scope. Left open on purpose rather
  # than rushed under time pressure — this is why E1 stays Candidate, not Enforced.
  #
  # Hardened 2026-08-28 after an independent adversarial review (see
  # memory/lessons/E1-pipe-exit-code.json) found 6 false negatives and 3 false positives in the
  # first version. All 9 of the reviewer's constructed cases are now regression-tested in
  # selftest() below, alongside the original 2. Root causes fixed:
  #   - adjacency was "the literal previous diff line", broken by a blank line or a comment
  #     between the pipe and the $? read -- now tracked as "the last CODE line", skipping
  #     comment-only and blank lines entirely (neither updates nor resets it).
  #   - the pipefail guard was a bare substring match on "pipefail" -- matched a comment merely
  #     mentioning the word, and never distinguished `set -o pipefail` (on) from `set +o pipefail`
  #     (back off). Now a real `set` statement match with on/off tracking.
  #   - is_pipe() scanned raw line text -- a `|` inside a quoted string (regex alternation) or a
  #     `case ... in pat1|pat2)` pattern both look identical to a command pipe to a naive scanner.
  #     Now quotes are stripped before scanning, and a line ending in `)` right after the `|`
  #     (the case-pattern shape) is excluded.
  local diff_file="$1"
  awk '
    function strip_quotes(s,    out, i, n, c, q) {
      # Naive (no backslash-escape handling) removal of quoted spans, so a "|" or "#" inside a
      # string does not look like a real pipe or comment. Good enough for the shapes this gate
      # actually needs to tell apart; not a shell parser.
      out = ""; n = length(s); q = ""
      for (i = 1; i <= n; i++) {
        c = substr(s, i, 1)
        if (q == "") {
          if (c == "\x27" || c == "\"") { q = c; continue }
          out = out c
        } else {
          if (c == q) q = ""
        }
      }
      return out
    }
    function is_case_pattern(stripped) {
      # `pat1|pat2)` -- a `|` immediately followed only by more pattern text and a trailing `)`.
      # A real command pipe essentially never ends a line with a bare `)`.
      return (stripped ~ /\|[^|]*\)[ \t]*$/)
    }
    function is_pipe(line,    stripped, n, c) {
      # true if line contains a command pipe "|" that is not "||", a quoted "|", or a case pattern
      stripped = strip_quotes(line)
      if (is_case_pattern(stripped)) return 0
      n = length(stripped)
      for (i = 1; i <= n; i++) {
        c = substr(stripped, i, 1)
        if (c == "|") {
          if (substr(stripped, i+1, 1) == "|") { i++; continue }  # skip "||"
          if (substr(stripped, i-1, 1) == "|") { continue }        # already consumed
          return 1
        }
      }
      return 0
    }
    function is_bare_qmark(line) {
      # true if line references $? and it is not ${PIPESTATUS[0]} / ${PIPESTATUS[-1]} readback
      if (line !~ /\$\?/) return 0
      if (line ~ /PIPESTATUS\[0\]/ || line ~ /PIPESTATUS\[-1\]/) return 0
      return 1
    }
    function is_noise(stripped) {
      # blank, or a comment (a "#" as the first non-blank character once quotes are stripped)
      return (stripped ~ /^[ \t]*$/ || stripped ~ /^[ \t]*#/)
    }
    function pipefail_toggle(stripped,    on) {
      # 0 = no change, 1 = turns pipefail ON, -1 = turns it back OFF. Requires a real `set`
      # statement, not a comment or a string that merely mentions the word.
      if (stripped !~ /^[ \t]*set[ \t]+/) return 0
      if (stripped ~ /\+[a-zA-Z]*o[ \t]+pipefail/ || stripped ~ /\+o[a-zA-Z]*[ \t]+pipefail/) return -1
      if (stripped ~ /-[a-zA-Z]*o[a-zA-Z]*[ \t]+pipefail/) return 1
      return 0
    }
    BEGIN { infile=""; is_sh=0; pipefail_seen=0; prev=""; prev_new=0; flagged=0; checked=0 }
    /^\+\+\+ b\// {
      infile = substr($0, 7)
      sub(/\t.*/, "", infile)   # raw `diff -u` (not git diff) appends a tab + timestamp here
      is_sh = (infile ~ /\.sh$/)
      pipefail_seen = 0
      prev = ""
      prev_new = 0
      next
    }
    /^\+\+\+ / { next }
    /^@@/ { prev = ""; prev_new = 0; next }
    # A surviving line (added "+" or unchanged context " ") is part of the final file and can
    # participate in the pipe -> $? adjacency check. A removed "-" line is gone from the final
    # file and must not be treated as adjacent to anything.
    /^[+ ]/ {
      if (!is_sh) next
      is_new = ($0 ~ /^\+/)
      line = substr($0, 2)
      if (is_new) checked++
      stripped = strip_quotes(line)
      if (is_noise(stripped)) next   # comment/blank: invisible, does not touch prev
      toggle = pipefail_toggle(stripped)
      if (toggle != 0) pipefail_seen = (toggle > 0)
      # Only flag when THIS diff is what creates the hazard: either the pipe line or the $?
      # line itself is newly added. Two unchanged context lines forming the pattern predate
      # this change and belong to a separate full-tree sweep (11-THREE-MEMORY-LAYERS.md §4.4),
      # not this diff-scoped gate.
      if (is_bare_qmark(line) && !pipefail_seen && (is_pipe(prev) || is_pipe(line))) {
        if (is_new || prev_new) {
          flagged++
          printf("RECUR E1 %s: bare $? follows a pipe, no pipefail seen yet in this diff\n  prev: %s\n  line: %s\n", infile, prev, line) > "/dev/stderr"
        }
      }
      prev = line
      prev_new = is_new
      next
    }
    /^-/ { next }
    { prev = "" }
    END {
      # STDOUT, not /dev/stderr. `parsers::recur` reads stdout only, so writing the published
      # denominator to stderr meant this gate reported `Unparseable` on every single run since it
      # was written -- it never published a count at all. Every peer gate (semgrep, trivy,
      # detector-integrity, policy) prints its denominator line to stdout; this was the outlier.
      printf("recur-gate: checked=%d flagged=%d (signatures=1: E1)\n", checked, flagged)
      exit (flagged > 0 ? 6 : 0)
    }
  ' "$diff_file"
}

selftest() {
  # Cases 3-8 are the independent adversarial review's constructed evasions/false-positives
  # (2026-08-28, see memory/lessons/E1-pipe-exit-code.json) — kept as permanent regression
  # fixtures, not just a one-time finding. Two of the reviewer's findings are DELIBERATELY not
  # here, and not fixed: a backslash-continued multi-line pipe (`cmd | \` / `  tail -1` / `$?`)
  # still evades this gate, and the PIPESTATUS-wrong-index concern turned out not to reproduce
  # (a line with `PIPESTATUS[1]` and no literal `$?` was never going to match is_bare_qmark in
  # the first place). Both are said here, not hidden.
  local tmp rc p f name expect diff_body
  tmp="$(mktemp -d)" || { echo "recur-gate: mktemp failed"; return 3; }
  p=0; f=0

  run_case() {
    # $1=name $2=expect(bad|good) $3=diff body (heredoc via caller)
    name="$1"; expect="$2"
    printf '%s\n' "$3" > "$tmp/case.diff"
    check_diff "$tmp/case.diff" >/dev/null 2>"$tmp/case.log"; rc=$?
    if [ "$expect" = bad ]; then
      if [ "$rc" -eq 6 ]; then echo "  ok   $name"; p=$((p+1))
      else echo "  FAIL $name: expected exit 6 (flag), got $rc — false negative"; f=$((f+1)); fi
    else
      if [ "$rc" -eq 0 ]; then echo "  ok   $name"; p=$((p+1))
      else echo "  FAIL $name: expected exit 0 (clean), got $rc — false positive"; cat "$tmp/case.log"; f=$((f+1)); fi
    fi
  }

  run_case "E1 bad: pipe then bare \$? next line" bad \
'--- a/script.sh
+++ b/script.sh
@@ -1,2 +1,2 @@
+grep pattern file.txt | tail -1
+RUFF_EXIT=$?'

  run_case "E1 good: PIPESTATUS[0] / pipefail-guarded / unpiped \$?" good \
'--- a/script.sh
+++ b/script.sh
@@ -1,6 +1,6 @@
+grep pattern file.txt | tail -1
+RUFF_EXIT=${PIPESTATUS[0]}
+set -o pipefail
+grep pattern file.txt | tail -1
+RUFF_EXIT=$?
+some_unpiped_command
+RC=$?'

  run_case "E1 bad: blank line between pipe and \$? (reviewer FN1)" bad \
'--- a/script.sh
+++ b/script.sh
@@ -1,3 +1,3 @@
+grep pattern file.txt | tail -1
+
+RUFF_EXIT=$?'

  run_case "E1 bad: comment between pipe and \$? (reviewer FN2)" bad \
'--- a/script.sh
+++ b/script.sh
@@ -1,3 +1,3 @@
+grep pattern file.txt | tail -1
+# capture the linter exit code
+RUFF_EXIT=$?'

  run_case "E1 bad: pipefail turned back off before the pipe (reviewer FN6)" bad \
'--- a/script.sh
+++ b/script.sh
@@ -1,4 +1,4 @@
+set -o pipefail
+set +o pipefail
+grep pattern file.txt | tail -1
+RUFF_EXIT=$?'

  run_case "E1 good: pipe char only inside a quoted regex (reviewer FP1)" good \
'--- a/script.sh
+++ b/script.sh
@@ -1,2 +1,2 @@
+grep -E '"'"'error|warning'"'"' file.txt
+RC=$?'

  run_case "E1 good: comment mentions \$? right after a real pipe (reviewer FP2)" good \
'--- a/script.sh
+++ b/script.sh
@@ -1,3 +1,3 @@
+grep pattern file.txt | tail -1
+# a bare $? here would be wrong, we use PIPESTATUS below
+RUFF_EXIT=${PIPESTATUS[0]}'

  run_case "E1 good: case-pattern | is not a command pipe (reviewer FP3)" good \
'--- a/script.sh
+++ b/script.sh
@@ -1,3 +1,3 @@
+case "$result" in
+  pass|warn)
+    RC=$?'

  rm -rf "$tmp"
  unset -f run_case
  echo "-- $p passed, $f failed (denominator: $((p+f)) checks) --"
  [ "$f" -eq 0 ]
}

main() {
  if [ "${1:-}" = "--selftest" ]; then
    selftest
    return $?
  fi

  command -v git >/dev/null || { echo "recur-gate: git absent"; return 3; }

  if [ "${1:-}" = "--diff" ] && [ -n "${2:-}" ]; then
    check_diff "$2"
    return $?
  fi

  if git diff --cached --quiet 2>/dev/null; then
    if git rev-parse HEAD >/dev/null 2>&1; then
      git diff HEAD > /tmp/recur-gate-diff.$$ 2>/dev/null || { echo "recur-gate: git diff failed"; return 3; }
    else
      # NOT-APPLICABLE, not a zero count: there is no diff in existence to examine, which is
      # different from having a diff and examining none of it. Returning prose alone made the
      # parser report `Unparseable`; publishing `checked=0` made it `MeasuredNothing` and failed
      # the run. Both are wrong for a repo that simply has no changes yet -- this marker makes it
      # a visible SKIP (see `DenominatorResult::NotApplicable`).
      echo "recur-gate: not-applicable -- no commits yet, so there is no diff to examine"
      return 0
    fi
  else
    git diff --cached > /tmp/recur-gate-diff.$$ 2>/dev/null || { echo "recur-gate: git diff --cached failed"; return 3; }
  fi
  # An EMPTY diff is not-applicable, for the same reason as the no-commits path above: a clean
  # tree has nothing for a recurrence scan to examine. Without this, `fleet run` against any
  # unmodified repo failed on `recur: MeasuredNothing` -- the gate correctly refusing to call
  # 0-of-0 a pass, applied to a question that was never asked.
  if [ ! -s /tmp/recur-gate-diff.$$ ]; then
    rm -f /tmp/recur-gate-diff.$$
    echo "recur-gate: not-applicable -- working tree is clean, so there is no diff to examine"
    return 0
  fi
  check_diff /tmp/recur-gate-diff.$$
  local rc=$?
  rm -f /tmp/recur-gate-diff.$$
  return $rc
}

main "$@"
