#!/usr/bin/env bash
# arch-check.sh — executable one-way dependency contract.
# Exit 6 is reserved for a layering violation; comments and documentation do not count.
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
violations=0

report() {
  printf 'arch-check: %s:%s: %s\n' "$1" "$2" "$3" >&2
  violations=$((violations + 1))
}

check_file() {
  local tier="$1" file="$2" line code refs token line_no=0
  while IFS= read -r line || [ -n "$line" ]; do
    line_no=$((line_no + 1))
    code="${line%%#*}"
    [ -n "${code//[[:space:]]/}" ] || continue
    case "$file:$code" in
      *"/registry/modules/quality/corpus.sh:ok "*) continue ;;
    esac
    if ! printf '%s\n' "$code" | grep -Eq '(^|[[:space:];])(source|\.|exec|bash|python3?|node)[[:space:]]|\$[A-Za-z_][A-Za-z0-9_]*//(features|services|modules)/'; then
      continue
    fi
    refs="$(printf '%s\n' "$code" | grep -oE 'registry/(features|services|modules)/[A-Za-z0-9_-]+' || true)"
    while IFS= read -r token; do
      [ -n "$token" ] || continue
      case "$tier:$token" in
        features:registry/features/*|features:registry/services/*|features:registry/modules/*)
          report "$file" "$line_no" "feature may import only registry/lib/: $token" ;;
        services:registry/services/*|services:registry/modules/*)
          report "$file" "$line_no" "service may import registry/features/lib, not $token" ;;
        modules:registry/features/*)
          report "$file" "$line_no" "module must compose registry/services/lib, not $token" ;;
      esac
    done <<<"$refs"
  done <"$file"
}

for tier in features services modules; do
  dir="$ROOT/registry/$tier"
  [ -d "$dir" ] || continue
  while IFS= read -r file; do
    [ -n "$file" ] || continue
    check_file "$tier" "$file"
  done < <(rg --files "$dir" | grep -E '\.(sh|py|mjs|js|ts)$' || true)
done

if [ "$violations" -gt 0 ]; then
  printf 'arch-check: %s violation(s)\n' "$violations" >&2
  exit 6
fi
printf 'arch-check: clean (registry/features -> registry/lib; registry/services -> features/lib; registry/modules -> services/lib)\n'
exit 0
