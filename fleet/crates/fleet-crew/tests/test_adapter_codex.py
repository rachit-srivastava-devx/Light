from pathlib import Path
from types import SimpleNamespace

from crew.adapters.codex import CodexAdapter


def test_command_shape_uses_codex_exec():
    adapter = CodexAdapter(model="codex-test")
    assert list(adapter.command("do it")) == ["codex", "exec", "--model", "codex-test", "do it"]


def test_command_omits_model_flag_when_unset():
    adapter = CodexAdapter()
    assert list(adapter.command("do it")) == ["codex", "exec", "do it"]


def test_invoke_shares_mechanics_with_claude(monkeypatch, tmp_path: Path):
    """Thin parity check: CodexAdapter routes through the same `invoke_cli`
    helper as ClaudeAdapter, so most invocation behavior is tested once."""

    def fake_run(command, *, stdin, stdout, stderr, cwd, env, check):
        stdout.write(b"codex output with enough evidence to clear the floor\n")
        return SimpleNamespace(returncode=0)

    monkeypatch.setattr("crew.adapters.runner.subprocess.run", fake_run)
    diff = tmp_path / "diff.patch"
    diff.write_text("diff --git a/x b/x\n", encoding="utf-8")

    result = CodexAdapter().invoke(
        "go", stdout_path=tmp_path / "o.log", stderr_path=tmp_path / "e.log", diff_path=diff
    )

    assert result.returncode == 0
    assert result.resolved_model is None
    assert CodexAdapter().operator_credentials() is True
