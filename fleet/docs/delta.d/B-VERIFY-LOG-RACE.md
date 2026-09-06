# B-VERIFY-LOG-RACE — verify.sh's log path is shared, not per-caller

Found live, 2026-08-30 02:12 IST, while building `bin/morning-report.sh`: with `FLEET_MAXPAR=6`
now real, multiple codex workers each independently run `FLEET_MUTANTS=0 bash verify.sh` (per
their own brief rule 5) around the same time. `verify.sh` writes its detail log to a single fixed
path, `var/verify.log` — not parameterized by caller/PID. Confirmed empirically: read
`var/verify.log` mid-fanout and got a `FAIL` block for `N8`/`N9`/`Q1` acceptance rows that don't
match anything this session touched — almost certainly a different concurrent worker's run,
possibly interleaved with another's.

Impact: a worker deciding "is my change safe" by reading `var/verify.log` after running
`verify.sh` itself can read a DIFFERENT worker's result, or a torn/interleaved mix of both —
exactly the shared-mutable-state hazard C-conventions warn about, just in a log file instead of
a data file.

Not fixed here — this needs `verify.sh` itself to write to a PID- or caller-scoped path (e.g.
`var/verify-$$.log` or accept a `--log` override) and callers to read back only their own file.
That's a real, scoped fix to `verify.sh`, risky to make blind while 6 real workers are using it
concurrently right now. Left as a backlog item; `bin/morning-report.sh` was patched to detect a
concurrent `verify.sh` and warn rather than present possibly-stale content as fact.
