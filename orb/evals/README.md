# evals/ — Track E (anti-mirage)

Own isolated Python environment: `evals/.venv` (python3.12), never `backend/relay-py/.venv`.

```
python3.12 -m venv evals/.venv
evals/.venv/bin/pip install -r evals/requirements.txt -r evals/requirements-audio.txt -r evals/requirements-instruments.txt
```

- `TELEMETRY-CONTRACT.md` — E7: the exact `mode` + provider-telemetry fields Tracks B/C/D must
  emit for this directory's tooling and the Q score to work.
- `judges/gemini_judge.py` — a `DeepEvalBaseLLM` wrapper over `google-genai` (no OpenAI key in
  this environment).
- `simulated_user/` — E5: `run_deepeval_conversation.py` (DeepEval `ConversationSimulator`) and
  `run_scenario_conversation.py` (LangWatch Scenario — the primary pick; see the security note at
  the top of the DeepEval script for why).
- `quality_score/` — E4: the Q aggregator. See `quality_score/README.md` for the full sub-score
  table and the naturalness-instrument trail (NISQA blocked on license, UTMOSv2 working in its
  isolated `utmosv2/.venv`, DNSMOS run for real but proven not to discriminate correctly).

Both `POST /v1/respond` (real chain, opt-in) and `evals/quality_score`'s trace ingestion require
the real backend chain running: `ORB_START_METRO=0 bash scripts/dev.sh` from the repo root, after
`. scripts/load-env.sh`.
