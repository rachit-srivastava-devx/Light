#!/usr/bin/env bash
# M5 — a tool named as ADOPTED must be invoked by something that is not a document.
# `rekor-cli` was installed, named in docs/ADOPTION.md and docs/CONFORMANCE.md, and called by no
# code. PRINCIPLES #1: two tools were once adopted and defended for a day without either having
# run. An installed binary is not an adoption; a mention in a document is not a caller.
set -u
ROOT="${FLEET_TARGET_REPO:-$(pwd)}"
A="$ROOT/docs/ADOPTION.md"
[ -r "$A" ] || exit 77
FAILED=0; CHECKED=0
for tool in witness cosign conftest opa rekor-cli; do
  grep -qi "$tool" "$A" 2>/dev/null || continue
  # a row is ADOPTED only if it says **ADOPTED** and NOT UNADOPTED-WITH-REASON. The first pattern
  # matched neither, so M5 checked zero tools and correctly refused rather than passing vacuously.
  row=$(grep -i "^| .*\`\?$tool\`\?" "$A" 2>/dev/null | head -1)
  case "$row" in *UNADOPTED*) continue ;; *"**ADOPTED**"*) ;; *) continue ;; esac
  CHECKED=$((CHECKED+1))
  # a caller is a .sh or .rs that names it — documents do not count
  if ! grep -rlq "$tool" "$ROOT/bin" "$ROOT/policy" "$ROOT/keel/fleet/src" "$ROOT/verify.sh" 2>/dev/null; then
    echo "M5: '$tool' is listed ADOPTED but no script or source invokes it — only documents mention it."
    FAILED=$((FAILED+1))
  fi
done
[ "$CHECKED" -gt 0 ] || { echo "M5: no ADOPTED tools found to check — measuring nothing is a failure"; exit 1; }
echo "M5: $((CHECKED-FAILED)) of $CHECKED adopted tools have a real caller (denominator: $CHECKED)"
[ "$FAILED" -eq 0 ]
