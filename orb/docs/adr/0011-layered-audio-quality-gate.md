# ADR 0011: Layered objective audio-quality gate

- Status: accepted
- Date: 2026-08-07
- Rules: C3, C9, C11, C17

## Context

The existing human-simulation evaluator checks captured JSONL events, but it
cannot prove that the recorded microphone audio was understood or that the
audio played to the user was intelligible and natural. A response event can
exist while the waveform is silent, garbled, or clipped.

## Decision

Add an explicit, offline-capable gate under
`e2e-human-simulator/evaluator/audio_quality.py` with four independent layers:

1. `faster-whisper` produces the transcript from the captured microphone WAV;
   `jiwer` computes WER, CER, MER, and WIL against the expected words.
2. `pystoi` compares generated TTS audio with a clean reference recording of
   the same words and measures intelligibility.
3. `NISQA`/`NISQA-TTS` scores generated audio for MOS and quality dimensions.
4. `faster-whisper` transcribes the assistant playback and `jiwer` scores it
   against the expected assistant words. This is the direct intelligibility
   gate for the device output, independent of waveform similarity.

Every layer runs and reports its own result. Missing files, dependencies,
models, decode errors, and inference failures are `blocked` and fail the gate;
the evaluator never silently turns an unmeasured layer into a pass.

The pYSTOI reference must be a human-recorded, same-content clean WAV. A
second provider synthesis is not a valid waveform reference because pYSTOI
compares waveform intelligibility, not transcript similarity. A local device
run may explicitly mark pYSTOI `not_run` until that reference exists; release
verification keeps the strict reference requirement enabled.

The packages are an optional `audio-eval` dependency group rather than base
runtime dependencies. The gate is invoked explicitly with local model paths by
the E2E harness. This protects T0 startup cost and avoids downloading model
weights during normal app execution.

Default thresholds are input WER ≤ 0.35, input CER ≤ 0.20, playback WER ≤
0.25, playback CER ≤ 0.15, pYSTOI ≥ 0.75, and NISQA MOS ≥ 3.00. They are
CLI-configurable and are stored in the JSON evidence output.

## Consequences

- A real voice gate needs two audio references: expected text for STT and a
  clean, same-content TTS reference for pYSTOI. Comparing arbitrary user speech
  with unrelated assistant speech is invalid.
- NISQA checkpoint weights must be provisioned locally and selected explicitly;
  NISQA-TTS weights are the correct choice for synthesized speech naturalness.
- The light evaluator test suite remains dependency-free; the audio gate's
  integration run is a separate, model-backed verification step.
