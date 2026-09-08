#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="${FLEET_BIN_DIR:-${HOME}/.local/bin}"
BIN_PATH="${BIN_DIR}/fleet"
# M2/B18: AGENTS.md now documents an external CARGO_TARGET_DIR for developers of this repo (so
# build artifacts don't blow past the corpus suite's file-count cap). If that's exported here, the
# release build below lands there instead of the in-repo default -- compute the real build output
# path from it, don't hardcode the in-repo path, same pattern already used by verify.sh/FLEET_BIN.
BUILT_BIN="${CARGO_TARGET_DIR:-$ROOT_DIR/target}/release/fleet"
# NOTE: the env var is FLEET_STATE_DIR, not FLEET_STATE -- figment reads `state_dir` under the
# `FLEET_` prefix (src/runtime/config.rs). The old name here silently did nothing.
STATE_DIR="${FLEET_STATE_DIR:-${HOME}/.local/state/fleet}"
ZFUNC_DIR="${HOME}/.zfunc"
ZFUNC_FILE="${ZFUNC_DIR}/_fleet"

usage() {
    cat <<'EOF'
Usage: ./install.sh [--check|--uninstall]

Without a flag, build and install fleet. --check probes prerequisites and
prints the changes an install would make. --uninstall removes the binary and
the generated zsh completion file, but preserves local state and receipts.
EOF
}

fail_env() {
    printf 'fleet install refused: %s\n' "$1" >&2
    exit 3
}

if [[ "$(uname -s)" != "Darwin" ]]; then
    fail_env "macOS is required; detected $(uname -s). fleet is a local macOS CLI."
fi

MODE="install"
case "${1:-}" in
    "") ;;
    --check) MODE="check" ;;
    --uninstall) MODE="uninstall" ;;
    --help|-h) usage; exit 0 ;;
    *) usage >&2; exit 7 ;;
esac

probe_version() {
    local tool="$1" minimum_major="$2" minimum_minor="$3" output status major minor
    set +e
    output="$($tool --version 2>&1)"
    status=$?
    set -e
    if (( status != 0 )); then
        fail_env "$tool --version failed; install or repair $tool"
    fi
    if [[ ! "$output" =~ ([0-9]+)\.([0-9]+) ]]; then
        fail_env "$tool --version returned no parseable version: $output"
    fi
    major="${BASH_REMATCH[1]}"
    minor="${BASH_REMATCH[2]}"
    if (( major < minimum_major || (major == minimum_major && minor < minimum_minor) )); then
        fail_env "$tool $major.$minor is too old; need $minimum_major.$minimum_minor or newer"
    fi
    printf 'OK prerequisite: %s --version => %s\n' "$tool" "$output"
}

printf 'fleet install target: macOS (%s)\n' "$(uname -m)"
probe_version cargo 1 74
probe_version rustc 1 74
probe_version git 2 30

# A DIFFERENT program may already own this name. On the author's own machine
# ~/.local/bin/fleet was a symlink into a separate, unrelated `fleet` repository, and this
# installer would have overwritten it -- and --uninstall would have deleted it -- with no warning
# at all. Refuse instead, and say what is in the way.
foreign_bin() {
    [[ -e "$BIN_PATH" || -L "$BIN_PATH" ]] || return 1
    # A symlink is ours only if it resolves into this repo's own tree (e.g. a dev symlink into
    # our own build output). The path-prefix test is meaningless for a plain file -- comparing
    # BIN_PATH's own location to ROOT_DIR says nothing about who put it there or what it is, and
    # doing so let a foreign plain file sitting under a BIN_DIR nested inside ROOT_DIR bypass the
    # --help marker check entirely and get silently overwritten. So a plain file is judged ONLY by
    # content (the --help marker), never by where it happens to sit.
    if [[ -L "$BIN_PATH" ]]; then
        local real; real="$(readlink "$BIN_PATH")"
        case "$real" in "$ROOT_DIR"/*) return 1 ;; esac
        return 0
    fi
    "$BIN_PATH" --help 2>/dev/null | grep -q 'frozen, attested change' && return 1
    return 0
}
describe_foreign() {
    if [[ -L "$BIN_PATH" ]]; then printf 'a symlink to %s' "$(readlink "$BIN_PATH")"
    else printf 'an existing file (%s)' "$(file -b "$BIN_PATH" 2>/dev/null | cut -c1-40)"; fi
}

if [[ "$MODE" == "check" ]]; then
    printf 'CHECK no files changed\n'
    printf 'WOULD build: cargo build --release --manifest-path %s/Cargo.toml\n' "$ROOT_DIR"
    if foreign_bin; then
        printf 'WOULD displace: %s is %s -- not installed by this repo.\n' "$BIN_PATH" "$(describe_foreign)"
        printf '  It will be moved to %s.displaced-by-fleet-rs (kept, never clobbered),\n' "$BIN_PATH"
        printf '  because `fleet` on PATH must always be this CLI. Install elsewhere with:\n'
        printf '    FLEET_BIN_DIR=~/.local/bin/fleet-rs ./install.sh\n'
    else
        printf 'WOULD install: %s\n' "$BIN_PATH"
    fi
    printf 'WOULD create/chmod 700: %s/{runs,artifacts,attestations,ledger}\n' "$STATE_DIR"
    printf 'UNDO: ./install.sh --uninstall (preserves %s)\n' "$STATE_DIR"
    exit 0
fi

if [[ "$MODE" == "uninstall" ]]; then
    if foreign_bin; then
        printf 'fleet uninstall refused: %s is %s -- this repo did not install it.\n' \
            "$BIN_PATH" "$(describe_foreign)" >&2
        exit 7
    fi
    if [[ -e "$BIN_PATH" ]]; then
        rm -f "$BIN_PATH"
        printf 'CHANGE removed: %s\n' "$BIN_PATH"
    else
        printf 'UNCHANGED missing: %s\n' "$BIN_PATH"
    fi
    if [[ -e "$ZFUNC_FILE" ]]; then
        rm -f "$ZFUNC_FILE"
        printf 'CHANGE removed: %s\n' "$ZFUNC_FILE"
    else
        printf 'UNCHANGED missing: %s\n' "$ZFUNC_FILE"
    fi
    printf 'UNDO: rerun ./install.sh; preserved state at %s\n' "$STATE_DIR"
    exit 0
fi

printf 'CHANGE build: cargo build --release --manifest-path %s/Cargo.toml\n' "$ROOT_DIR"
cargo build --release --manifest-path "$ROOT_DIR/Cargo.toml" --bin fleet
if [[ ! -x "$BUILT_BIN" ]]; then
    fail_env "release build produced no executable at $BUILT_BIN"
fi

mkdir -p "$BIN_DIR"
# Owner decision (2026-09-09): `fleet` on PATH must ALWAYS be this CLI. Every build replaces
# whatever is there. Previously this refused and exited 7, which is how a stale symlink to the
# predecessor repo (`Principal Engineering/fleet/fleet`, dated Aug 24) kept shadowing this binary --
# so `fleet` in a shell ran the old project no matter how many times this repo was built.
# The displaced file is preserved ONCE at <path>.displaced-by-fleet-rs so nothing is lost; repeat
# installs will not clobber that backup with our own binary.
if foreign_bin; then
    BACKUP="${BIN_PATH}.displaced-by-fleet-rs"
    FOREIGN_DESC="$(describe_foreign)"   # probe BEFORE moving, or it describes a path that is gone
    if [[ ! -e "$BACKUP" ]]; then
        mv "$BIN_PATH" "$BACKUP"
        printf 'CHANGE displaced %s (%s) -> %s\n' "$BIN_PATH" "$FOREIGN_DESC" "$BACKUP"
    else
        rm -f "$BIN_PATH"
        printf 'CHANGE removed %s (%s); existing backup kept at %s\n' \
            "$BIN_PATH" "$FOREIGN_DESC" "$BACKUP"
    fi
fi

if [[ ! -e "$BIN_PATH" ]] || ! cmp -s "$BUILT_BIN" "$BIN_PATH"; then
    install -m 755 "$BUILT_BIN" "$BIN_PATH"
    printf 'CHANGE installed: %s (mode 755)\n' "$BIN_PATH"
else
    printf 'UNCHANGED binary already current: %s\n' "$BIN_PATH"
fi

mkdir -p "$STATE_DIR"
chmod 700 "$STATE_DIR"
printf 'CHANGE ensured mode 700: %s\n' "$STATE_DIR"
for child in runs artifacts attestations ledger; do
    if [[ ! -d "$STATE_DIR/$child" ]]; then
        mkdir -p "$STATE_DIR/$child"
        printf 'CHANGE created: %s/%s\n' "$STATE_DIR" "$child"
    else
        printf 'UNCHANGED exists: %s/%s\n' "$STATE_DIR" "$child"
    fi
    chmod 700 "$STATE_DIR/$child"
    printf 'CHANGE ensured mode 700: %s/%s\n' "$STATE_DIR" "$child"
done

case ":${PATH}:" in
    *":${BIN_DIR}:"*) printf 'PATH already contains %s\n' "$BIN_DIR" ;;
    *) printf 'PATH missing. Run exactly:\nexport PATH="%s:\$PATH"\n' "$BIN_DIR" ;;
esac

if [[ ! -d "$ZFUNC_DIR" ]]; then
    mkdir -p "$ZFUNC_DIR"
    printf 'CHANGE created: %s\n' "$ZFUNC_DIR"
fi
"$BIN_PATH" completions zsh > "$ZFUNC_FILE"
chmod 644 "$ZFUNC_FILE"
printf 'CHANGE wrote zsh completion: %s (mode 644)\n' "$ZFUNC_FILE"
if [[ -t 0 ]]; then
    read -r -p "Wire zsh completions into ~/.zshrc? [y/N] " answer
    if [[ "$answer" =~ ^[Yy]$ ]]; then
        if ! grep -Fq '# fleet completions (managed by fleet)' "${HOME}/.zshrc" 2>/dev/null; then
            {
                printf '\n# fleet completions (managed by fleet)\n'
                printf 'fpath=("%s" $fpath)\n' "$ZFUNC_DIR"
                printf 'autoload -Uz compinit && compinit\n'
            } >> "${HOME}/.zshrc"
            printf 'CHANGE appended zsh wiring: %s\n' "${HOME}/.zshrc"
            printf 'UNDO: remove the three lines under # fleet completions (managed by fleet)\n'
        else
            printf 'UNCHANGED zsh wiring already present: %s\n' "${HOME}/.zshrc"
        fi
    else
        printf 'UNCHANGED zshrc; activate manually with: fpath=("%s" \$fpath); autoload -Uz compinit && compinit\n' "$ZFUNC_DIR"
    fi
else
    printf 'OFFER zsh wiring skipped (non-interactive). Activate with: fpath=("%s" \$fpath); autoload -Uz compinit && compinit\n' "$ZFUNC_DIR"
fi

printf 'DONE fleet installed at %s\n' "$BIN_PATH"
printf 'UNDO: ./install.sh --uninstall; this preserves %s\n' "$STATE_DIR"
