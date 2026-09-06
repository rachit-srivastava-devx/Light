"""E4 support script — synthesize one real Fish Audio sample via the real voice-provider-sidecar,
for the UTMOSv2/DNSMOS naturalness discrimination proof (see evals/quality_score/README.md).

Calls the SAME HTTP contract `backend/relay-rs` speaks
(`backend/voice-provider-sidecar/src/contract.ts`): `POST /v1/tts/synthesize` ->
`{audio_chunks: number[][]}`, headerless PCM (24kHz/mono/16-bit — must match
`backend/voice-provider-sidecar/src/tts/fish.ts`'s `RELAY_AUDIO_SAMPLE_RATE_HZ`). This script only
reassembles those chunks into a real .wav file so an offline MOS model can read it; it does not
reimplement or bypass the sidecar's own provider call.

Requires the real backend chain running (`scripts/dev.sh`) and `ORB_TTS_PROVIDER=fish` with a
funded `FISH_API_KEY` (both already true in this repo's `.env`). Never prints key values.

Usage:
    evals/.venv/bin/python evals/quality_score/fetch_fish_sample.py \\
        --text "..." --voice-id orb.warm.v1 --emotion warm \\
        --out evals/quality_score/samples/fish_tts_sample.wav
"""

from __future__ import annotations

import argparse
import sys

import numpy as np
import requests
import soundfile as sf

SAMPLE_RATE_HZ = 24_000  # must match backend/voice-provider-sidecar/src/tts/fish.ts


def synthesize(base_url: str, text: str, voice_id: str, emotion: str, timeout_s: float) -> bytes:
    response = requests.post(
        f"{base_url.rstrip('/')}/v1/tts/synthesize",
        json={"text": text, "voice_id": voice_id, "emotion": emotion},
        timeout=timeout_s,
    )
    response.raise_for_status()
    payload = response.json()
    chunks: list[list[int]] = payload["audio_chunks"]
    return bytes(byte for chunk in chunks for byte in chunk)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default="http://127.0.0.1:8083")
    parser.add_argument("--text", required=True)
    parser.add_argument("--voice-id", default="orb.warm.v1")
    parser.add_argument("--emotion", default="warm")
    parser.add_argument("--out", required=True)
    parser.add_argument("--timeout-s", type=float, default=30.0)
    args = parser.parse_args(argv)

    pcm_bytes = synthesize(args.base_url, args.text, args.voice_id, args.emotion, args.timeout_s)
    if len(pcm_bytes) == 0:
        print("ERROR: synthesize returned zero audio bytes", file=sys.stderr)
        return 1
    samples = np.frombuffer(pcm_bytes, dtype="<i2")  # PCM16 little-endian, mono
    sf.write(args.out, samples, SAMPLE_RATE_HZ, subtype="PCM_16")
    duration_s = len(samples) / SAMPLE_RATE_HZ
    print(f"wrote {args.out}: {len(pcm_bytes)} bytes, {duration_s:.2f}s at {SAMPLE_RATE_HZ}Hz")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
