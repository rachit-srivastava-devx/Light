# Q — the quality score (E4)

One number a human and a verifier both read. `qscore.compute_q()` returns a **geometric mean**
over normalized `[0,1]` sub-scores, gated by hard **veto** conditions that force `Q = 0`
regardless of the other numbers. Geometric mean specifically so one broken dimension cannot be
averaged away by four good ones: an arithmetic mean of `[0.05, 1, 1, 1, 1]` is `0.81` (looks
fine); the geometric mean is `0.40` (does not).

Run it: `evals/.venv/bin/python evals/quality_score/run_qscore_demo.py` — invokes UTMOSv2 through
its isolated subprocess, prints Q for a real-evidence case, then applies a fake-adapter veto to a
non-zero real measured naturalness control so the forced transition to `Q=0` is observable.

## Veto gates (any one forces Q=0)

| Veto | Source of truth |
|---|---|
| Fake/`memory` LLM adapter used | `backend/gateway-sidecar/src/adapterEvidence.ts`'s `isFakeAdapterInvocation()` (E1) — the SAME predicate gateway-sidecar itself now returns as `is_fake_adapter` on every `/v1/complete` response |
| Native/on-device TTS fallback detected | `e2e-human-simulator/evaluator/tests/test_no_silent_fallback.py` (E3) — the `native_tts_fallback: true` / reintroduced-native-API signals |
| Dead air exceeded threshold | (see `no_dead_air` sub-score below — currently `[TBM]`, so this veto cannot fire from real data yet; it defaults to "not vetoed," never to "vetoed," on missing evidence, matching the same never-guess-toward-failure-either posture) |
| Silent fallback detected | Same detector as the native-TTS veto — a fallback that happened without the `voice.fish_unavailable` / `voice.fish_send_failed` log event firing |

## Sub-scores

| Sub-score | Instrument | Normalization | Status |
|---|---|---|---|
| **naturalness** | UTMOSv2 in `evals/utmosv2/.venv`, CPU-only subprocess | `(mos - 1) / 4` clipped to `[0,1]` (1-5 MOS scale); pass at MOS ≥2.77 | **Working** — two Fish clips 3.162919/2.880779 vs native 2.652761; worst gap +0.228017 MOS |
| **latency_at_user_clock** | `hdrhistogram` (verified real PyPI package `hdrhistogram==0.10.7`; the owner directive's literal "hdrh" package name does not exist — checked live against PyPI) over per-turn latency samples | `1.0` if p50≤1100ms and p99≤2000ms (the blueprint's own voice-to-voice SLO anchors); linear decay to `0` at 2x each budget | **Working** — real numbers from E2's own smoke run |
| **task_teach_success** | `e2e-human-simulator/evaluator/checks.py`'s existing `evaluate()` (reused, not reimplemented) over a captured trace | passed checks / total checks; a critical or error finding fails a check | **Working** — real numbers from `runs/live-20260807/trace/trace-final.jsonl` |
| **no_dead_air** | `silero-vad` (installed, verified importable; MIT, PyPI 6.2.1, 2026-02-24) | `1.0` if max VAD-confirmed silence gap ≤300ms (research brief F1) else linear decay to 0 at 2000ms | `[TBM]` for a full-session claim — no full captured session recording was available this run; the instrument itself is installed and demonstrated importable, not exercised on a real multi-turn session |
| **repair_success** | Same `evaluate()` call, filtered to `RESPONSE_DUPLICATE` / `FAILURE_NOT_SPOKEN` / `FAILURE_SPEECH_WRONG` findings (research brief F3: a repair must never be a verbatim repeat) | `1 - (blocking repair findings / repair-relevant findings)`; vacuously `1.0` if no repair-relevant event occurred in the trace (documented, not hidden) | **Working** — real numbers from the same real trace |

## Naturalness instrument decision

1. **NISQA** — already wired in `e2e-human-simulator/evaluator/audio_quality.py::nisqa_quality()`
   (ADR-0011). **Not used**: its pretrained weights (`nisqa.tar`/`nisqa_tts.tar`) are licensed
   **CC BY-NC-SA 4.0** — non-commercial (verified against github.com/gabrielmittag/NISQA's own
   license file; the *code* is MIT, the *weights* are not). This product is commercial. The `nisqa`
   PyPI package (code only) is still installed in `evals/.venv` — harmless — but its MOS output is
   never read by `qscore.py`.
2. **UTMOSv2** — MIT code and weights, designed for TTS-naturalness ranking. It now runs in the
   dedicated `evals/utmosv2/.venv` with every dependency exactly pinned. `qscore.py` launches
   `scripts/score-utmosv2.py`; neither Q nor any runtime process imports torch/transformers.
   The first single-crop probe was rejected because it drifted by >0.2 MOS between processes.
   The real gate fixes seed 42 and averages 10 supported test-time repetitions. Repeated scoring
   of the same native/calibration files is deterministic across independent processes:

   | Sample | Duration | UTMOSv2 MOS | Gate at 2.77 | Margin |
   |---|---:|---:|---:|---:|
   | Real Fish Audio S2-Pro, `orb.warm.v1` (calibration clip) | 5.105458s | 3.162919 | PASS | +0.392919 |
   | Real Fish Audio S2-Pro, `orb.warm.v1` (fresh paid Track O clip) | 4.757125s | 2.880779 | PASS | +0.110779 |
   | Native macOS `say -v Samantha` | 4.891042s | 2.652761 | FAIL | -0.117239 |

   The 2.77 threshold is the rounded midpoint, 2.766770, between the lowest of two real Fish
   samples and the native control. The earlier 2.91 single-positive threshold was rejected when
   the fresh Fish clip scored 2.880779: it discriminated correctly but would have failed the gate.
   The full calibration, sample hashes, and exact margins are in `naturalness_calibration.json`.
3. **DNSMOS** (`speechmos`, MIT, fully self-contained — bundles its ONNX weights directly in the
   wheel, no external download, `pip install speechmos` is the entire install) — **run for real**:

   ```
   evals/.venv/bin/python - <<'PY'
   # resample both samples to 16kHz (DNSMOS's required rate), then:
   import speechmos.dnsmos as dnsmos
   dnsmos.run(samples, 16000, model_type="dnsmos", return_df=False)
   PY
   ```

   Real output (see `evals/quality_score/samples/`; these task artifacts remain uncommitted as
   requested):

   | Sample | ovrl_mos | sig_mos | bak_mos | p808_mos |
   |---|---|---|---|---|
   | `fish_tts_sample.wav` (real Fish Audio, the premium voice) | 3.212 | 3.507 | 4.049 | 3.800 |
   | `native_tts_sample.wav` (macOS `say`, on-device/native TTS — the actual scarred failure mode) | 3.296 | 3.569 | 4.099 | 4.003 |

   The native/on-device sample scored **higher** than the real Fish sample on **every** DNSMOS
   sub-metric. This is not a broken run — DNSMOS is trained to detect noise/distortion
   (Deep Noise Suppression MOS), and a clean, dry, artifact-free robotic voice can legitimately
   out-score a more expressive, dynamically-varied neural voice on "absence of noise" while still
   sounding worse to a person. Using DNSMOS as the naturalness gate here would have been exactly
   the proxy-that-passes this whole track exists to reject, so it is not wired into `qscore.py`
   even though it is the one instrument that actually ran cleanly.

**Net**: UTMOSv2 closes naturalness and fails the original native-TTS scar. DNSMOS stays secondary
evidence only. `no_dead_air` still remains `[TBM]` until Q receives a full-session mixed recording;
therefore Q correctly reports `partial`, not `complete`, on the current captured artifacts.

## Files

- `qscore.py` — the aggregator (veto + geometric mean + `[TBM]` reporting).
- `run_qscore_demo.py` — runs it for real against a real trace + real latencies.
- `naturalness_calibration.json` — measured Fish/native threshold basis and sample hashes.
- `../utmosv2/` — isolated lockfile, setup/run documentation, and ignored local venv/cache.
- `fetch_fish_sample.py` — synthesizes a real Fish Audio sample via the live `voice-provider-sidecar`.
- `samples/` — two real Fish samples plus the native macOS control used for calibration.
