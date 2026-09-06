# Independent human-simulation evaluator

This folder is intentionally independent of the app. It consumes exported JSONL evidence and never imports app modules, launches a simulator, sends microphone frames, calls a provider, or changes app files.

## Trace contract

Each non-empty JSONL line is one event:

```json
{"ts":"2026-08-07T10:00:00.000Z","seq":1,"source":"mobile","event":"orb.state","run_id":"run-1","session_id":"s-1","data":{"state":"booting"}}
```

Required fields are `ts`, `seq`, `source`, `event`, and object-valued `data`. The complete machine-readable contract is [trace.schema.json](trace.schema.json).

Useful events include `orb.state`, `turn.end`, `stt.final`, `gateway.response`, `assistant.response`, `tts.first_audio`, `failure`, and `run.expectations`.

`run.expectations.data` can contain `expected_transcripts`, for example:

```json
{"expected_transcripts":[{"text":"hello how are you","max_word_error_rate":0.25}]}
```

Assistant response evidence should use:

```json
{"speech":{"spoken_text":"[chuckle] Hello! [emphasis]How are you?","plain_text":"Hello! How are you?"}}
```

## Run

From `e2e-human-simulator/`:

```sh
python3 -m unittest discover -s evaluator/tests -v
python3 -m evaluator evaluator/fixtures/good.jsonl --out /tmp/orb-good.md
python3 -m evaluator evaluator/fixtures/problematic.jsonl --out /tmp/orb-problems.md --judges
```

From the repository root, the same evaluator can be run with absolute local
judge binaries. This is a passing command for the clean fixture:

```sh
PYTHONPATH="$PWD/company/products/adhd-focus-orb/e2e-human-simulator" \
E2E_CODEX_BIN="/Applications/ChatGPT.app/Contents/Resources/codex" \
E2E_CLAUDE_BIN="/Users/rachitsrivastava/Library/Application Support/Claude/claude-code/2.1.221/claude.app/Contents/MacOS/claude" \
python3 -m evaluator \
  company/products/adhd-focus-orb/e2e-human-simulator/evaluator/fixtures/good.jsonl \
  --out /tmp/adhd-focus-orb-good.md --judges
```

`E2E_CODEX_BIN` and `E2E_CLAUDE_BIN` may be either PATH names or absolute
executable paths. The adapters start each judge with the evaluator directory
as its working directory, pass only a redacted JSON evidence bundle on stdin,
and never pass simulator handles, microphone data, provider credentials, or
app commands. Codex is started in read-only/ephemeral mode. A judge failure or
missing binary is recorded in `Optional judge review`; it does not alter the
deterministic verdict.

The process exits `1` when deterministic critical/error findings exist. Missing `codex` or `claude` CLIs are written as `unavailable`; they do not crash the report or change the deterministic verdict.

## What is checked

- final transcript presence and word error rate against fixture expectations;
- stage and end-to-first-response latency budgets;
- known orb states and valid transitions;
- missing, empty, raw-JSON, overly long, and repeated responses;
- failure events followed by a specific spoken recovery;
- Fish Audio tags, supported tag names, bounded use, and clean `plain_text`;
- optional evidence-only review by local Codex and Claude adapters.

The report’s **What felt wrong** section is deterministic first. Judge output is additive and never drives the app.

## Layered audio-quality gate

Trace checks cannot prove waveform quality. Run the separate audio gate after a
real journey has captured the microphone WAV and the app's playback WAV:

```sh
cd backend/relay-py
python -m pip install -e '.[audio-eval]'
cd ../../e2e-human-simulator
PYTHONPATH=. python -m evaluator.audio_quality \
  --stt-audio runs/<run-id>/input/hello.wav \
  --expected-transcript 'hello how are you' \
  --tts-audio runs/<run-id>/audio/assistant.wav \
  --tts-expected-transcript 'Hey, Rachit. Good to hear from you.' \
  --tts-reference-audio fixtures/tts/hello-how-are-you-clean.wav \
  --whisper-model /models/faster-whisper-small.en \
  --nisqa-model /models/nisqa_tts.tar \
  --json-out runs/<run-id>/audio-quality.json
```

`faster-whisper` is local-files-only by default. Use
`--allow-model-download` only when deliberately provisioning a model cache.
The gate exits `0` only when all four layers pass; missing dependencies,
missing model weights, missing references, and inference errors are explicit
`blocked` failures. Use NISQA-TTS weights for generated speech naturalness.

The pYSTOI reference must be a human-recorded, same-content clean WAV. A
second Fish/provider synthesis is not a valid reference because pYSTOI compares
waveform intelligibility, not transcript similarity. For local device checks
where that recording is not yet available, pass
`--allow-missing-stoi-reference`; pYSTOI is then reported as `not_run` while
faster-whisper/JiWER and NISQA remain enforced. Release gates should omit that
flag and fail closed until the human reference is present.
