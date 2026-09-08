from pathlib import Path
from types import SimpleNamespace

import pytest

from crew.adapters.claude import ClaudeAdapter
from crew.adapters.errors import AdapterError
from crew.adapters.metadata import read_metadata


def test_rejects_negative_log_size_floor(monkeypatch, tmp_path: Path):
    """§9 gap-closing test: a negative `log_size_floor` must not let a
    near-empty invocation trivially pass the "successful, has evidence"
    invariant -- it must fall back to the package default floor instead."""

    def fake_run(command, *, stdout, **kwargs):
        stdout.write(b"tiny")
        return SimpleNamespace(returncode=0)

    monkeypatch.setattr("crew.adapters.runner.subprocess.run", fake_run)
    diff = tmp_path / "diff.patch"
    diff.write_text("x", encoding="utf-8")
    with pytest.raises(AdapterError, match="log below floor"):
        ClaudeAdapter(log_size_floor=-1).invoke(
            "make the change", stdout_path=tmp_path / "out", stderr_path=tmp_path / "err",
            diff_path=diff,
        )


def test_metadata_survives_non_utf8_transcript(tmp_path: Path):
    """§9 gap-closing test: a transcript containing invalid UTF-8 bytes must
    make `read_metadata` return `(None, None)`, not raise `UnicodeDecodeError`."""

    path = tmp_path / "transcript.log"
    path.write_bytes(b"\xff\xfe not valid utf-8 \x80\x81")

    model, usage = read_metadata(path)

    assert model is None
    assert usage is None


def test_metadata_missing_file_yields_absence_not_error(tmp_path: Path) -> None:
    model, usage = read_metadata(tmp_path / "does-not-exist.log")
    assert model is None
    assert usage is None


def test_metadata_reads_usage_from_last_matching_document(tmp_path: Path) -> None:
    path = tmp_path / "transcript.log"
    path.write_text('{"usage": {"input_tokens": 3, "output_tokens": 5}}\n', encoding="utf-8")

    _, usage = read_metadata(path)

    assert usage is not None
    assert usage.input_tokens == 3
    assert usage.output_tokens == 5
