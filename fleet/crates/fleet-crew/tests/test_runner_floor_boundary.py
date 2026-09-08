from pathlib import Path
from types import SimpleNamespace

from crew.adapters.claude import ClaudeAdapter
from crew.adapters.runner import DEFAULT_LOG_SIZE_FLOOR


def test_log_exactly_at_floor_is_accepted(monkeypatch, tmp_path: Path):
    """Mutation-testing target: flipping `size < log_size_floor` to
    `size <= log_size_floor` in runner.py must be caught by a boundary
    fixture, since a below-floor fixture can't tell the two operators apart.
    A log of exactly `DEFAULT_LOG_SIZE_FLOOR` bytes is a pass under the real
    strict-less-than check."""

    def fake_run(command, *, stdout, **kwargs):
        stdout.write(b"x" * DEFAULT_LOG_SIZE_FLOOR)
        return SimpleNamespace(returncode=0)

    monkeypatch.setattr("crew.adapters.runner.subprocess.run", fake_run)
    diff = tmp_path / "diff.patch"
    diff.write_text("x", encoding="utf-8")

    result = ClaudeAdapter().invoke(
        "make the change", stdout_path=tmp_path / "out", stderr_path=tmp_path / "err",
        diff_path=diff,
    )

    assert result.ok
