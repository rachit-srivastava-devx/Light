#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'B4 a mutation score was reported without reproducible prerequisites'
printf '%s\n' 'NOT MECHANISABLE: historical, runtime, attribution, or human-judgement failure; no reliable tree mechanism.'
exit 77

