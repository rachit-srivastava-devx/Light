from pathlib import Path
from types import SimpleNamespace

import pytest

from crew.adapters.base import AdapterError
from crew.adapters.capability import probe_adapter
from crew.adapters.claude import ClaudeAdapter


def _fake_run(command, *, stdin, stdout, stderr, cwd, env, check):
    assert stdin is __import__("subprocess").DEVNULL
    stdout.write(b"model-output with enough evidence to clear the floor\n")
    return SimpleNamespace(returncode=0)


def test_successful_invocation_closes_stdin_and_returns_file_evidence(monkeypatch, tmp_path: Path):
    monkeypatch.setattr("crew.adapters._subprocess.subprocess.run", _fake_run)
    output = tmp_path / "stdout.log"
    errors = tmp_path / "stderr.log"
    diff = tmp_path / "diff.patch"
    diff.write_text("diff --git a/main.py b/main.py\n", encoding="utf-8")

    result = ClaudeAdapter(model="claude-test").invoke(
        "make the change", stdout_path=output, stderr_path=errors, diff_path=diff
    )

    assert result.returncode == 0
    # A REQUESTED model is not a RESOLVED model. The fake emits no model metadata, so the
    # adapter must claim nothing. This previously asserted == "claude-test" (the constructor
    # arg echoed back) -- i.e. the test asserted the forgery the fd-3 design exists to prevent.
    # Making the product satisfy that assertion would have ADDED a self-reported model field.
    assert result.resolved_model is None
    assert output.read_text(encoding="utf-8")


def test_resolved_model_is_read_back_from_output_not_from_the_request(monkeypatch, tmp_path: Path):
    """The other direction: when the CLI DOES report a model, the adapter must read it back --
    and it must be the reported value, not the requested one."""
    def fake_run(command, *, stdin, stdout, stderr, cwd, env, check):
        # valid JSON on its OWN line -- the parser requires a parseable line, correctly, so
        # padding must not share it. (First draft of this fixture appended text to the JSON line
        # and the parser rejected it: my fixture was malformed, not the product.)
        stdout.write(b'{"resolved_model": "claude-actually-served"}\n')
        stdout.write(b'padding line to clear the log-size floor for this invocation\n')
        return SimpleNamespace(returncode=0)
    monkeypatch.setattr("crew.adapters._subprocess.subprocess.run", fake_run)
    # a diff is REQUIRED: the adapter refuses a "successful" invocation that changed nothing.
    # (My first two drafts of this fixture omitted it and read the refusal as a product bug.)
    d = tmp_path / "d.patch"; d.write_text("diff --git a/x b/x\n", encoding="utf-8")
    r = ClaudeAdapter(model="claude-requested").invoke(
        "go", stdout_path=tmp_path / "o.log", stderr_path=tmp_path / "e.log", diff_path=d
    )
    assert r.resolved_model == "claude-actually-served"
    assert r.resolved_model != "claude-requested"


def test_log_floor_refuses_a_false_success(monkeypatch, tmp_path: Path):
    def fake_run(command, *, stdout, **kwargs):
        stdout.write(b"x" * 39)
        return SimpleNamespace(returncode=0)

    monkeypatch.setattr("crew.adapters._subprocess.subprocess.run", fake_run)
    diff = tmp_path / "diff.patch"
    diff.write_text("x", encoding="utf-8")
    with pytest.raises(AdapterError, match="log below floor"):
        ClaudeAdapter().invoke(
            "make the change", stdout_path=tmp_path / "out", stderr_path=tmp_path / "err", diff_path=diff
        )


def test_empty_diff_refuses_a_false_success(monkeypatch, tmp_path: Path):
    monkeypatch.setattr("crew.adapters._subprocess.subprocess.run", _fake_run)
    with pytest.raises(AdapterError, match="empty diff"):
        ClaudeAdapter().invoke(
            "make the change", stdout_path=tmp_path / "out", stderr_path=tmp_path / "err",
            diff_path=tmp_path / "empty.patch",
        )


def test_missing_resolved_model_is_builder_only():
    class BuilderOnly:
        name = "builder-only"

        def invoke(self, *args, **kwargs):
            raise AssertionError("probe must not invoke a model")

        def command(self, prompt):
            return ("operator-model", prompt)

        def resolved_model(self):
            return None

        def usage_record(self):
            return None

        def operator_credentials(self):
            return True

    report = probe_adapter(BuilderOnly())
    assert report.usable_as_builder
    assert not report.usable_as_verifier
    assert "resolved_model_readback" in report.missing
