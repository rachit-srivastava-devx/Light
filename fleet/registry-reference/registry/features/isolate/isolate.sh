#!/usr/bin/env bash
# isolate.sh — thin adapter over `treehouse` (pooled, reusable git worktrees) for parallel-agent
# isolation. Replaces bin/snapshot.sh + registry/lib/cilock.sh (ADOPT.md capability A, 194 lines): treehouse
# pools real worktrees with in-use detection and kills lingering processes on return — the upstream
# answer to the mktemp path collision, the stale CI lock, and the orphaned-process pile-up that
# ADOPT.md records against the hand-rolled versions. This script authors no isolation logic of its
# own: it only shells out to treehouse and writes a receipt.
#
# WHY --lease, NEVER `get`/`enter`: a bare `treehouse get` or `treehouse enter` opens an interactive
# subshell and hangs a non-interactive agent forever. `--lease` is treehouse's own documented
# non-interactive path: it reserves a worktree, prints ONLY the path to stdout (banners go to
# stderr; `--json` adds the lease id), and never opens a shell. This script never calls a subcommand
# that can block on a TTY.
#
# WHY return ALWAYS PASSES --force: measured empirically (2026-08-22) — `treehouse return` on a
# dirty worktree with stdin closed does NOT hang (it reads EOF as "no"), but it prints "Aborted" and
# still exits 0, leaving the worktree leased. An automation caller trusting that exit code would
# believe the worktree was returned when it was not. --force removes the prompt path entirely, so
# the exit code here is always truthful.
#
# Usage: isolate.sh acquire [--repo PATH] [--holder NAME] [--json]
#        isolate.sh return <path>
#        isolate.sh status [--json]
#        isolate.sh prune [--yes]
# Exit:  0 ok · 2 usage · 3 missing tool · 4 unparseable · 6 invariant (registry/lib/err.sh is the full table)
set -u
D="$(cd "$(dirname "$0")/../../.." && pwd)"
# shellcheck source=../../registry/lib/err.sh
. "$D/registry/lib/err.sh"
# shellcheck source=../../registry/lib/receipt.sh
. "$D/registry/lib/receipt.sh"

require_bin treehouse 'brew install treehouse (kunchenguid/treehouse)'
require_bin jq 'brew install jq'

# In-project pool by convention: keeps worktrees at <repo>/var/treehouse/ instead of sprawling into
# $HOME (treehouse's own default). Override with FLEET_TREEHOUSE_ROOT for a shared/global pool.
ROOT="${FLEET_TREEHOUSE_ROOT:-$D/var/treehouse}"
GEN="not-applicable"; VER="not-applicable"   # worktree bookkeeping: no model involved   # I1 needs both named and distinct; this adapter runs no model.
now_iso() { [ -n "${NOW:-}" ] && printf '%s' "$NOW" || date -u +%Y-%m-%dT%H:%M:%SZ; }

# _receipt <event> <task_id> <exit_code> — never fails the caller's own exit code on a ledger hiccup.
_receipt() { receipt_append "$(now_iso)" "$1" isolate "$2" "$GEN" "$VER" "$3" 0 0 0 >/dev/null 2>&1 || true; }

usage() {
  printf '%s\n' \
    'isolate.sh acquire [--repo PATH] [--holder NAME] [--json]   # non-interactive lease; prints path (or lease JSON)' \
    'isolate.sh return <path>                       # always --force: truthful exit code, never prompts' \
    'isolate.sh status [--json]                     # pool status, passthrough' \
    'isolate.sh prune [--yes]                       # dry run unless --yes, passthrough'
}

cmd="${1:-}"; shift || true
case "$cmd" in
  acquire)
    holder="${FLEET_ISOLATE_HOLDER:-fleet-$$}"; json=0; repo=""
    while [ $# -gt 0 ]; do case "$1" in
      --repo) need_val --repo "${2-}"; repo="$2"; shift 2 ;;
      --holder) need_val --holder "${2-}"; holder="$2"; shift 2 ;;
      --json) json=1; shift ;;
      -h|--help) usage; exit 0 ;;
      *) reject_unknown_flag "$1" ;;
    esac; done
    if [ -n "$repo" ]; then
      repo="$(cd "$repo" 2>/dev/null && pwd)" || die "$ERR_USAGE" bad_repo "repo is not a readable directory: $repo" 'pass --repo /path/to/git/repo'
      git -C "$repo" rev-parse --show-toplevel >/dev/null 2>&1 || die "$ERR_USAGE" not_git_repo "repo is not a git repository: $repo" 'initialize the target repository and retry'
    fi
    if [ -n "$repo" ] && [ -z "${FLEET_TREEHOUSE_ROOT:-}" ]; then
      ROOT="$repo/var/treehouse"
    fi
    errfile="$(mktemp "${TMPDIR:-/tmp}/fleet-isolate-get.XXXXXX")"
    if [ -n "$repo" ]; then
      out="$(cd "$repo" && treehouse get --root "$ROOT" --lease --json --lease-holder "$holder" --no-fetch 2>"$errfile")"
      ec=$?
    else
      out="$(treehouse get --root "$ROOT" --lease --json --lease-holder "$holder" 2>"$errfile")"
      ec=$?
    fi
    errmsg="$(cat "$errfile" 2>/dev/null)"; rm -f "$errfile"
    if [ "$ec" -ne 0 ] || [ -z "$out" ]; then
      _receipt acquire_failed "$holder" "$ec"
      die "$ERR_GENERIC" acquire_failed "treehouse get --lease failed: ${errmsg:-no output}" "run: treehouse status --root $ROOT"
    fi
    path="$(printf '%s' "$out" | jq -r '.path // empty' 2>/dev/null)"
    lease="$(printf '%s' "$out" | jq -r '.lease_id // empty' 2>/dev/null)"
    [ -n "$path" ] || { _receipt acquire_unparseable "$holder" "$ERR_PARSE"; die "$ERR_PARSE" acquire_unparseable 'treehouse --lease --json produced no .path' 'run treehouse get --lease --json directly to inspect its output'; }
    [ -d "$path" ] || { _receipt acquire_invariant "${lease:-$path}" "$ERR_INVARIANT"; die "$ERR_INVARIANT" acquire_no_dir "leased path does not exist on disk: $path" "run: treehouse status --root $ROOT --json"; }
    _receipt acquire "${lease:-$path}" 0
    if [ "$json" -eq 1 ]; then printf '%s\n' "$out"; else printf '%s\n' "$path"; fi
    ;;
  return)
    path="${1:-}"
    [ -n "$path" ] || die "$ERR_USAGE" missing_path 'return requires a worktree path' 'isolate.sh return <path>'
    shift || true
    [ $# -eq 0 ] || reject_unknown_flag "$1"
    treehouse return "$path" --force; ec=$?
    _receipt return "$path" "$ec"
    exit "$ec"
    ;;
  status)
    json=0
    while [ $# -gt 0 ]; do case "$1" in
      --json) json=1; shift ;;
      -h|--help) usage; exit 0 ;;
      *) reject_unknown_flag "$1" ;;
    esac; done
    if [ "$json" -eq 1 ]; then exec treehouse status --root "$ROOT" --json
    else exec treehouse status --root "$ROOT"
    fi
    ;;
  prune)
    yes=0
    while [ $# -gt 0 ]; do case "$1" in
      --yes) yes=1; shift ;;
      -h|--help) usage; exit 0 ;;
      *) reject_unknown_flag "$1" ;;
    esac; done
    if [ "$yes" -eq 1 ]; then exec treehouse prune --root "$ROOT" --yes
    else exec treehouse prune --root "$ROOT"
    fi
    ;;
  -h|--help|"") usage; exit 0 ;;
  *) die "$ERR_USAGE" unknown_subcommand "unknown subcommand '$cmd'" 'isolate.sh {acquire|return|status|prune}' ;;
esac
