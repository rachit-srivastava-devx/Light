"""Score WAV files with isolated, CPU-only UTMOSv2 and emit machine-readable JSON."""

from __future__ import annotations

import argparse
import contextlib
import json
import math
import os
import random
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CACHE_ROOT = REPO_ROOT / "evals" / "utmosv2" / ".venv" / "cache"
CALIBRATION_PATH = (
    REPO_ROOT / "evals" / "quality_score" / "naturalness_calibration.json"
)
MODEL_CONFIG = "fusion_stage3"
MODEL_FOLD = 0
SOURCE_COMMIT = "cc2700db57bb83ee13dc31ebe1b868c254e15d09"
RANDOM_SEED = 42
NUM_REPETITIONS = 10


def _default_threshold() -> float:
    payload = json.loads(CALIBRATION_PATH.read_text(encoding="utf-8"))
    return float(payload["threshold_mos"])


def _configure_cache(cache_root: Path) -> None:
    cache_root = cache_root.resolve()
    os.environ.setdefault("UTMOSV2_CHACHE", str(cache_root / "utmosv2"))
    os.environ.setdefault("HF_HOME", str(cache_root / "huggingface"))
    # Transformers 4.42 reads this path directly. It stays inside the ignored venv.
    os.environ.setdefault("TRANSFORMERS_CACHE", str(cache_root / "huggingface"))
    os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--audio", action="append", required=True, type=Path)
    parser.add_argument("--threshold", type=float, default=None)
    parser.add_argument("--cache-root", type=Path, default=DEFAULT_CACHE_ROOT)
    parser.add_argument(
        "--require-pass",
        action="store_true",
        help="Exit 1 when any sample is below the calibrated threshold.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    threshold = _default_threshold() if args.threshold is None else args.threshold
    if not math.isfinite(threshold) or not 1.0 <= threshold <= 5.0:
        print("threshold must be finite and within the 1-5 MOS range", file=sys.stderr)
        return 2

    audio_paths = [path.resolve() for path in args.audio]
    missing = [str(path) for path in audio_paths if not path.is_file()]
    if missing:
        print(f"audio file does not exist: {missing[0]}", file=sys.stderr)
        return 2

    _configure_cache(args.cache_root)
    # Imports happen only inside this dedicated CLI/venv, never in Q or runtime code.
    import numpy as np
    import soundfile as sf
    import torch
    import utmosv2

    torch.use_deterministic_algorithms(True)

    load_started = time.perf_counter()
    with contextlib.redirect_stdout(sys.stderr):
        model = utmosv2.create_model(
            pretrained=True,
            config=MODEL_CONFIG,
            fold=MODEL_FOLD,
            device="cpu",
        )
    load_seconds = time.perf_counter() - load_started

    samples: list[dict[str, object]] = []
    for audio_path in audio_paths:
        # UTMOSv2 tiles short clips and chooses random crop offsets. A single unseeded
        # crop moved MOS by >0.2 in the first probe, so the gate fixes every RNG and
        # averages the package's supported test-time repetitions.
        random.seed(RANDOM_SEED)
        np.random.seed(RANDOM_SEED)
        torch.manual_seed(RANDOM_SEED)
        info = sf.info(str(audio_path))
        score_started = time.perf_counter()
        with contextlib.redirect_stdout(sys.stderr):
            mos = float(
                model.predict(
                    input_path=audio_path,
                    device="cpu",
                    num_workers=0,
                    batch_size=1,
                    num_repetitions=NUM_REPETITIONS,
                    remove_silent_section=True,
                    verbose=False,
                )
            )
        score_seconds = time.perf_counter() - score_started
        samples.append(
            {
                "audio_path": str(audio_path),
                "duration_seconds": round(float(info.duration), 6),
                "sample_rate_hz": int(info.samplerate),
                "mos": mos,
                "normalized_score": max(0.0, min(1.0, (mos - 1.0) / 4.0)),
                "threshold_mos": threshold,
                "margin_mos": mos - threshold,
                "passed": mos >= threshold,
                "score_seconds": round(score_seconds, 3),
            }
        )

    checkpoint = (
        Path(os.environ["UTMOSV2_CHACHE"])
        / "models"
        / MODEL_CONFIG
        / "fold0_s42_best_model.pth"
    )
    payload = {
        "instrument": "UTMOSv2",
        "instrument_version": utmosv2.__version__,
        "source_commit": SOURCE_COMMIT,
        "model_config": MODEL_CONFIG,
        "model_fold": MODEL_FOLD,
        "random_seed": RANDOM_SEED,
        "num_repetitions": NUM_REPETITIONS,
        "device": "cpu",
        "cuda_available": bool(torch.cuda.is_available()),
        "mps_available": bool(torch.backends.mps.is_available()),
        "parameter_count": sum(parameter.numel() for parameter in model.parameters()),
        "checkpoint_path": str(checkpoint.resolve()),
        "checkpoint_bytes": checkpoint.stat().st_size if checkpoint.is_file() else None,
        "model_load_seconds": round(load_seconds, 3),
        "samples": samples,
    }
    print(json.dumps(payload, indent=2))
    if args.require_pass and not all(bool(sample["passed"]) for sample in samples):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
