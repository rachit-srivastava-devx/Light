#!/usr/bin/env bash
# portable.sh — resolve the BSD/GNU differences once, so fleet runs in a client's Linux CI.
#
# WHY THIS EXISTS: the committed GitHub Actions workflow targets ubuntu-latest, while 12 files
# called `shasum` (macOS) rather than `sha256sum` (Linux), and two used BSD-only `date -j` and
# `sed -i ''`. That workflow would have failed on its first run in a client's CI. "Works on the
# author's machine" is not a property a client can use.
#
# Detection happens ONCE at source time. Every function is deterministic and integer-safe.
# Does NOT handle: Windows, busybox (documented — both would need their own shim).
set -u

# --- sha256 -------------------------------------------------------------------
if command -v sha256sum >/dev/null 2>&1; then
  P_SHA_IMPL=sha256sum
elif command -v shasum >/dev/null 2>&1; then
  P_SHA_IMPL=shasum
else
  P_SHA_IMPL=none
fi
# p_sha256  — reads stdin, prints the bare hex digest
p_sha256() {
  case "$P_SHA_IMPL" in
    sha256sum) sha256sum | awk '{print $1}' ;;
    shasum)    shasum -a 256 | awk '{print $1}' ;;
    *) echo "portable: no sha256 implementation (need sha256sum or shasum)" >&2; return 3 ;;
  esac
}

# --- iso8601 -> epoch seconds -------------------------------------------------
# p_epoch_from_iso <2026-08-22T07:33:43Z>  — prints epoch seconds, or empty on failure.
p_epoch_from_iso() {
  local iso="${1-}" s
  [ -n "$iso" ] || return 1
  s="${iso%%.*}"; s="${s%Z}"
  # GNU first (Linux CI is the target), then BSD.
  date -u -d "$s" +%s 2>/dev/null && return 0
  date -j -u -f "%Y-%m-%dT%H:%M:%S" "$s" +%s 2>/dev/null && return 0
  return 1
}

# --- in-place sed -------------------------------------------------------------
# p_sed_inplace <expr> <file>
p_sed_inplace() {
  local expr="${1:?expr}" file="${2:?file}"
  if sed --version >/dev/null 2>&1; then sed -i -e "$expr" "$file"      # GNU
  else sed -i '' -e "$expr" "$file"; fi                                  # BSD
}

# --- file mtime as epoch ------------------------------------------------------
p_mtime() {
  local f="${1:?file}"
  stat -c %Y "$f" 2>/dev/null || stat -f %m "$f" 2>/dev/null
}

# p_platform_report — one TOON-ready line naming what was resolved, for receipts/CI logs
p_platform_report() { printf 'sha=%s sed=%s date=%s\n' "$P_SHA_IMPL" \
  "$(sed --version >/dev/null 2>&1 && echo gnu || echo bsd)" \
  "$(date -u -d '2026-01-01T00:00:00' +%s >/dev/null 2>&1 && echo gnu || echo bsd)"; }
