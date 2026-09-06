# Isolated UTMOSv2 naturalness instrument

UTMOSv2 runs only as dev/CI evaluation tooling through `scripts/score-utmosv2.py`. The app,
relay, and provider sidecars never import it, and it is not on a user latency path. This dedicated
venv exists because UTMOSv2's torch stack conflicts with the legally unusable NISQA stack in
`evals/.venv`.

## Install (macOS arm64, CPU-only)

```bash
/opt/homebrew/bin/python3.12 -m venv evals/utmosv2/.venv
evals/utmosv2/.venv/bin/python -m pip install \
  --requirement evals/utmosv2/requirements.lock.txt
evals/utmosv2/.venv/bin/python -m pip check
```

The recorded environment is Python 3.12.9, pip 25.0, UTMOSv2 commit
`cc2700db57bb83ee13dc31ebe1b868c254e15d09`, torch/torchaudio 2.2.1, transformers 4.42.4,
and numpy 1.26.4. `requirements.lock.txt` pins every resolved runtime package exactly.

## Score

```bash
evals/utmosv2/.venv/bin/python scripts/score-utmosv2.py \
  --audio evals/quality_score/samples/fish_tts_sample.wav \
  --audio evals/quality_score/samples/native_tts_sample.wav
```

The actual gate commands are:

```bash
# Expected exit 0: premium positive control clears 2.77 MOS.
evals/utmosv2/.venv/bin/python scripts/score-utmosv2.py \
  --audio evals/quality_score/samples/fish_tts_sample.wav --require-pass

# Expected exit 1: native negative control stays below 2.77 MOS.
evals/utmosv2/.venv/bin/python scripts/score-utmosv2.py \
  --audio evals/quality_score/samples/native_tts_sample.wav --require-pass
```

The scorer fixes Python/NumPy/Torch seed 42 and averages 10 UTMOSv2 repetitions because a
one-crop probe drifted by more than 0.2 MOS on these short clips.

## Model and cache

The first score fetches official weights into the gitignored
`evals/utmosv2/.venv/cache/` directory. Nothing under that cache is committed.

- UTMOSv2 fusion checkpoint: 818,531,314 bytes (781 MiB), 203,675,376 parameters.
- Supporting Wav2Vec2/EfficientNet caches: about 451 MiB.
- Total model cache: about 1.2 GiB; complete venv: about 2.1 GiB.
- GPU requirement: none. The acceptance run used Apple Silicon CPU with CUDA=false and MPS=false.
- Warm two-clip run: 96-125 seconds for 20 forward passes; use a cached CI job or scheduled eval,
  not the per-commit runtime/unit lane.

## Calibration

`evals/quality_score/naturalness_calibration.json` is the source of truth. Two real Fish S2-Pro
clips scored 3.162919 and 2.880779 MOS; native macOS `say -v Samantha` scored 2.652761 MOS. The
threshold is the midpoint between the lowest Fish score and native, rounded to 2.77. It leaves
+0.110779 MOS on the worst measured Fish clip and -0.117239 MOS on native. The earlier single-Fish
2.91 threshold was rejected after a fresh paid Fish clip scored 2.880779: installation evidence is
not calibration evidence, and a one-positive threshold did not generalize.

NISQA weights are not vendored or committed in this repository. The existing evaluator accepts an
external `nisqa_model` path; repository code does not download that checkpoint. NISQA remains
legally excluded because its pretrained weights are non-commercial.
