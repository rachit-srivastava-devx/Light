from pathlib import Path
from types import SimpleNamespace

import pytest

from crew.adapters.claude import ClaudeAdapter
from crew.adapters.errors import AdapterError


def _fake_run(command, *, stdin, stdout, stderr, cwd, env, check):
    assert stdin is __import__("subprocess").DEVNULL
    stdout.write(b"model-output with enough evidence to clear the floor\n")
    return SimpleNamespace(returncode=0)


def test_successful_invocation_closes_stdin_and_returns_file_evidence(monkeypatch, tmp_path: Path):
    monkeypatch.setattr("crew.adapters.runner.subprocess.run", _fake_run)
    output = tmp_path / "stdout.log"
    errors = tmp_path / "stderr.log"
    diff = tmp_path / "diff.patch"
    diff.write_text("diff --git a/main.py b/main.py\n", encoding="utf-8")

    result = ClaudeAdapter(model="claude-test").invoke(
        "make the change", stdout_path=output, stderr_path=errors, diff_path=diff
    )

    assert result.returncode == 0
    # A REQUESTED model is not a RESOLVED model. The fake emits no model
    # metadata, so the adapter must claim nothing -- never the constructor's
    # own `model=` argument echoed back as if it were evidence.
    assert result.resolved_model is None
    assert output.read_text(encoding="utf-8")


def test_resolved_model_is_read_back_from_output_not_from_the_request(monkeypatch, tmp_path: Path):
    """When the CLI DOES report a model, the adapter reads back the reported
    value -- never the requested one."""

    def fake_run(command, *, stdin, stdout, stderr, cwd, env, check):
        stdout.write(b'{"resolved_model": "claude-actually-served"}\n')
        stdout.write(b"padding line to clear the log-size floor for this invocation\n")
        return SimpleNamespace(returncode=0)

    monkeypatch.setattr("crew.adapters.runner.subprocess.run", fake_run)
    diff = tmp_path / "d.patch"
    diff.write_text("diff --git a/x b/x\n", encoding="utf-8")
    result = ClaudeAdapter(model="claude-requested").invoke(
        "go", stdout_path=tmp_path / "o.log", stderr_path=tmp_path / "e.log", diff_path=diff
    )
    assert result.resolved_model == "claude-actually-served"
    assert result.resolved_model != "claude-requested"


def test_log_floor_refuses_a_false_success(monkeypatch, tmp_path: Path):
    def fake_run(command, *, stdout, **kwargs):
        stdout.write(b"x" * 39)
        return SimpleNamespace(returncode=0)

    monkeypatch.setattr("crew.adapters.runner.subprocess.run", fake_run)
    diff = tmp_path / "diff.patch"
    diff.write_text("x", encoding="utf-8")
    with pytest.raises(AdapterError, match="log below floor"):
        ClaudeAdapter().invoke(
            "make the change", stdout_path=tmp_path / "out", stderr_path=tmp_path / "err",
            diff_path=diff,
        )


def test_empty_diff_refuses_a_false_success(monkeypatch, tmp_path: Path):
    monkeypatch.setattr("crew.adapters.runner.subprocess.run", _fake_run)
    with pytest.raises(AdapterError, match="empty diff"):
        ClaudeAdapter().invoke(
            "make the change", stdout_path=tmp_path / "out", stderr_path=tmp_path / "err",
            diff_path=tmp_path / "empty.patch",
        )
