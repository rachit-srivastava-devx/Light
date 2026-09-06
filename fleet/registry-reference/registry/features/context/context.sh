#!/usr/bin/env bash
# context.sh — thin adapter over tiktoken (exact token counts) and LLMLingua-2 (real prompt
# compression). Replaces bin/compress.sh + bin/compress2.sh (ADOPT.md §2 "prompt compression"
# row, 780 lines of a chars/4 heuristic presented AS a token count). This file authors no
# counting or compression logic of its own: it shells out to `.venv/bin/python` — never the
# system python3, which already broke fleet's property-test lane once — and calls tiktoken /
# llmlingua directly.
#
# HONESTY CONTRACT, the reason this file exists: every count is labelled with the method that
# produced it. tiktoken gives an EXACT count for OpenAI/Codex-family models. There is no public,
# local, exact tokenizer for Claude, so a claude*/opus*/sonnet*/haiku* --model is labelled
# "estimated" with the heuristic named (chars/4) — never silently reported as exact. The same
# honesty applies to a --model tiktoken has never heard of: counted with o200k_base as the
# nearest known encoding, and labelled estimated, not exact.
#
# LLMLingua model: microsoft/llmlingua-2-bert-base-multilingual-cased-meetingbank — the SMALLEST
# LLMLingua-2 checkpoint, not the library's own default (NousResearch/Llama-2-7b-hf, a gated 7B
# model that wants CUDA and would not run on this Mac at all). Measured on this host 2026-08-22:
# first pull ~45.5s + ~1.1GB peak RSS (one-time; HF caches it under ~/.cache/huggingface after);
# cached reload ~1.5s, compression itself ~1.5s. device_map=cpu (no CUDA here; mps was not
# attempted — cpu already clears budget and this machine has wedged twice this week under
# concurrent load, so a second variable was not worth adding). Never call this from two
# processes at once for that reason.
#
# Usage: context.sh count <file|-> [--model NAME]
#        context.sh compress <file|-> --target N [--model NAME] [--out FILE]
#        context.sh check <file> --limit N [--model NAME]
# Exit:  0 ok · 2 usage · 3 tool missing · 4 unparseable · 5 budget · 6 invariant (registry/lib/err.sh)
set -u
D="$(cd "$(dirname "$0")/../../.." && pwd)"
# shellcheck source=../../registry/lib/err.sh
. "$D/registry/lib/err.sh"; . "$D/registry/lib/receipt.sh"; . "$D/registry/lib/toon.sh"

PYBIN="${FLEET_CONTEXT_PYBIN:-$D/var/venv/bin/python}"   # override lets tests inject a stub interpreter
[ -x "$PYBIN" ] || die "$ERR_NOTOOL" venv_missing "$PYBIN not found" 'run: ./fleet setup (uv venv at fleet/var/venv)'

LLMLINGUA_MODEL="${FLEET_LLMLINGUA_MODEL:-microsoft/llmlingua-2-bert-base-multilingual-cased-meetingbank}"
MODEL_DEFAULT="${FLEET_CONTEXT_MODEL:-gpt-4o}"
GEN="not-applicable"; VER="not-applicable"   # internal tokeniser/compressor op: no model generates or verifies it   # I1: both named/distinct; this adapter makes no LLM call itself.
now_iso() { [ -n "${NOW:-}" ] && printf '%s' "$NOW" || date -u +%Y-%m-%dT%H:%M:%SZ; }
is_uint() { case "${1-}" in ''|*[!0-9]*) return 1 ;; esac; return 0; }
_receipt() { receipt_append "$(now_iso)" "$1" context "$2" "$GEN" "$VER" "$3" "${4:-0}" "${5:-0}" 0 >/dev/null 2>&1 || true; }
require_py() { "$PYBIN" -c "import $1" >/dev/null 2>&1 || die "$ERR_NOTOOL" module_missing "python module '$1' not importable via $PYBIN" "run: ./fleet setup --only $1"; }

usage() {
  printf '%s\n' \
    'context.sh count <file|-> [--model NAME]                exact tiktoken count; estimated for claude*' \
    'context.sh compress <file|-> --target N [--model NAME] [--out FILE]   LLMLingua-2 to a token budget' \
    'context.sh check <file> --limit N [--model NAME]        exit 5 over budget; reports the real overage' \
    "  NAME default: $MODEL_DEFAULT (override: FLEET_CONTEXT_MODEL)"
}
# _read_src <path|-> <dest_file> — never inside a pipeline: die() here must be able to exit
# the whole script, and the left side of a `|` runs in a subshell that would swallow it.
_read_src() {
  if [ "$1" = - ]; then cat > "$2"
  else [ -f "$1" ] || die "$ERR_USAGE" file_missing "file not found: $1" 'pass an existing file or - for stdin'
    cat -- "$1" > "$2"
  fi
}

# shellcheck disable=SC2016
COUNT_PY='
import json, sys
model, path = sys.argv[1], sys.argv[2]
text = open(path).read()
if any(m in model.lower() for m in ("claude", "opus", "sonnet", "haiku", "anthropic")):
    n = -(-len(text) // 4)
    print(json.dumps({"tokens": n, "method": "estimated", "tokenizer": "heuristic-chars-per-4",
                       "reason": "Anthropic tokenizer is closed-source; no local exact counter"}))
else:
    import tiktoken
    reason = ""
    try:
        enc, method = tiktoken.encoding_for_model(model), "exact"
    except KeyError:
        enc = tiktoken.get_encoding("o200k_base"); method = "estimated"
        reason = "model not in tiktoken " + tiktoken.__version__ + " registry; used o200k_base"
    print(json.dumps({"tokens": len(enc.encode(text)), "method": method, "tokenizer": enc.name,
                       "reason": reason}))
'
# _measure <src> <model> — sets TOKENS METHOD TOKENIZER REASON; dies (exit 3/4/6) on failure.
_measure() {
  local src="$1" model="$2" tmp json ec
  tmp="$(mktemp "${TMPDIR:-/tmp}/fleet-context-txt.XXXXXX")" || die "$ERR_GENERIC" temp_failed 'could not create a scratch file' 'check the temporary directory'
  _read_src "$src" "$tmp"
  require_py tiktoken
  json="$("$PYBIN" -c "$COUNT_PY" "$model" "$tmp")"; ec=$?
  rm -f "$tmp"
  [ "$ec" -eq 0 ] && [ -n "$json" ] || die "$ERR_PARSE" count_failed "tiktoken helper failed (exit $ec)" 'run: var/venv/bin/python -c "import tiktoken"'
  TOKENS="$(printf '%s' "$json" | jq -r '.tokens')"
  is_uint "$TOKENS" || die "$ERR_INVARIANT" i3_violated "token count '$TOKENS' is not an integer" "inspect: $json"
  METHOD="$(printf '%s' "$json" | jq -r '.method')"; TOKENIZER="$(printf '%s' "$json" | jq -r '.tokenizer')"
  REASON="$(printf '%s' "$json" | jq -r '.reason')"
}

cmd="${1:-}"; shift || true
case "$cmd" in
  count)
    src="${1:-}"; [ -n "$src" ] || die "$ERR_USAGE" no_path 'count requires a file or -' 'context.sh count <file|->'
    shift || true
    model="$MODEL_DEFAULT"
    while [ $# -gt 0 ]; do case "$1" in
      --model) need_val --model "${2-}"; model="$2"; shift 2 ;;
      -h|--help) usage; exit 0 ;;
      *) reject_unknown_flag "$1" ;;
    esac; done
    _measure "$src" "$model"
    _receipt count "$src" 0 "$TOKENS" "$TOKENS"
    toon_preamble context "exact token count (tiktoken) or a named, honest estimate" "$(now_iso)"
    toon_open tokens 1 'tokens,method,tokenizer,model,file,reason'
    toon_row "$TOKENS" "$METHOD" "$TOKENIZER" "$model" "$src" "$REASON"
    ;;
  compress)
    src="${1:-}"; [ -n "$src" ] || die "$ERR_USAGE" no_path 'compress requires a file or -' 'context.sh compress <file|-> --target N'
    shift || true
    target=""; model="$MODEL_DEFAULT"; out=""
    while [ $# -gt 0 ]; do case "$1" in
      --target) need_val --target "${2-}"; target="$2"; shift 2 ;;
      --model) need_val --model "${2-}"; model="$2"; shift 2 ;;
      --out) need_val --out "${2-}"; out="$2"; shift 2 ;;
      -h|--help) usage; exit 0 ;;
      *) reject_unknown_flag "$1" ;;
    esac; done
    { [ -n "$target" ] && is_uint "$target" && [ "$target" -gt 0 ]; } || die "$ERR_USAGE" bad_target '--target must be a positive integer token count' 'context.sh compress FILE --target 500'
    require_py tiktoken; require_py llmlingua
    tmp_in="$(mktemp "${TMPDIR:-/tmp}/fleet-context-in.XXXXXX")"; tmp_meta="$(mktemp "${TMPDIR:-/tmp}/fleet-context-meta.XXXXXX")"; tmp_err="$(mktemp "${TMPDIR:-/tmp}/fleet-context-err.XXXXXX")"
    trap 'rm -f "$tmp_in" "$tmp_meta" "$tmp_err"' EXIT HUP INT TERM
    _read_src "$src" "$tmp_in"
    out_target="${out:-/dev/stdout}"
    # shellcheck disable=SC2016
    "$PYBIN" -c '
import json, sys
target, model, meta_path, llm_model, path = int(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]
text = open(path).read()
import tiktoken
try:
    enc = tiktoken.encoding_for_model(model)
except KeyError:
    enc = tiktoken.get_encoding("o200k_base")
before = len(enc.encode(text))
from llmlingua import PromptCompressor
comp = PromptCompressor(model_name=llm_model, use_llmlingua2=True, device_map="cpu")
ask, best_text, best_count = target, text, before
for _ in range(4):
    got = comp.compress_prompt(text, target_token=max(1, ask), chunk_end_tokens=[".", "\n"],
                                force_tokens=["\n", "."], drop_consecutive=True)
    actual = len(enc.encode(got["compressed_prompt"]))
    if actual < best_count:
        best_text, best_count = got["compressed_prompt"], actual
    if actual <= target:
        break
    ask = max(1, ask - (actual - target) - max(5, int(target * 0.02)))
sys.stdout.write(best_text)
json.dump({"before": before, "after": best_count, "met": best_count <= target, "tokenizer": enc.name},
          open(meta_path, "w"))
' "$target" "$model" "$tmp_meta" "$LLMLINGUA_MODEL" "$tmp_in" > "$out_target" 2>"$tmp_err"
    ec=$?
    if [ "$ec" -ne 0 ] || [ ! -s "$tmp_meta" ]; then
      _receipt compress "$src" "$ERR_PARSE"
      die "$ERR_PARSE" compress_failed "llmlingua helper failed (exit $ec): $(tail -c 300 "$tmp_err")" 'run: var/venv/bin/python -c "import llmlingua"'
    fi
    before="$(jq -r '.before' "$tmp_meta")"; after="$(jq -r '.after' "$tmp_meta")"
    met="$(jq -r '.met' "$tmp_meta")"; tokenizer="$(jq -r '.tokenizer' "$tmp_meta")"
    { is_uint "$before" && is_uint "$after"; } || { _receipt compress "$src" "$ERR_INVARIANT"; die "$ERR_INVARIANT" i3_violated "before/after must be integers ($before/$after)" "inspect: $(cat "$tmp_meta")"; }
    ratio="$(awk -v b="$before" -v a="$after" 'BEGIN{ if (a>0) printf "%.2f", b/a; else print "inf" }')"
    _receipt compress "$src" 0 "$before" "$after"
    dest_label="${out:-stdout}"
    summary="$( { toon_preamble context "LLMLingua-2 compression, measured against a real tokenizer" "$(now_iso)"
      toon_open compression 1 'before,after,target,met,ratio,tokenizer,dest'
      toon_row "$before" "$after" "$target" "$met" "${ratio}x" "$tokenizer" "$dest_label"; } )"
    if [ -n "$out" ]; then printf '%s\n' "$summary"; else printf '%s\n' "$summary" >&2; fi
    ;;
  check)
    src="${1:-}"; [ -n "$src" ] || die "$ERR_USAGE" no_path 'check requires a file' 'context.sh check FILE --limit N'
    shift || true
    limit=""; model="$MODEL_DEFAULT"
    while [ $# -gt 0 ]; do case "$1" in
      --limit) need_val --limit "${2-}"; limit="$2"; shift 2 ;;
      --model) need_val --model "${2-}"; model="$2"; shift 2 ;;
      -h|--help) usage; exit 0 ;;
      *) reject_unknown_flag "$1" ;;
    esac; done
    { [ -n "$limit" ] && is_uint "$limit"; } || die "$ERR_USAGE" bad_limit '--limit must be a non-negative integer' 'context.sh check FILE --limit 100000'
    _measure "$src" "$model"
    toon_preamble context "budget check against an exact/estimated token count" "$(now_iso)"
    toon_open budget 1 'tokens,limit,status,overage'
    if [ "$TOKENS" -gt "$limit" ]; then
      _receipt check "$src" "$ERR_BUDGET" "$TOKENS" "$TOKENS"
      toon_row "$TOKENS" "$limit" over "$((TOKENS - limit))"
      exit "$ERR_BUDGET"
    fi
    _receipt check "$src" 0 "$TOKENS" "$TOKENS"
    toon_row "$TOKENS" "$limit" ok 0
    ;;
  -h|--help|"") usage; exit 0 ;;
  *) die "$ERR_USAGE" unknown_subcommand "unknown subcommand '$cmd'" 'context.sh {count|compress|check}' ;;
esac
