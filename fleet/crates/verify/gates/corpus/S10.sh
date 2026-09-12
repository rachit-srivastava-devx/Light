#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'S10 an acceptance test drove a synthetic harness instead of the real entrypoint'
printf '%s\n' 'NOT MECHANISABLE: historical, runtime, attribution, or human-judgement failure; no reliable tree mechanism.'
exit 77

