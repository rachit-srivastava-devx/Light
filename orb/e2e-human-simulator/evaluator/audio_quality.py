"""Layered objective audio gate for the Focus Orb voice journey.

The gate is deliberately outside the application. It evaluates captured files:

* faster-whisper + jiwer: did the microphone recording produce the words the
  human said?
* pYSTOI: is the generated speech intelligible against a clean, same-content
  reference recording?
* faster-whisper + jiwer on playback: can an independent recognizer recover
  what the assistant was supposed to say? This catches fluent-sounding
  waveforms that still produce unintelligible words on the device.
* NISQA-TTS: is the generated speech natural and free of obvious quality
  degradation?

The four packages are imported only when their layer runs. This keeps the
  black-box evaluator usable in the lightweight test environment while making
  a real gate fail closed when a required dependency or model is absent.
"""

from __future__ import annotations

import argparse
import json
import re
from collections.abc import Callable
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any


class AudioQualityError(RuntimeError):
    """A concrete, user-actionable audio gate failure."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message


@dataclass(frozen=True)
class AudioGateConfig:
    """Thresholds and inference settings for one reproducible gate run."""

    max_wer: float = 0.35
    max_cer: float = 0.20
    max_tts_wer: float = 0.25
    max_tts_cer: float = 0.15
    min_stoi: float = 0.75
    min_nisqa_mos: float = 3.00
    require_stoi_reference: bool = True
    language: str = "en"
    whisper_model: str = "small.en"
    whisper_device: str = "cpu"
    whisper_compute_type: str = "int8"
    whisper_beam_size: int = 5
    local_files_only: bool = True
    nisqa_device: str = "cpu"


@dataclass
class AudioLayerResult:
    name: str
    status: str
    metrics: dict[str, Any] = field(default_factory=dict)
    message: str = ""
    error_code: str | None = None
    required: bool = True

    @property
    def passed(self) -> bool:
        return self.status == "pass" or (self.status == "not_run" and not self.required)


@dataclass
class AudioGateReport:
    config: AudioGateConfig
    layers: list[AudioLayerResult]

    @property
    def passed(self) -> bool:
        return bool(self.layers) and all(layer.passed for layer in self.layers)

    def to_dict(self) -> dict[str, Any]:
        return {
            "gate": "audio-quality",
            "passed": self.passed,
            "config": asdict(self.config),
            "layers": [asdict(layer) for layer in self.layers],
        }


@dataclass(frozen=True)
class TranscriptionResult:
    text: str
    language: str | None = None
    language_probability: float | None = None
    segments: int = 0


def _normalise_transcript(value: str) -> str:
    """Apply only deterministic text cleanup before JiWER scoring."""
    value = value.lower()
    value = re.sub(r"[^\w\s']", " ", value, flags=re.UNICODE)
    return re.sub(r"\s+", " ", value).strip()


def compute_jiwer_metrics(reference: str, hypothesis: str) -> dict[str, float]:
    """Compute normalized WER/CER/MER/WIL using JiWER.

    Importing JiWER here is intentional: the evaluator can still run its
    trace-only checks without installing the audio-eval extra.
    """
    try:
        import jiwer
    except ImportError as exc:  # pragma: no cover - exercised by integration env
        raise AudioQualityError(
            "AUDIO_DEPENDENCY_MISSING",
            "JiWER is required for the STT layer; install the audio-eval extra.",
        ) from exc

    reference = _normalise_transcript(reference)
    hypothesis = _normalise_transcript(hypothesis)
    return {
        "wer": float(jiwer.wer(reference, hypothesis)),
        "cer": float(jiwer.cer(reference, hypothesis)),
        "mer": float(jiwer.mer(reference, hypothesis)),
        "wil": float(jiwer.wil(reference, hypothesis)),
    }


def transcribe_with_faster_whisper(audio_path: Path, config: AudioGateConfig) -> TranscriptionResult:
    """Transcribe one captured WAV with a local faster-whisper model."""
    try:
        from faster_whisper import WhisperModel
    except ImportError as exc:  # pragma: no cover - exercised by integration env
        raise AudioQualityError(
            "AUDIO_DEPENDENCY_MISSING",
            "faster-whisper is required for the STT layer; install the audio-eval extra.",
        ) from exc

    if not audio_path.is_file():
        raise AudioQualityError("AUDIO_INPUT_MISSING", f"STT audio file does not exist: {audio_path}")

    try:
        model = WhisperModel(
            config.whisper_model,
            device=config.whisper_device,
            compute_type=config.whisper_compute_type,
            local_files_only=config.local_files_only,
        )
        segments, info = model.transcribe(
            str(audio_path),
            language=config.language,
            beam_size=config.whisper_beam_size,
            vad_filter=True,
            condition_on_previous_text=False,
        )
        materialized = list(segments)
    except Exception as exc:  # provider/model errors must become gate evidence
        raise AudioQualityError("STT_INFERENCE_FAILED", f"faster-whisper failed: {exc}") from exc

    return TranscriptionResult(
        text=" ".join(segment.text.strip() for segment in materialized).strip(),
        language=getattr(info, "language", None),
        language_probability=_as_float(getattr(info, "language_probability", None)),
        segments=len(materialized),
    )


def _load_audio(path: Path) -> tuple[Any, int]:
    try:
        import numpy as np
        import soundfile as sf
    except ImportError as exc:  # pragma: no cover - exercised by integration env
        raise AudioQualityError(
            "AUDIO_DEPENDENCY_MISSING",
            "numpy and soundfile are required for the pYSTOI layer; install the audio-eval extra.",
        ) from exc

    if not path.is_file():
        raise AudioQualityError("AUDIO_INPUT_MISSING", f"Audio file does not exist: {path}")
    try:
        samples, sample_rate = sf.read(str(path), always_2d=False)
    except Exception as exc:
        raise AudioQualityError("AUDIO_DECODE_FAILED", f"Could not decode WAV {path}: {exc}") from exc
    samples = np.asarray(samples, dtype=np.float64)
    if samples.ndim == 2:
        samples = samples.mean(axis=1)
    if samples.ndim != 1 or samples.size == 0:
        raise AudioQualityError("AUDIO_INVALID_SHAPE", f"Audio must contain non-empty mono or stereo samples: {path}")
    if not np.isfinite(samples).all():
        raise AudioQualityError("AUDIO_NONFINITE", f"Audio contains NaN or infinity: {path}")
    return samples, int(sample_rate)


def stoi_against_reference(reference_path: Path, degraded_path: Path) -> dict[str, float]:
    """Score generated audio against same-content clean speech with pYSTOI."""
    try:
        import numpy as np
        from pystoi import stoi
    except ImportError as exc:  # pragma: no cover - exercised by integration env
        raise AudioQualityError(
            "AUDIO_DEPENDENCY_MISSING",
            "pystoi is required for the TTS intelligibility layer; install the audio-eval extra.",
        ) from exc

    clean, clean_rate = _load_audio(reference_path)
    degraded, degraded_rate = _load_audio(degraded_path)
    if clean_rate != degraded_rate:
        raise AudioQualityError(
            "AUDIO_SAMPLE_RATE_MISMATCH",
            f"pYSTOI requires matching sample rates: reference={clean_rate}, candidate={degraded_rate}.",
        )

    # pYSTOI requires equal-length 1-D signals. Padding preserves the full
    # utterance and makes duration mismatch visible in the recorded metric.
    length = max(clean.size, degraded.size)
    clean = np.pad(clean, (0, length - clean.size))
    degraded = np.pad(degraded, (0, length - degraded.size))
    try:
        score = float(stoi(clean, degraded, clean_rate, extended=False))
    except Exception as exc:
        raise AudioQualityError("STOI_INFERENCE_FAILED", f"pYSTOI failed: {exc}") from exc
    return {
        "stoi": score,
        "reference_duration_ms": float(clean.size / clean_rate * 1000),
        "candidate_duration_ms": float(degraded.size / degraded_rate * 1000),
    }


def nisqa_quality(audio_path: Path, model_path: Path, device: str = "cpu") -> dict[str, float]:
    """Run the official NISQA model against one generated speech file.

    NISQA's published Python API loads model architecture and preprocessing
    settings from the checkpoint. Passing the same minimal argument envelope as
    the upstream predict_file command keeps this adapter compatible with both
    NISQA and NISQA-TTS checkpoints.
    """
    try:
        from nisqa.NISQA_model import nisqaModel
    except ImportError as exc:  # pragma: no cover - exercised by integration env
        raise AudioQualityError(
            "AUDIO_DEPENDENCY_MISSING",
            "NISQA is required for the TTS naturalness layer; install the audio-eval extra.",
        ) from exc

    if not model_path.is_file():
        raise AudioQualityError("NISQA_MODEL_MISSING", f"NISQA checkpoint does not exist: {model_path}")
    if not audio_path.is_file():
        raise AudioQualityError("AUDIO_INPUT_MISSING", f"TTS audio file does not exist: {audio_path}")

    args: dict[str, Any] = {
        "mode": "predict_file",
        "pretrained_model": str(model_path),
        "deg": str(audio_path),
        "data_dir": None,
        "output_dir": None,
        "tr_bs_val": 1,
        "tr_num_workers": 0,
        "tr_device": device,
        "ms_channel": None,
    }
    try:
        predictor = nisqaModel(args)
        dataframe = predictor.predict()
        row = dataframe.iloc[0].to_dict()
    except Exception as exc:
        raise AudioQualityError("NISQA_INFERENCE_FAILED", f"NISQA failed: {exc}") from exc

    metrics: dict[str, float] = {}
    for key in ("mos_pred", "noi_pred", "dis_pred", "col_pred", "loud_pred", "naturalness"):
        value = _as_float(row.get(key))
        if value is not None:
            metrics[key] = value
    mos = metrics.get("mos_pred", metrics.get("naturalness"))
    if mos is None:
        raise AudioQualityError(
            "NISQA_OUTPUT_INVALID",
            f"NISQA output did not contain mos_pred or naturalness; columns={sorted(row)}",
        )
    metrics["mos"] = mos
    return metrics


def _as_float(value: Any) -> float | None:
    if isinstance(value, bool):
        return None
    try:
        return None if value is None else float(value)
    except (TypeError, ValueError):
        return None


def _blocked(name: str, error: AudioQualityError) -> AudioLayerResult:
    return AudioLayerResult(name=name, status="blocked", message=error.message, error_code=error.code)


def _not_run(name: str, error: AudioQualityError) -> AudioLayerResult:
    """Record an optional layer that could not be measured without fake evidence."""
    return AudioLayerResult(
        name=name,
        status="not_run",
        message=error.message,
        error_code=error.code,
        required=False,
    )


def run_audio_quality_gate(
    *,
    stt_audio: Path | None,
    expected_transcript: str | None,
    tts_audio: Path | None,
    tts_reference_audio: Path | None,
    nisqa_model: Path | None,
    tts_expected_transcript: str | None = None,
    config: AudioGateConfig | None = None,
    transcriber: Callable[[Path, AudioGateConfig], TranscriptionResult] | None = None,
    jiwer_runner: Callable[[str, str], dict[str, float]] | None = None,
    stoi_runner: Callable[[Path, Path], dict[str, float]] | None = None,
    nisqa_runner: Callable[[Path, Path, str], dict[str, float]] | None = None,
) -> AudioGateReport:
    """Run all layers and return every failure, rather than stopping at layer one."""
    config = config or AudioGateConfig()
    transcriber = transcriber or transcribe_with_faster_whisper
    jiwer_runner = jiwer_runner or compute_jiwer_metrics
    stoi_runner = stoi_runner or stoi_against_reference
    nisqa_runner = nisqa_runner or nisqa_quality
    layers: list[AudioLayerResult] = []

    if stt_audio is None or not expected_transcript:
        layers.append(_blocked("stt", AudioQualityError("STT_INPUT_MISSING", "STT audio and expected transcript are required.")))
    else:
        try:
            transcription = transcriber(stt_audio, config)
            metrics = jiwer_runner(expected_transcript, transcription.text)
            metrics.update({
                "reference": expected_transcript,
                "hypothesis": transcription.text,
                "language": transcription.language,
                "language_probability": transcription.language_probability,
                "segments": transcription.segments,
            })
            passed = metrics["wer"] <= config.max_wer and metrics["cer"] <= config.max_cer
            layers.append(AudioLayerResult(
                name="stt",
                status="pass" if passed else "fail",
                metrics=metrics,
                message=("Transcript is within WER/CER thresholds." if passed else "Transcript accuracy exceeded WER or CER threshold."),
                error_code=None if passed else "STT_ACCURACY_BELOW_THRESHOLD",
            ))
        except AudioQualityError as exc:
            layers.append(_blocked("stt", exc))

    if tts_audio is None or tts_reference_audio is None:
        error = AudioQualityError(
            "TTS_REFERENCE_MISSING",
            "pYSTOI was not run: provide a human-recorded, same-content clean reference WAV. "
            "A second provider synthesis is not a valid reference.",
        )
        layers.append(_blocked("tts_intelligibility", error) if config.require_stoi_reference else _not_run("tts_intelligibility", error))
    else:
        try:
            metrics = stoi_runner(tts_reference_audio, tts_audio)
            passed = metrics["stoi"] >= config.min_stoi
            layers.append(AudioLayerResult(
                name="tts_intelligibility",
                status="pass" if passed else "fail",
                metrics=metrics,
                message=("pYSTOI is within the intelligibility threshold." if passed else "pYSTOI is below the intelligibility threshold."),
                error_code=None if passed else "TTS_INTELLIGIBILITY_BELOW_THRESHOLD",
            ))
        except AudioQualityError as exc:
            layers.append(_blocked("tts_intelligibility", exc))

    # pYSTOI compares waveforms, but it cannot tell us whether the words are
    # recoverable. Run the same independent ASR against playback when the
    # journey supplies the expected assistant text. This is the hard gate for
    # the user's actual complaint: "I cannot understand a single word".
    if tts_expected_transcript:
        if tts_audio is None:
            layers.append(_blocked("tts_transcript", AudioQualityError("TTS_INPUT_MISSING", "TTS audio and expected assistant transcript are required.")))
        else:
            try:
                transcription = transcriber(tts_audio, config)
                metrics = jiwer_runner(tts_expected_transcript, transcription.text)
                metrics.update({
                    "reference": tts_expected_transcript,
                    "hypothesis": transcription.text,
                    "language": transcription.language,
                    "language_probability": transcription.language_probability,
                    "segments": transcription.segments,
                })
                passed = metrics["wer"] <= config.max_tts_wer and metrics["cer"] <= config.max_tts_cer
                layers.append(AudioLayerResult(
                    name="tts_transcript",
                    status="pass" if passed else "fail",
                    metrics=metrics,
                    message=("Playback transcript is within WER/CER thresholds." if passed else "Playback transcript accuracy exceeded WER or CER threshold."),
                    error_code=None if passed else "TTS_TRANSCRIPT_ACCURACY_BELOW_THRESHOLD",
                ))
            except AudioQualityError as exc:
                layers.append(_blocked("tts_transcript", exc))

    if tts_audio is None or nisqa_model is None:
        layers.append(_blocked("tts_naturalness", AudioQualityError("NISQA_INPUT_MISSING", "TTS audio and a local NISQA checkpoint are required.")))
    else:
        try:
            metrics = nisqa_runner(tts_audio, nisqa_model, config.nisqa_device)
            passed = metrics["mos"] >= config.min_nisqa_mos
            layers.append(AudioLayerResult(
                name="tts_naturalness",
                status="pass" if passed else "fail",
                metrics=metrics,
                message=("NISQA MOS is within the naturalness threshold." if passed else "NISQA MOS is below the naturalness threshold."),
                error_code=None if passed else "TTS_NATURALNESS_BELOW_THRESHOLD",
            ))
        except AudioQualityError as exc:
            layers.append(_blocked("tts_naturalness", exc))

    return AudioGateReport(config=config, layers=layers)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the Focus Orb layered audio-quality gate.")
    parser.add_argument("--stt-audio", type=Path, required=True, help="Captured microphone WAV")
    parser.add_argument("--expected-transcript", required=True, help="Words spoken in the STT WAV")
    parser.add_argument("--tts-audio", type=Path, required=True, help="Captured app playback/TTS WAV")
    parser.add_argument("--tts-expected-transcript", required=True, help="Words the assistant was expected to speak")
    parser.add_argument("--tts-reference-audio", type=Path, help="Human-recorded clean WAV of the same TTS words")
    parser.add_argument(
        "--allow-missing-stoi-reference",
        action="store_true",
        help="Mark pYSTOI not_run when no human same-content reference is available (not for release gates)",
    )
    parser.add_argument("--whisper-model", default="small.en", help="Local faster-whisper model directory or cached model name")
    parser.add_argument("--allow-model-download", action="store_true", help="Allow faster-whisper to download a model")
    parser.add_argument("--nisqa-model", type=Path, required=True, help="Local NISQA or NISQA-TTS .tar checkpoint")
    parser.add_argument("--max-wer", type=float, default=0.35)
    parser.add_argument("--max-cer", type=float, default=0.20)
    parser.add_argument("--max-tts-wer", type=float, default=0.25)
    parser.add_argument("--max-tts-cer", type=float, default=0.15)
    parser.add_argument("--min-stoi", type=float, default=0.75)
    parser.add_argument("--min-nisqa-mos", type=float, default=3.00)
    parser.add_argument("--json-out", type=Path, help="Write machine-readable gate evidence")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    report = run_audio_quality_gate(
        stt_audio=args.stt_audio,
        expected_transcript=args.expected_transcript,
        tts_audio=args.tts_audio,
        tts_expected_transcript=args.tts_expected_transcript,
        tts_reference_audio=args.tts_reference_audio,
        nisqa_model=args.nisqa_model,
        config=AudioGateConfig(
            max_wer=args.max_wer,
            max_cer=args.max_cer,
            max_tts_wer=args.max_tts_wer,
            max_tts_cer=args.max_tts_cer,
            min_stoi=args.min_stoi,
            min_nisqa_mos=args.min_nisqa_mos,
            require_stoi_reference=not args.allow_missing_stoi_reference,
            whisper_model=args.whisper_model,
            local_files_only=not args.allow_model_download,
        ),
    )
    payload = json.dumps(report.to_dict(), indent=2, ensure_ascii=False)
    if args.json_out:
        args.json_out.write_text(payload + "\n", encoding="utf-8")
    print(payload)
    return 0 if report.passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
