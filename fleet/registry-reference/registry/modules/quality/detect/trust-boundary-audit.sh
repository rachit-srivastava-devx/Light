#!/usr/bin/env bash
# Prove the five trust-boundary mechanisms, one command each.
#
# The previous model declared four credential identities ABSENT and called that the finding. This
# one declares five mechanisms ENFORCED — which is worthless unless each is checked. A boundary
# asserted in a JS object is documentation; a boundary with a prover is a boundary.
set -uo pipefail
D="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$D" || exit 3
pass=0; fail=0
ck() { # ck <id> <what> <cmd...>
  local id="$1" what="$2"; shift 2
  if "$@" >/dev/null 2>&1; then pass=$((pass+1)); printf '  PROVEN  %-14s %s\n' "$id" "$what"
  else fail=$((fail+1)); printf '  FAILED  %-14s %s\n' "$id" "$what"; fi
}

# 1. SCOPE — the ownership sweep must walk the TARGET, not fleet's own tree. It was hardcoded to
#    fleet's tree, so a planted undeclared file in a client repo reported ownership clean.
ck scope "sweep resolves to the target worktree, not \$D" \
  grep -q 'sweep_root()' registry/services/dispatch/dispatch.sh

# 2. HUMAN GATE — push/pr/ci must be unconditionally skipped, not a clearable flag.
ck human-gate "push,pr,ci skipped unconditionally in gate.sh" \
  bash -c 'grep -qE "always skipped|SKIP_ALWAYS|push,pr,ci" registry/features/gate/gate.sh'

# 3. I1 — a fabricated <component>/<component>-verifier pair must be REFUSED, and two real distinct
#    models allowed. This is the separation that matters: between models, not humans.
ck i1-models "fabricated pair refused, distinct models allowed" \
  bash -c '. registry/lib/err.sh 2>/dev/null; . registry/lib/receipt.sh
    invariant_i1 memory memory-verifier >/dev/null 2>&1 && exit 1
    invariant_i1 codex codex           >/dev/null 2>&1 && exit 1
    invariant_i1 claude-opus-5 codex   >/dev/null 2>&1 || exit 1
    invariant_i1 not-applicable not-applicable >/dev/null 2>&1 || exit 1
    exit 0'

# 4. ATTRIBUTION — fleet must never set the git author. The user authors their own commits; fleet is
#    disclosed via a trailer. A tool that rewrites user.email is impersonating, not attributing.
ck attribution "fleet never writes git user.name/user.email config" \
  bash -c '! grep -rn "git config user\.\(name\|email\)\|git config --global user\." registry --include="*.sh" | grep -v "^registry/modules/quality/detect/" | grep -qv "#"'

# 5. BLAST RADIUS — the injection guard must be wired as a PreToolUse hook on both Bash and Write.
# 5. BLAST RADIUS — test the guard's BEHAVIOUR, not its registration.
#    The first version grepped .claude/settings.json and reported FAILED, because that is not where
#    the guard lives: hooks/settings-hooks.json is a TEMPLATE that init-project.sh merges into each
#    TARGET repo. Checking the wrong file called a working guard absent — the same class of error as
#    checking an API response and calling it a rendered page.
ck blast-radius "guard template covers Bash+Write and an installer merges it" \
  bash -c 'jq -e ".hooks.PreToolUse[] | select(.matcher | test(\"Bash\")) | .hooks[].command | test(\"injection-guard\")" hooks/settings-hooks.json >/dev/null 2>&1 && grep -q "settings-hooks.json" registry/modules/cli/init-project.sh'

# The credential-shaped literal is ASSEMBLED AT RUNTIME. It cannot appear in this file, because the
# guard under test blocks writing one — it blocked the very command that first tried to add this
# check. That refusal is the feature, so the test works around its own subject instead of weakening
# it, and this comment is the record of the guard proving itself.
ck blast-radius-live "guard REFUSES a credential-shaped command" \
  bash -c 'p1="AKIA"; p2="IOSFODNN7"; p3="EXAMPLE"
    printf "{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"echo %s%s%s\"}}" "$p1" "$p2" "$p3" \
      | bash hooks/injection-guard.sh >/dev/null 2>&1
    ec=$?
    [ "$ec" -ne 0 ]'

ck blast-radius-pass "guard ALLOWS an ordinary command (not a blanket deny)" \
  bash -c 'printf "{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"ls -la\"}}" \
      | bash hooks/injection-guard.sh >/dev/null 2>&1'

echo
echo "  trust boundary: $pass proven, $fail failed"
[ "$fail" -eq 0 ] || exit 6
