#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'S1 refusal paths emitted no receipt'
printf '%s\n' 'NOT MECHANISABLE: historical, runtime, attribution, or human-judgement failure; no reliable tree mechanism.'
exit 77

