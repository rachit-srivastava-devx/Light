# Cost reservation overspend race — verification evidence

Date: 2026-09-03

## Defect

`SessionMeter.settle()` rejected an 11-paise settlement against a 10-paise hold, but raised before
consuming the hold. The observable state after the failed settlement was therefore
`spent_paise=0`, `reserved_paise=10`, and an empty ledger. That stale hold could never be settled
or released by the route, because both production callers translate the exception directly to HTTP
402.

## Contract

`blueprints/ADHD-Focus-Orb-L8-Deep-Dive/08-COST-MODEL.md` section 6 requires in-path,
fail-closed reservation; `docs/BUILD-DIGEST.md` section 4 explicitly forbids post-hoc cost
enforcement. A settlement must atomically do one of two things:

1. consume the hold and record an admitted cost no greater than the hold; or
2. consume the hold, record no spend, and reject an oversized settlement.

## Fix

All `SessionMeter` balance reads and mutations now use one per-meter re-entrant lock.
`settle()` consumes its hold inside the same critical section that either records admitted spend
or rejects an overrun. The exact 10-reserved/11-settled regression now proves the error leaves
`spent_paise=0`, `reserved_paise=0`, and no ledger entry.

## Evidence

- `red.txt`: failing-first regression against the prior implementation.
- `focused-green.txt`: exact regression after the fix.
- `relay-py-pytest.txt`: full `backend/relay-py` pytest suite, exit 0.
- `repository-verify.txt`: required repository gate attempt. It is red in the pre-existing
  gateway-sidecar file dependency/typecheck baseline; the Python backend gate had already passed.

Focused static checks:

```text
$ uv run --project backend/relay-py --extra dev ruff check backend/relay-py/src/orb_relay/cost/meter.py backend/relay-py/tests/test_cost_meter.py
All checks passed!

$ uv run --project backend/relay-py --extra dev mypy backend/relay-py/src/orb_relay/cost/meter.py
Success: no issues found in 1 source file
```
