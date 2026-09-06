#!/usr/bin/env bash
# receipt.sh — hash-chained append-only receipt ledger + the three invariants.
# WHY: this tree is not a git repo, so "a record the agent cannot edit" is a hash chain.
# Each line carries prev_hash; editing or deleting any line breaks verification.
# Canonical form (fixed order, '|' joined) is what gets hashed — NOT the JSON text,
# so key ordering or whitespace in the JSON can never change a hash.
#   hash = sha256( prev_hash + "\n" + canonical )
# Genesis prev_hash = 64 zeros.
# Assumes bash 3.2, shasum, jq. Concurrency: on one host, receipt_append serializes the
# read-modify-write of the ledger with an atomic mkdir lock, so every successful append observes
# the prior row and gets a unique prev_hash. It does not handle multiple machines or filesystems
# without atomic mkdir semantics (for example, NFS without a locking guarantee).
set -u

RECEIPT_GENESIS="0000000000000000000000000000000000000000000000000000000000000000"
# shellcheck disable=SC2034
RECEIPT_FIELDS="ts event component task_id generator_model verifier_model exit_code tokens_in tokens_out cost_micro_usd"

_receipt_path() { printf '%s' "${FLEET_LEDGER:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/ledger/RECEIPTS.jsonl}"; }
_receipt_lock() { printf '%s.lock' "$(_receipt_path)"; }

# sha256, portable, with ZERO external file dependency. Linux ships sha256sum; macOS ships
# shasum. Detection is inline and self-contained ON PURPOSE: this function is the root of the
# tamper-evidence chain, so it must not be able to fail because a sibling file could not be
# located. An earlier version sourced registry/lib/portable.sh via an inferred path and silently broke.
if command -v sha256sum >/dev/null 2>&1; then
  _sha() { printf '%s' "$1" | sha256sum | awk '{print $1}'; }
elif command -v shasum >/dev/null 2>&1; then
  _sha() { printf '%s' "$1" | shasum -a 256 | awk '{print $1}'; }
else
  _sha() { echo "receipt: no sha256 implementation (need sha256sum or shasum)" >&2; return 3; }
fi

# _canon <ts> <event> <component> <task_id> <gen> <ver> <exit> <tin> <tout> <cost>
_canon() { printf '%s|%s|%s|%s|%s|%s|%s|%s|%s|%s' "${1-}" "${2-}" "${3-}" "${4-}" "${5-}" "${6-}" "${7-}" "${8-}" "${9-}" "${10-}"; }

_is_int() { case "${1-}" in ''|*[!0-9-]*) return 1 ;; -) return 1 ;; *) return 0 ;; esac; }

receipt_last_hash() {
  local f; f="$(_receipt_path)"
  [ -s "$f" ] || { printf '%s' "$RECEIPT_GENESIS"; return 0; }
  tail -n 1 "$f" | jq -r '.hash // empty' 2>/dev/null || printf '%s' "$RECEIPT_GENESIS"
}

# A killed writer used to leave its lock directory behind, and every later append then failed with
# "could not acquire ledger lock" — forever. That is exactly what happened when concurrent agents
# were killed mid-write: several CI stages went red for a stale directory, not for a real defect.
# So the lock now records its owner PID and is breakable on two conditions: the owner is gone, or
# the lock is older than LOCK_STALE_SEC. Both are logged, because silently stealing a lock is how
# two writers end up interleaving.
RECEIPT_LOCK_STALE_SEC="${FLEET_LOCK_STALE_SEC:-120}"

_lock_mtime() { stat -c %Y "$1" 2>/dev/null || stat -f %m "$1" 2>/dev/null; }

_lock_find_owner_marker() {
  local lock="$1" owner="$2" marker
  for marker in "$lock"/owner."$owner".*; do
    [ -d "$marker" ] && { printf '%s' "$marker"; return 0; }
  done
  return 1
}

_lock_find_any_marker() {
  local lock="$1" marker
  for marker in "$lock"/owner.*; do
    [ -d "$marker" ] && { printf '%s' "$marker"; return 0; }
  done
  return 1
}

_lock_remove_owner_marker() {
  local lock="$1" marker="$2"
  # Removing the exact marker observed by the breaker is the compare step: a replacement
  # owner has a different marker, so this rmdir fails and its lock remains untouched.
  rmdir "$marker" 2>/dev/null || return 1
  rm -f "$lock/pid" 2>/dev/null || true
  rmdir "$lock" 2>/dev/null || return 1
  return 0
}

_lock_break_if_stale() {
  local lock="$1" owner age now marker any_marker
  [ -d "$lock" ] || return 1
  owner="$(cat "$lock/pid" 2>/dev/null || true)"
  now="$(date +%s)"; age=$(( now - $(_lock_mtime "$lock" 2>/dev/null || echo "$now") ))
  if [ -n "$owner" ] && ! kill -0 "$owner" 2>/dev/null; then
    marker="$(_lock_find_owner_marker "$lock" "$owner" 2>/dev/null || true)"
    if [ -n "$marker" ] && _lock_remove_owner_marker "$lock" "$marker"; then
      echo "receipt: breaking lock held by dead pid $owner" >&2; return 0
    fi
    # Compatibility path for locks created by an older receipt.sh without owner markers. If any
    # marker exists, it belongs to a successor and must prevent this legacy fallback from touching
    # the successor's pid file.
    any_marker="$(_lock_find_any_marker "$lock" 2>/dev/null || true)"
    if [ -z "$marker" ] && [ -z "$any_marker" ] && rm -f "$lock/pid" 2>/dev/null && rmdir "$lock" 2>/dev/null; then
      echo "receipt: breaking lock held by dead pid $owner" >&2; return 0
    fi
    return 1
  fi
  if [ "$age" -ge "$RECEIPT_LOCK_STALE_SEC" ]; then
    # The age rule deliberately permits recovery from an abandoned/live-but-stuck owner. Use the
    # exact marker when present, so a waiter that observed an older owner cannot steal a successor.
    marker="$(_lock_find_owner_marker "$lock" "$owner" 2>/dev/null || _lock_find_any_marker "$lock" 2>/dev/null || true)"
    if [ -n "$marker" ] && _lock_remove_owner_marker "$lock" "$marker"; then
      echo "receipt: breaking lock stale for ${age}s (owner ${owner:-unknown})" >&2; return 0
    fi
    # Compatibility path for locks created by an older receipt.sh without owner markers.
    if [ -z "$marker" ] && rm -f "$lock/pid" 2>/dev/null && rmdir "$lock" 2>/dev/null; then
      echo "receipt: breaking lock stale for ${age}s (owner ${owner:-unknown})" >&2; return 0
    fi
    return 1
  fi
  return 1
}

_lock_acquire() {
  local lock marker; lock="$(_receipt_lock)"; local n=0
  while ! mkdir "$lock" 2>/dev/null; do
    n=$((n+1))
    # Stale-owner inspection performs several subprocesses. It is recovery work, not the hot
    # wait path; once per second keeps the existing 100 x 0.1s (~10s) failure contract while
    # leaving most contention time to a cheap sleep.
    if [ "$n" -eq 1 ] || [ $((n % 10)) -eq 0 ]; then
      _lock_break_if_stale "$lock" && continue
    fi
    [ "$n" -ge 100 ] && { echo "receipt: lock held by live pid $(cat "$lock/pid" 2>/dev/null || echo unknown) after 10s" >&2; return 1; }
    sleep 0.1
  done
  marker="$lock/owner.$$.$RANDOM"
  if ! mkdir "$marker" 2>/dev/null; then
    rmdir "$lock" 2>/dev/null || true
    return 1
  fi
  if ! printf '%s' "$$" > "$lock/pid" 2>/dev/null; then
    rmdir "$marker" 2>/dev/null || true
    rmdir "$lock" 2>/dev/null || true
    return 1
  fi
  return 0
}
# Remove the exact owner marker before rmdir: unlike recursive rm, this cannot delete a replacement
# lock that another writer creates after the directory disappears. Do not treat a missing pid as
# ownership: a new owner has a short mkdir-to-pid window, and removing that lock would reopen the
# read-modify-write race.
_lock_release() {
  local lock owner marker
  lock="$(_receipt_lock)"
  owner="$(cat "$lock/pid" 2>/dev/null || true)"
  [ "$owner" = "$$" ] || return 0
  marker="$(_lock_find_owner_marker "$lock" "$owner" 2>/dev/null || true)"
  [ -n "$marker" ] || return 0
  _lock_remove_owner_marker "$lock" "$marker" || true
}

# receipt_append <ts> <event> <component> <task_id> <gen_model> <ver_model> <exit> <tin> <tout> <cost_micro>
# Exit 6 if a numeric field is not an integer (invariant I3). Exit 1 if the lock times out.
receipt_append() {
  local ts="${1:?ts}" ev="${2:?event}" comp="${3:?component}" tid="${4:?task_id}"
  local gen="${5-}" ver="${6-}" ec="${7:-0}" tin="${8:-0}" tout="${9:-0}" cost="${10:-0}"
  local n; for n in "$ec" "$tin" "$tout" "$cost"; do
    _is_int "$n" || { echo "receipt_append: I3 violated — '$n' is not an integer" >&2; return 6; }
  done
  local f; f="$(_receipt_path)"; mkdir -p "$(dirname "$f")"
  _lock_acquire || { echo "receipt_append: could not acquire ledger lock" >&2; return 1; }
  local prev canon h
  prev="$(receipt_last_hash)"; [ -n "$prev" ] || prev="$RECEIPT_GENESIS"
  canon="$(_canon "$ts" "$ev" "$comp" "$tid" "$gen" "$ver" "$ec" "$tin" "$tout" "$cost")"
  h="$(_sha "$prev
$canon")"
  jq -cn --arg ts "$ts" --arg event "$ev" --arg component "$comp" --arg task_id "$tid" \
        --arg generator_model "$gen" --arg verifier_model "$ver" \
        --argjson exit_code "$ec" --argjson tokens_in "$tin" --argjson tokens_out "$tout" \
        --argjson cost_micro_usd "$cost" --arg prev_hash "$prev" --arg hash "$h" \
    '{ts:$ts,event:$event,component:$component,task_id:$task_id,generator_model:$generator_model,verifier_model:$verifier_model,exit_code:$exit_code,tokens_in:$tokens_in,tokens_out:$tokens_out,cost_micro_usd:$cost_micro_usd,prev_hash:$prev_hash,hash:$hash}' >> "$f"
  # The write's status is the function's status. Previously `_lock_release` and `printf` -- both
  # always successful -- were the last statements, so a failed jq produced a printed hash, exit 0,
  # and NOTHING appended. Any caller counting receipts would believe a line existed that did not.
  _w=$?
  _lock_release
  if [ "$_w" -ne 0 ]; then
    echo "receipt: jq failed to encode the record; nothing appended" >&2
    return 4
  fi
  printf '%s\n' "$h"
}

# receipt_verify_chain [--incremental] — exit 8 on a break, printing the 1-based line number.
#
# SCALING, MEASURED: the previous implementation spawned `jq` + a sha PER LINE — 23ms/line,
# linear. 3,200 lines took 73.6s; 1M lines projected to ~6.4 HOURS, and this runs inside CI.
# Replaced with a SINGLE streaming pass (one process, hashlib), plus Certificate-Transparency
# style signed checkpoints so incremental verification is O(new lines) instead of O(n).
# Full verification remains available and is what a client audit should run.
_receipt_ckpt() { printf '%s' "${FLEET_LEDGER_CKPT:-$(_receipt_path).ckpt}"; }

receipt_verify_chain() {
  local f incremental=0
  f="$(_receipt_path)"
  [ "${1-}" = "--incremental" ] && incremental=1
  [ -s "$f" ] && : || { echo "chain: empty ledger (vacuously intact)"; return 0; }
  FLEET_CKPT="$(_receipt_ckpt)" FLEET_INCR="$incremental" python3 - "$f" <<'PY_VERIFY'
import hashlib, json, os, sys
GENESIS = "0" * 64
path = sys.argv[1]
ckpt_path = os.environ.get("FLEET_CKPT", path + ".ckpt")
incremental = os.environ.get("FLEET_INCR") == "1"

start_line, prev = 0, GENESIS
if incremental and os.path.exists(ckpt_path):
    try:
        with open(ckpt_path) as fh:
            c = json.load(fh)
        # A checkpoint is only usable if the line it names still carries the hash it recorded.
        # That makes a rewritten prefix detectable rather than skipped.
        with open(path) as fh:
            for i, line in enumerate(fh, 1):
                if i == c["line"]:
                    if json.loads(line)["hash"] == c["hash"]:
                        start_line, prev = c["line"], c["hash"]
                    break
    except Exception:
        start_line, prev = 0, GENESIS   # unusable checkpoint => full verify, never a skip

FIELDS = ("ts","event","component","task_id","generator_model","verifier_model",
          "exit_code","tokens_in","tokens_out","cost_micro_usd")
n = 0
with open(path) as fh:
    for n, line in enumerate(fh, 1):
        if n <= start_line:
            continue
        line = line.strip()
        if not line:
            continue
        try:
            r = json.loads(line)
        except Exception:
            print(f"chain: line {n} is not valid JSON", file=sys.stderr); raise SystemExit(8)
        if r.get("prev_hash") != prev:
            print(f"chain: BROKEN at line {n} - prev_hash mismatch "
                  f"(expected {prev}, found {r.get('prev_hash')})", file=sys.stderr)
            raise SystemExit(8)
        canon = "|".join(str(r.get(k, "")) for k in FIELDS)
        h = hashlib.sha256((prev + "\n" + canon).encode()).hexdigest()
        if h != r.get("hash"):
            print(f"chain: BROKEN at line {n} - content edited (hash mismatch)", file=sys.stderr)
            raise SystemExit(8)
        prev = r["hash"]
if n:
    tmp = ckpt_path + ".tmp"
    with open(tmp, "w") as fh:
        json.dump({"line": n, "hash": prev}, fh)
    os.replace(tmp, ckpt_path)
print(f"chain: intact ({n} lines)" + (f", verified from line {start_line+1}" if start_line else ""))
PY_VERIFY
}

# --- the three mechanical invariants ---
# I1: nobody verifies their own work.
# I1: a generator must not grade its own work. Two rules, and the second is the one that was
# missing for the whole life of this file:
#
#  1. Two REAL models must differ.
#  2. A pair that names no model at all is exempt, not compliant. Requiring `gen != ver`
#     unconditionally is what drove seven adapters to invent pairs like memory/memory-verifier,
#     kmap-generator/kmap-verifier and context/context-verifier just to get past the check. The
#     result: 447 of 521 receipts (86%) satisfied I1 with strings that are not models, so the
#     invariant auto-passed and proved nothing. A check that is cheaper to fake than to satisfy
#     will be faked.
#
# So: an internal bookkeeping operation writes `not-applicable` for BOTH fields and is allowed.
# It is recorded as unattributed rather than counted as independently verified — see
# console/server/collect.mjs, which refuses to treat a non-model string as a model.
I1_SENTINELS='human none not-applicable'
_i1_is_sentinel() { case " $I1_SENTINELS " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }
# Deliberately the same shape as collect.mjs's MODEL_PATTERN. Two copies is one too many; the
# duplication is called out here so the next edit moves both.
_i1_looks_like_model() {
  printf '%s' "$1" | grep -qiE '^(claude|opus|sonnet|haiku|fable|codex|gpt|o[0-9]|gemini|grok|llama|qwen|deepseek|mistral|kimi|pi|pi-signed|opencode|cursor|rovodev|copilot|antigravity|acp)'
}
invariant_i1() {
  local gen="${1-}" ver="${2-}"
  [ -n "$gen" ] && [ -n "$ver" ] || { echo "I1: both fields must be set (use 'not-applicable' when no model is involved)" >&2; return 6; }
  # Both sentinels: no generation happened, nothing to verify independently. Exempt.
  if _i1_is_sentinel "$gen" && _i1_is_sentinel "$ver"; then return 0; fi
  # A real model on one side and a made-up token on the other is the failure this now catches:
  # it LOOKS like independent verification and is not.
  if _i1_looks_like_model "$gen" && ! _i1_looks_like_model "$ver" && ! _i1_is_sentinel "$ver"; then
    echo "I1 VIOLATED: verifier '$ver' is not a model or a sentinel — '$gen' is unverified" >&2; return 6
  fi
  if _i1_looks_like_model "$ver" && ! _i1_looks_like_model "$gen" && ! _i1_is_sentinel "$gen"; then
    echo "I1 VIOLATED: generator '$gen' is not a model or a sentinel" >&2; return 6
  fi
  # Neither side is a model and neither is a sentinel: this is a FABRICATED pair. It is the exact
  # shape seven adapters invented (memory/memory-verifier, kmap-generator/kmap-verifier,
  # context/context-verifier) to get past the old unconditional inequality check. Refuse it and say
  # what to write instead, or the invariant keeps being satisfied by naming convention.
  if ! _i1_looks_like_model "$gen" && ! _i1_looks_like_model "$ver"; then
    echo "I1 VIOLATED: neither '$gen' nor '$ver' is a model; use 'not-applicable' for both when no model is involved" >&2
    return 6
  fi
  [ "$gen" != "$ver" ] || { echo "I1 VIOLATED: generator == verifier ($gen)" >&2; return 6; }
  return 0
}
# I2: never start work you cannot finish.
invariant_i2() {
  local projected="${1:?projected}" remaining="${2:?remaining}"
  _is_int "$projected" && _is_int "$remaining" || { echo "I2: non-integer input" >&2; return 6; }
  [ "$projected" -le "$remaining" ] || { echo "I2 VIOLATED: projected $projected > remaining $remaining" >&2; return 5; }
  return 0
}
# I3: money/tokens/counts are integers.
invariant_i3() {
  local v; for v in "$@"; do _is_int "$v" || { echo "I3 VIOLATED: '$v' is not an integer" >&2; return 6; }; done
  return 0
}
