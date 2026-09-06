#!/usr/bin/env bash

# Deterministic scorer fixture. It is not an experimental agent: it emits a deliberately weak
# terse patch and a stronger detailed patch so parity-run's pilot pass branch can be tested without
# spending model tokens or weakening the production stub's determinism.
set -u

if [ "${1:-}" != run ]; then
  exit 7
fi
shift
task=""
repo=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --task) task="$2"; shift 2 ;;
    --repo) repo="$2"; shift 2 ;;
    --agent) shift 2 ;;
    *) exit 7 ;;
  esac
done
[ -n "$task" ] && [ -n "$repo" ] || exit 7

if [[ "$task" == *"human-readable"* || "$task" == *"required field"* || "$task" == *"without changing"* ]]; then
  printf '%s\n' \
    'fn validate(value: Option<&str>) -> Result<(), ValidationError> {' \
    '    if value.is_none() || value == Some("") { return Err(ValidationError::Empty); }' \
    '    Ok(())' \
    '}' >> "$repo/main.rs"
  mkdir -p "$repo/tests"
  printf '%s\n' \
    '#[test]' \
    'fn accepts_valid_and_rejects_empty_boundary() {' \
    '    assert!(validate(Some("ok")).is_ok()); // success' \
    '    assert!(validate(None).is_err()); // failure' \
    '}' > "$repo/tests/boundary_test.rs"
else
  printf '%s\n' 'fn requested_change() {}' >> "$repo/main.rs"
fi

artifact_id=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
mkdir -p "$FLEET_STATE/artifacts"
git -C "$repo" add -N . || exit 3
git -C "$repo" diff --binary > "$FLEET_STATE/artifacts/$artifact_id" || exit 3
chmod 0444 "$FLEET_STATE/artifacts/$artifact_id" || exit 3
printf 'artifact=%s\n' "$artifact_id"
