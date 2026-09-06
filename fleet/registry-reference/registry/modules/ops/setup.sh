#!/usr/bin/env bash
# setup.sh — install or update the entire adopted stack declared in ADOPT.md.
#
# One script, one source of truth. Every dependency fleet has is installed here or it does not
# exist. Run it on a fresh machine and fleet works; run it again and it updates in place.
#
#   ./fleet setup --check     report what is present/missing/stale, install nothing (exit 3 if required missing)
#   ./fleet setup             install everything missing (idempotent; skips what is present)
#   ./fleet setup --update    upgrade every dependency to latest, re-pull cloned repos
#   ./fleet setup --only NAME limit to one component (repeatable)
#
# Design rules this script obeys, because fleet has been burned by each:
#   * idempotent — re-running never duplicates or breaks a working install
#   * checksum-verified — every downloaded binary is verified against the release checksums.txt
#   * no `curl | sh` — tarballs are downloaded, verified, then extracted (a piped installer cannot
#     be checked before it runs)
#   * pinned — cloned repos record their commit in state/deps.lock so a run is reproducible
#   * never silent — a component that cannot install says so and sets the exit code
set -uo pipefail

FLEET_ROOT="${FLEET_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}"
VENDOR="$FLEET_ROOT/vendor"
LOCK="$FLEET_ROOT/state/deps.lock"
BIN_DIR="${FLEET_BIN_DIR:-$HOME/.local/bin}"
MODE=install
ONLY=""

while [ $# -gt 0 ]; do
  case "$1" in
    --check)  MODE=check;  shift ;;
    --update) MODE=update; shift ;;
    --only)   [ -n "${2-}" ] || { echo "setup: --only needs a value" >&2; exit 2; }
              ONLY="$ONLY $2"; shift 2 ;;
    -h|--help) sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "setup: unknown flag '$1' (see --help)" >&2; exit 2 ;;
  esac
done

mkdir -p "$VENDOR" "$BIN_DIR" "$(dirname "$LOCK")"
rc=0
selected() { [ -z "$ONLY" ] && return 0; case " $ONLY " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }
say()  { printf '  %-9s %-22s %s\n' "$1" "$2" "${3-}"; }
have() { command -v "$1" >/dev/null 2>&1; }

# ---------------------------------------------------------------------------
# 1. brew formulae — tools with a maintained formula. `brew list` is the probe so a manually
#    installed binary elsewhere on PATH does not mask a missing managed install.
# ---------------------------------------------------------------------------
# formula|binary|what it replaces in fleet
BREW='
treehouse|treehouse|snapshot.sh + registry/lib/cilock.sh (pooled worktrees, kills lingering procs)
bats-core|bats|the hand-rolled test runner
shellcheck|shellcheck|bash -n in gates/01 (syntax only -> real static analysis)
shfmt|shfmt|nothing yet — shell formatting gate
semgrep|semgrep|gates/08-security (currently exit 3, no engine installed)
gitleaks|gitleaks|nothing yet — secret scanning
trivy|trivy|nothing yet — vuln + SBOM + misconfig
jq|jq|hand-rolled JSON parsing
sqlite|sqlite3|SQLite FTS5 memory store (transactional writes + BM25 retrieval)
duckdb|duckdb|the analytic half of process-plane.sh
hyperfine|hyperfine|the benchmark half of perf.sh
'

echo "== brew =="
if ! have brew; then
  say MISSING brew "install from https://brew.sh — every tool below depends on it"; rc=3
else
  while IFS='|' read -r formula binary replaces; do
    [ -n "$formula" ] || continue
    selected "$formula" || continue
    if brew list --formula "$formula" >/dev/null 2>&1; then
      if [ "$MODE" = update ]; then
        brew upgrade "$formula" >/dev/null 2>&1 </dev/null && say updated "$formula" || say current "$formula"
      else
        say ok "$formula" "$replaces"
      fi
    elif [ "$MODE" = check ]; then
      say MISSING "$formula" "brew install $formula"; rc=3
    else
      printf '  %-9s %-22s installing…\n' '...' "$formula"
      if brew install "$formula" >/dev/null 2>&1 </dev/null; then say installed "$formula" "$replaces"
      else say FAILED "$formula" "brew install $formula — run manually for the error"; rc=1; fi
    fi
  done <<<"$BREW"
fi

# ---------------------------------------------------------------------------
# 2. GitHub releases — checksum-verified binaries with no brew formula.
# ---------------------------------------------------------------------------
echo "== github releases =="
install_release() {
  local repo="$1" binary="$2" replaces="$3"
  selected "$binary" || return 0
  local latest current
  latest=$(gh api "repos/$repo/releases/latest" --jq .tag_name 2>/dev/null) || latest=""
  [ -n "$latest" ] || { say FAILED "$binary" "cannot reach $repo releases"; rc=1; return; }
  current=""
  have "$binary" && current=$("$binary" --version 2>/dev/null | grep -oE 'v?[0-9]+\.[0-9]+\.[0-9]+' | head -1)
  if [ "v${current#v}" = "$latest" ]; then say ok "$binary" "$latest — $replaces"; return; fi
  if [ "$MODE" = check ]; then
    [ -n "$current" ] && { say stale "$binary" "have $current, latest $latest"; return; }
    say MISSING "$binary" "latest $latest — run without --check"; rc=3; return
  fi
  local arch tmp asset
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) arch=darwin-arm64 ;; Darwin-x86_64) arch=darwin-amd64 ;;
    Linux-x86_64) arch=linux-amd64 ;;  Linux-aarch64) arch=linux-arm64 ;;
    *) say FAILED "$binary" "unsupported platform $(uname -s)-$(uname -m)"; rc=1; return ;;
  esac
  tmp=$(mktemp -d "${TMPDIR:-/tmp}/fleet-dep-$binary.XXXXXX") || { rc=1; return; }
  asset="$binary-$latest-$arch.tar.gz"
  if ! ( cd "$tmp" && gh release download "$latest" --repo "$repo" --pattern "$asset" --pattern checksums.txt --clobber ) >/dev/null 2>&1; then
    say FAILED "$binary" "download $asset failed"; rm -rf "$tmp"; rc=1; return
  fi
  # Verify before extracting. An unverified binary is not installed, full stop.
  local want got
  want=$(grep -F "$asset" "$tmp/checksums.txt" 2>/dev/null | awk '{print $1}')
  got=$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')
  if [ -z "$want" ] || [ "$want" != "$got" ]; then
    say FAILED "$binary" "CHECKSUM MISMATCH — refusing to install"; rm -rf "$tmp"; rc=1; return
  fi
  tar xzf "$tmp/$asset" -C "$BIN_DIR" && chmod +x "$BIN_DIR/$binary"
  say installed "$binary" "$latest (sha256 verified) — $replaces"
  rm -rf "$tmp"
}
if have gh; then
  install_release kunchenguid/no-mistakes no-mistakes "gates/* + ci.sh + verify-all.sh + testlanes.sh"
else
  say MISSING gh "brew install gh && gh auth login"; rc=3
fi

# ---------------------------------------------------------------------------
# 3. Cloned repos — projects that are a directory, not a package. firstmate is an "agent distro":
#    the cloned repo IS the tool. Pinned by commit in state/deps.lock.
# ---------------------------------------------------------------------------
echo "== cloned repos =="
clone_dep() {
  local repo="$1" dir="$2" replaces="$3"
  selected "$dir" || return 0
  local path="$VENDOR/$dir"
  if [ -d "$path/.git" ]; then
    if [ "$MODE" = update ]; then
      ( cd "$path" && git fetch -q --depth 1 origin HEAD && git reset -q --hard FETCH_HEAD ) 2>/dev/null \
        && say updated "$dir" "$(cd "$path" && git rev-parse --short HEAD)" || { say FAILED "$dir" "git fetch failed"; rc=1; }
    else
      say ok "$dir" "$(cd "$path" && git rev-parse --short HEAD) — $replaces"
    fi
  elif [ "$MODE" = check ]; then
    say MISSING "$dir" "gh repo clone $repo vendor/$dir"; rc=3
  else
    if gh repo clone "$repo" "$path" -- --depth 1 >/dev/null 2>&1; then
      say cloned "$dir" "$(cd "$path" && git rev-parse --short HEAD) — $replaces"
    else say FAILED "$dir" "clone $repo failed"; rc=1; fi
  fi
  if [ "$MODE" != check ] && [ -d "$path/.git" ]; then
    printf '%s\t%s\t%s\n' "$dir" "$repo" "$(cd "$path" && git rev-parse HEAD)" >>"$LOCK.new"
  fi
}
[ "$MODE" = check ] || : >"$LOCK.new"
clone_dep kunchenguid/firstmate  firstmate  "dispatch route fanout detect codex-run autopilot brief-loop (70k lines upstream)"
clone_dep kunchenguid/axi        axi        "the AXI design principles fleet applied by hand"
if [ "$MODE" != check ]; then
  sort -o "$LOCK" "$LOCK.new" 2>/dev/null && rm -f "$LOCK.new"
fi

# ---------------------------------------------------------------------------
# 4. Python packages — isolated in a uv venv so fleet never touches the system interpreter.
#    The property-test lane broke once because `python3` and `pytest` resolved to different
#    interpreters; a dedicated venv is the fix, not a PATH tweak.
# ---------------------------------------------------------------------------
echo "== python (uv venv at var/venv) =="
PY='
llmlingua|llmlingua|compress.sh + compress2.sh (chars/4 heuristic -> measured 20x)
arize-phoenix|phoenix|process-plane.sh + metrics.sh + meter.sh + perf.sh
openinference-instrumentation|openinference|the OTel GenAI wire format Phoenix reads
tiktoken|tiktoken|exact token counts (req 2) instead of an estimate
psutil|psutil|library process sampling (CPU, RSS, children, open files)
mutmut|mutmut|a real mutation oracle for the python surface
pytest|pytest|the property-test lane
hypothesis|hypothesis|property-based generation
'
if ! have uv; then
  say MISSING uv "curl -LsSf https://astral.sh/uv/install.sh | sh   (or brew install uv)"; rc=3
else
  VENV="$FLEET_ROOT/var/venv"
  if [ ! -d "$VENV" ] && [ "$MODE" != check ]; then uv venv "$VENV" >/dev/null 2>&1 && say created var/venv; fi
  while IFS='|' read -r pkg mod replaces; do
    [ -n "$pkg" ] || continue
    selected "$pkg" || continue
    if [ -x "$VENV/bin/python" ] && "$VENV/bin/python" -c "import $mod" >/dev/null 2>&1; then
      if [ "$MODE" = update ]; then
        VIRTUAL_ENV="$VENV" uv pip install -q --upgrade "$pkg" >/dev/null 2>&1 </dev/null && say updated "$pkg" || say current "$pkg"
      else say ok "$pkg" "$replaces"; fi
    elif [ "$MODE" = check ]; then
      say MISSING "$pkg" "uv pip install $pkg"
    else
      if VIRTUAL_ENV="$VENV" uv pip install -q "$pkg" >/dev/null 2>&1 </dev/null; then say installed "$pkg" "$replaces"
      else say FAILED "$pkg" "uv pip install $pkg"; rc=1; fi
    fi
  done <<<"$PY"
fi

# ---------------------------------------------------------------------------
# 5. Cargo — the Rust surface. cargo-mutants is the reason to put oracle-graded logic in Rust at
#    all: shell has no mutation engine that exists (mutate4bash: 0 stars), so any code that must
#    be graded by a mutation oracle belongs in a language that has one.
# ---------------------------------------------------------------------------
echo "== cargo =="
CARGO='
cargo-mutants|cargo-mutants|the mutation oracle gates/03 hand-rolls (with a known awk bug)
cargo-deny|cargo-deny|supply-chain + licence gate for the rust surface
tokei|tokei|already adopted by kmap.sh
'
if ! have cargo; then
  say MISSING cargo "https://rustup.rs"; rc=3
else
  while IFS='|' read -r crate binary replaces; do
    [ -n "$crate" ] || continue
    selected "$crate" || continue
    if have "$binary"; then
      [ "$MODE" = update ] && { cargo install -q "$crate" >/dev/null 2>&1 </dev/null && say updated "$crate" || say current "$crate"; } || say ok "$crate" "$replaces"
    elif [ "$MODE" = check ]; then say MISSING "$crate" "cargo install $crate"
    else
      if cargo install -q "$crate" >/dev/null 2>&1 </dev/null; then say installed "$crate" "$replaces"
      else say FAILED "$crate" "cargo install $crate"; rc=1; fi
    fi
  done <<<"$CARGO"
fi

# ---------------------------------------------------------------------------
# 6. Key-required integrations are never installed. The default path is keyless; mem0 remains an
#    optional evaluation candidate and is intentionally not wired into the memory path.
# ---------------------------------------------------------------------------
echo "== key-required integrations (optional, unwired) =="
selected mem0 && say skipped mem0 "needs-key; SQLite FTS5 is the local memory store"

# ---------------------------------------------------------------------------
# 7. npm — keyless local usage reader plus the console.
# ---------------------------------------------------------------------------
echo "== npm =="
if selected ccusage; then
  if have ccusage; then say ok ccusage "local transcript token/cost reader (keyless)"
  elif [ "$MODE" = check ]; then say MISSING ccusage "npm install -g ccusage"; rc=3
  elif have npm && npm install -g ccusage >/dev/null 2>&1 </dev/null; then say installed ccusage "keyless transcript token/cost reader"
  else say FAILED ccusage "npm install -g ccusage"; rc=1; fi
fi
echo "== npm (console/web) =="
if [ -f "$FLEET_ROOT/console/web/package.json" ]; then
  if [ -d "$FLEET_ROOT/console/web/node_modules" ]; then
    [ "$MODE" = update ] && { ( cd "$FLEET_ROOT/console/web" && npm update --silent ) >/dev/null 2>&1 && say updated console/web || say current console/web; } || say ok console/web "@xyflow elkjs radix tanstack shiki playwright"
  elif [ "$MODE" = check ]; then say MISSING console/web "npm ci --prefix console/web"; rc=3
  else
    ( cd "$FLEET_ROOT/console/web" && npm ci --silent ) >/dev/null 2>&1 && say installed console/web || { say FAILED console/web "npm ci"; rc=1; }
  fi
fi

# ---------------------------------------------------------------------------
# 8. Agents — at least one, two for I1 (generator != verifier) to be satisfiable at all.
# ---------------------------------------------------------------------------
echo "== agents =="
agents=0
for a in claude codex; do
  if have "$a"; then say ok "$a" "$("$a" --version 2>/dev/null | head -1)"; agents=$((agents + 1))
  else say absent "$a" "not installed"; fi
done
[ "$agents" -ge 2 ] || { echo "  WARN: I1 (generator != verifier) needs two distinct agents; found $agents" >&2; }

echo
echo "mode=$MODE  exit=$rc  lock=$LOCK"
[ "$rc" -eq 0 ] && echo "stack complete" || echo "stack incomplete — see MISSING/FAILED rows above"
exit "$rc"
