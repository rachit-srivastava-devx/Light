#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'T17 a slow check was treated as a skipped check'
printf '%s\n' 'NOT MECHANISABLE: historical, runtime, attribution, or human-judgement failure; no reliable tree mechanism.'
exit 77

