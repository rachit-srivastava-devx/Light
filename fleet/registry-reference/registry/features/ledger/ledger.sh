#!/usr/bin/env bash
# shellcheck disable=SC1010
# ledger.sh — append-only receipt-backed human ledger.
# Assumes BACKLOG.md rows are "- [status] id | title"; IDs are ASCII tokens.
# Does not handle literal newlines in titles or non-atomic mkdir filesystems.
set -u
D="$(cd "$(dirname "$0")/../../.." && pwd)"
. "$D/registry/lib/toon.sh"; . "$D/registry/lib/receipt.sh"; . "$D/registry/lib/err.sh"
BACKLOG="${FLEET_BACKLOG:-$D/ledger/BACKLOG.md}"; RECEIPTS="${FLEET_LEDGER:-$D/ledger/RECEIPTS.jsonl}"
NOW="${FLEET_NOW:-}"; JSON=0; FULL=0; LOCK_HELD=0
cleanup(){ [ "$LOCK_HELD" -eq 1 ] && rmdir "${BACKLOG}.lock" 2>/dev/null || true; LOCK_HELD=0; }
trap cleanup EXIT HUP INT TERM
usage(){ printf '%s\n' \
 'ledger.sh [--json] [--full] [--now ISO] [add|set-status|show|verify|report|metrics]' \
 '  (none)                         live feature dashboard' \
 '  add ID TITLE [--status STATUS] append a feature and receipt' \
 '  set-status ID STATUS           append a status correction' \
 '  show ID                        show one feature' \
 '  verify                         verify the complete receipt chain' \
 '  report --html                  render a self-contained report' \
 '  metrics                        report measured receipt metrics'; }
now_iso(){ [ -n "$NOW" ] && printf '%s' "$NOW" || date -u +%Y-%m-%dT%H:%M:%SZ; }
valid_status(){ case "${1-}" in done|pending|failed|needs-iteration) return 0;; *) return 1;; esac; }
valid_id(){ case "${1-}" in ''|*[!A-Za-z0-9._-]*) return 1;; *) return 0;; esac; }
require_paths(){ require_bin jq 'install jq and retry'; require_bin shasum 'install shasum and retry'; }
acquire_lock(){ local n=0; mkdir -p "$(dirname "$BACKLOG")"; while ! mkdir "${BACKLOG}.lock" 2>/dev/null; do n=$((n+1)); [ "$n" -ge 100 ] && die "$ERR_GENERIC" ledger_locked 'could not acquire backlog lock' 'retry after the concurrent writer finishes'; sleep 0.1; done; LOCK_HELD=1; }
ensure_backlog(){ if [ ! -e "$BACKLOG" ]; then mkdir -p "$(dirname "$BACKLOG")"; printf '%s\n' '# fleet ledger' > "$BACKLOG"; fi; [ -f "$BACKLOG" ] || die "$ERR_USAGE" backlog_not_file 'backlog path is not a regular file' 'set FLEET_BACKLOG to a markdown file'; }
validate_backlog(){
  [ -f "$BACKLOG" ] || return 0
  awk 'function bad(m){print "backlog: " m > "/dev/stderr";exit 2}
    /^- \[/ {c=index($0,"]");s=index(substr($0,c+2)," | ");if(c<4||s==0)bad("malformed feature line " FNR);
    st=substr($0,4,c-4);r=substr($0,c+2);id=substr(r,1,s-1);
    if(st !~ /^(done|pending|failed|needs-iteration)$/)bad("invalid status at line " FNR);
    if(id !~ /^[A-Za-z0-9._-]+$/)bad("invalid feature id at line " FNR);
    if(seen[id]++)bad("duplicate feature id " id)}' "$BACKLOG" ||
    die "$ERR_PARSE" backlog_unparseable 'backlog contains an invalid or duplicate feature row' 'repair the markdown row and retry'
}
chain_or_exit(){ FLEET_LEDGER="$RECEIPTS" receipt_verify_chain >/dev/null; local ec=$?; [ "$ec" -eq 0 ] || exit "$ERR_CHAIN"; }
receipt_map(){ local out="$1"; : > "$out"; [ -s "$RECEIPTS" ] || return 0; jq -r 'select((.event|type)=="string" and (.event|startswith("ledger_"))) | [.task_id,(input_line_number|tostring)] | @tsv' "$RECEIPTS" > "$out" 2>/dev/null || die "$ERR_PARSE" receipt_unparseable 'receipt JSONL could not be parsed' 'repair the receipt source before reading the ledger'; }
feature_data(){
  local map="$1" out="$2"
  [ -f "$BACKLOG" ] || { : > "$out"; return 0; }
  awk -v map="$map" 'FILENAME==map{latest[$1]=$2;next}
    /^- \[/ {c=index($0,"]");s=index(substr($0,c+2)," | ");st=substr($0,4,c-4);r=substr($0,c+2);id=substr(r,1,s-1);title=substr(r,s+3);print st "\t" id "\t" title "\t" FNR "\t" (latest[id]==""?"unrecorded":latest[id])}' "$map" "$BACKLOG" > "$out"
}
line_for_hash(){ local h="$1"; [ -s "$RECEIPTS" ] || { printf 0; return; }; grep -n '"hash":"'"$h"'"' "$RECEIPTS" | head -1 | cut -d: -f1; }
append_receipt(){ local ts="$1" event="$2" id="$3" ec="${4:-0}" h line; h="$(FLEET_LEDGER="$RECEIPTS" receipt_append "$ts" "$event" ledger "$id" not-applicable not-applicable "$ec" 0 0 0)" || die "$ERR_GENERIC" receipt_failed 'could not append the ledger receipt' 'repair the receipt path and retry'; line="$(line_for_hash "$h")"; [ -n "$line" ] || die "$ERR_GENERIC" receipt_line_missing 'receipt was appended but its line id was not found' 'inspect the receipt chain'; printf '%s' "$line"; }
dashboard(){
  local ts m f n d p x i u ev
  ts="$(now_iso)"; require_paths; validate_backlog; chain_or_exit
  m="$(mktemp "${TMPDIR:-/tmp}/ledger-map.XXXXXX")"; f="$(mktemp "${TMPDIR:-/tmp}/ledger-data.XXXXXX")"; receipt_map "$m"; feature_data "$m" "$f"
  n="$(wc -l < "$f" | tr -d ' ')"; d="$(awk -F '\t' '$1=="done"{n++}END{print n+0}' "$f")"; p="$(awk -F '\t' '$1=="pending"{n++}END{print n+0}' "$f")"; x="$(awk -F '\t' '$1=="failed"{n++}END{print n+0}' "$f")"; i="$(awk -F '\t' '$1=="needs-iteration"{n++}END{print n+0}' "$f")"; u="$(awk -F '\t' '$5=="unrecorded"{n++}END{print n+0}' "$f")"
  if [ "$JSON" -eq 1 ]; then
    jq -Rn --argjson count "$n" --argjson done "$d" --argjson pending "$p" --argjson failed "$x" --argjson iterate "$i" --argjson unrecorded "$u" '[inputs|split("\t")|{id:.[1],status:.[0],title:.[2],backlog_line:(.[3]|tonumber),receipt_line:(if .[4]=="unrecorded" then .[4] else (.[4]|tonumber) end)}] as $features | {features:$features,summary:{count:$count,done:$done,pending:$pending,failed:$failed,needs_iteration:$iterate,unrecorded:$unrecorded}}' < "$f"
    rm -f "$m" "$f"; return 0
  fi
  toon_preamble ledger 'append-only feature ledger' "$ts"
  if [ "$n" -eq 0 ]; then toon_empty features; else toon_open features "$n" 'id,status,title,receipt_line'; while IFS='	' read -r st id title bl ev; do [ -n "$id" ] || continue; [ "$FULL" -eq 1 ] && t="$title" || t="${title:0:120}"; toon_row "$id" "$st" "$t" "$ev"; done < "$f"; fi
  if [ "$n" -gt 0 ] && [ "$u" -eq 0 ]; then ev="$(awk -F '\t' 'BEGIN{a=0;b=0}$5!="unrecorded"{z=$5+0;if(a==0||z<a)a=z;if(z>b)b=z}END{if(a==0)print "none";else if(a==b)print a;else print a "-" b}' "$f")"; toon_open summary 1 'features,done,pending,failed,needs_iteration,receipt_lines'; toon_row "$n" "$d" "$p" "$x" "$i" "$ev"; else toon_empty summary; fi
  if [ "$u" -gt 0 ]; then toon_open attention 1 'rows,reason,remedy'; toon_row "$u" 'unrecorded feature rows' 'use ledger.sh add or set-status to create receipt evidence'; fi
  toon_next 'ledger.sh set-status ID STATUS' 'ledger.sh verify' 'ledger.sh metrics'; rm -f "$m" "$f"
}
add_feature(){
  local id="$1" title="$2" st=pending ts line; shift 2
  while [ "$#" -gt 0 ]; do case "$1" in --status) need_val --status "${2-}"; [ "$#" -ge 2 ] || die "$ERR_USAGE" missing_status 'missing --status value' 'use one of the four statuses';st="$2";shift 2;; --json) need_val --json "${2-}";JSON=1;shift;; --now) need_val --now "${2-}";[ "$#" -ge 2 ]||die "$ERR_USAGE" missing_now 'missing --now value' 'pass an ISO timestamp';NOW="$2";shift 2;; -*)reject_unknown_flag "$1";;*)die "$ERR_USAGE" unexpected_argument "unexpected argument '$1'" 'run ledger.sh --help';;esac;done
  valid_id "$id" || die "$ERR_USAGE" invalid_id 'feature ID is invalid' 'choose a stable ASCII ID'; valid_status "$st" || die "$ERR_USAGE" invalid_status 'status must be one of the four' 'use done,pending,failed,needs-iteration'
  acquire_lock;ensure_backlog;validate_backlog
  awk -v w="$id" '/^- \[/{c=index($0,"]");s=index(substr($0,c+2)," | ");r=substr($0,c+2);if(substr(r,1,s-1)==w)f=1}END{exit f?0:1}' "$BACKLOG" && die "$ERR_USAGE" duplicate_feature "feature ID '$id' already exists" 'use set-status for a correction'
  line="$(append_receipt "$(now_iso)" "ledger_add:$st" "$id")"; printf '%s\n' "- [$st] $id | $title" >> "$BACKLOG"; if [ "$JSON" -eq 1 ]; then jq -cn --arg id "$id" --arg status "$st" --arg line "$line" '{ok:true,operation:"add",id:$id,status:$status,receipt_line:($line|tonumber)}'; else printf 'added,%s,%s,receipt_line=%s\n' "$id" "$st" "$line"; fi
}
set_status(){
  local id="$1" st="$2" old line; valid_id "$id"||die "$ERR_USAGE" invalid_id 'feature ID is invalid' 'choose a stable ASCII ID';valid_status "$st"||die "$ERR_USAGE" invalid_status 'status must be one of the four' 'use done,pending,failed,needs-iteration'
  acquire_lock;ensure_backlog;validate_backlog;old="$(awk -v w="$id" '/^- \[/{c=index($0,"]");s=index(substr($0,c+2)," | ");q=substr($0,c+2);if(substr(q,1,s-1)==w)print substr($0,4,c-4)}' "$BACKLOG")";[ -n "$old" ]||die "$ERR_USAGE" feature_missing "feature ID '$id' not found" 'use ledger.sh add first'
  [ "$old" = "$st" ] && { if [ "$JSON" -eq 1 ]; then jq -cn --arg id "$id" --arg status "$st" '{ok:true,operation:"set-status",id:$id,status:$status,changed:false}'; else printf 'unchanged,%s,%s\n' "$id" "$st"; fi; return; }
  line="$(append_receipt "$(now_iso)" "ledger_status:$old:$st" "$id")"; require_bin python3 'install python3 and retry'
  python3 - "$BACKLOG" "$id" "$st" <<'PY'
import sys
path, wanted, new_status = sys.argv[1:]
raw = open(path, 'rb').read()
lines = raw.splitlines(True)
matches = 0
out = []
for line in lines:
    text = line.decode('utf-8')
    if text.startswith('- [') and '] ' in text:
        close = text.find(']')
        rest = text[close + 2:]
        sep = rest.find(' | ')
        if sep >= 0 and rest[:sep] == wanted:
            matches += 1
            line = ('- [' + new_status + '] ' + rest).encode('utf-8')
    out.append(line)
if matches != 1:
    raise SystemExit(2)
open(path, 'wb').write(b''.join(out))
PY
  if [ "$JSON" -eq 1 ]; then jq -cn --arg id "$id" --arg status "$st" --arg line "$line" '{ok:true,operation:"set-status",id:$id,status:$status,changed:true,receipt_line:($line|tonumber)}'; else printf 'updated,%s,%s,receipt_line=%s\n' "$id" "$st" "$line"; fi
}
show_feature(){ local id="$1" m f row; valid_id "$id"||die "$ERR_USAGE" invalid_id 'feature ID is invalid' 'choose a stable ASCII ID';ensure_backlog;validate_backlog;chain_or_exit;m="$(mktemp "${TMPDIR:-/tmp}/ledger-map.XXXXXX")";f="$(mktemp "${TMPDIR:-/tmp}/ledger-data.XXXXXX")";receipt_map "$m";feature_data "$m" "$f";row="$(awk -F '\t' -v w="$id" '$2==w{print;ok=1}END{exit ok?0:1}' "$f"||true)";[ -n "$row" ]||die "$ERR_USAGE" feature_missing "feature ID '$id' not found" 'use ledger.sh add first';toon_preamble ledger 'single feature' "$(now_iso)";toon_open feature 1 'id,status,title,backlog_line,receipt_line';IFS='	' read -r st fid title bl ev <<EOF
$row
EOF
toon_row "$fid" "$st" "$title" "$bl" "$ev";toon_next 'ledger.sh set-status ID STATUS';rm -f "$m" "$f"; }
render_html(){ local m f n r st id title bl ev esc ts; ts="$(now_iso)";require_paths;validate_backlog;chain_or_exit;m="$(mktemp "${TMPDIR:-/tmp}/ledger-map.XXXXXX")";f="$(mktemp "${TMPDIR:-/tmp}/ledger-data.XXXXXX")";receipt_map "$m";feature_data "$m" "$f";n="$(wc -l<"$f"|tr -d ' ')";r=0;[ -s "$RECEIPTS" ]&&r="$(wc -l<"$RECEIPTS"|tr -d ' ')";cat <<EOF
<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>fleet ledger</title><style>:root{color-scheme:light dark;--bg:#fff;--fg:#17202a;--muted:#68737d;--card:#eef2f5;--done:#18794e;--pending:#9a6700;--failed:#b42318;--iterate:#7a3e9d}@media(prefers-color-scheme:dark){:root{--bg:#11161b;--fg:#eef2f5;--muted:#aab4bd}}body{background:var(--bg);color:var(--fg);font:16px system-ui,sans-serif;margin:2rem}table{border-collapse:collapse;width:100%}th,td{padding:.6rem;text-align:left;border-bottom:1px solid var(--muted)}.status{font-weight:700}.done{color:var(--done)}.pending{color:var(--pending)}.failed{color:var(--failed)}.needs-iteration{color:var(--iterate)}.meta{color:var(--muted)}</style></head><body><h1>fleet ledger</h1><p class="meta">features=$n; receipt_lines=$r; generated=$ts</p><table><tr><th>ID</th><th>Status</th><th>Title</th><th>Receipt line</th></tr>
EOF
while IFS='	' read -r st id title bl ev; do esc="$(printf '%s' "$title"|sed 's/&/\&amp;/g;s/</\&lt;/g;s/>/\&gt;/g;s/"/\&quot;/g')";printf '<tr><td>%s</td><td class="status %s">%s</td><td>%s</td><td>%s</td></tr>\n' "$id" "$st" "$st" "$esc" "$ev";done<"$f";printf '</table></body></html>\n';rm -f "$m" "$f"; }
wilson_ci(){
  python3 - "$1" "$2" <<'PY'
from decimal import Decimal, getcontext
import sys
getcontext().prec = 40
k = Decimal(int(sys.argv[1])); n = Decimal(int(sys.argv[2])); z = Decimal('1.96')
p = k / n; d = 1 + z*z/4/n
u = (p + z*z/2/n) / d
v = z * (p*(1-p)/n + z*z/4/(n*n)).sqrt() / d
print(int(max(Decimal(0), u-v)*10000), int(min(Decimal(1), u+v)*10000))
PY
}
metrics(){
  local s inv ir mut kill mr acc cost ar ratio ci low high total_lines
  require_paths;chain_or_exit;[ -s "$RECEIPTS" ] || { toon_preamble ledger 'measured receipt metrics' "$(now_iso)"; toon_empty metrics; toon_next 'ledger.sh add ID TITLE'; return; }
  total_lines="$(wc -l < "$RECEIPTS" | tr -d ' ')"
  s="$(jq -r '[(input_line_number|tostring),(.exit_code|tostring),(.event//""),(.cost_micro_usd|tostring)]|@tsv' "$RECEIPTS"|awk -F '\t' 'function rg(a,b){if(a=="")return "none";if(a==b)return a;return a "-" b}$2=="6"{i++;if(ir==""||$1<ir)ir=$1;if($1>ix)ix=$1}$3=="mutation"{m++;if($2!="0")k++;if(mr==""||$1<mr)mr=$1;if($1>mx)mx=$1}$3=="change"&&$2=="0"{a++;c+=$4;if(ar==""||$1<ar)ar=$1;if($1>ax)ax=$1}END{printf "%d\t%s\t%d\t%d\t%s\t%d\t%d\t%s\n",i,rg(ir,ix),m,k,rg(mr,mx),a,c,rg(ar,ax)}')"
  IFS='	' read -r inv ir mut kill mr acc cost ar <<EOF
$s
EOF
  [ "$ir" = none ] && ir="1-$total_lines"; toon_preamble ledger 'measured receipt metrics' "$(now_iso)";toon_open metrics 3 'metric,value,evidence';toon_row invariant_violations "$inv" "receipt_lines=$ir"
  if [ "$mut" -eq 0 ]; then
    toon_row mutation_kill_rate empty receipt_lines=none
  else
    ci="$(wilson_ci "$kill" "$mut")"; low="${ci%% *}"; high="${ci##* }"
    toon_row mutation_kill_rate "$kill/$mut;ci95_wilson_bp=[$low,$high]" "receipt_lines=$mr"
  fi
  if [ "$acc" -eq 0 ]; then
    toon_row cost_per_accepted_change empty receipt_lines=none
  else
    ratio=$((cost/acc)); toon_row cost_per_accepted_change "$ratio" "micro_usd;accepted_change_receipt_lines=$ar"
  fi
  toon_next 'ledger.sh report --html' 'ledger.sh verify'
}
main(){
 local cmd="" a="" b=""
 while [ "$#" -gt 0 ];do case "$1" in --help|-h)usage;return;;--json) need_val --json "${2-}";JSON=1;shift;;--full) need_val --full "${2-}";FULL=1;shift;;--now) need_val --now "${2-}";[ "$#" -ge 2 ]||die "$ERR_USAGE" missing_now 'missing --now value' 'pass an ISO timestamp';NOW="$2";shift 2;;--*)reject_unknown_flag "$1";;*)cmd="$1";shift;break;;esac;done
 case "$cmd" in '')dashboard;;add)[ "$#" -ge 2 ]||die "$ERR_USAGE" add_args 'add requires ID and TITLE' 'ledger.sh add ID TITLE';a="$1";b="$2";shift 2;add_feature "$a" "$b" "$@";;set-status)[ "$#" -eq 2 ]||die "$ERR_USAGE" status_args 'set-status requires ID and STATUS' 'ledger.sh set-status ID STATUS';set_status "$1" "$2";;show)[ "$#" -eq 1 ]||die "$ERR_USAGE" show_args 'show requires ID' 'ledger.sh show ID';show_feature "$1";;verify)[ "$#" -eq 0 ]||die "$ERR_USAGE" verify_args 'verify takes no arguments' 'ledger.sh verify';require_paths;FLEET_LEDGER="$RECEIPTS" receipt_verify_chain;;delete)die "$ERR_USAGE" append_only 'deletion is refused; receipts and corrections are append-only' 'use set-status to supersede a feature';;report)[ "$#" -eq 1 ]&&[ "$1" = --html ]||[ "$#" -eq 0 ]||reject_unknown_flag "$1";render_html;;metrics)[ "$#" -eq 0 ]||reject_unknown_flag "$1";metrics;;*)die "$ERR_USAGE" unknown_subcommand "unknown subcommand '$cmd'" 'run ledger.sh --help';;esac
}
main "$@"
