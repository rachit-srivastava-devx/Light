from crew.adapters.capability_probe import probe_adapter
from crew.adapters.claude import ClaudeAdapter
from crew.adapters.codex import CodexAdapter


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


def test_missing_resolved_model_is_builder_only():
    report = probe_adapter(BuilderOnly())
    assert report.usable_as_builder
    assert not report.usable_as_verifier
    assert "resolved_model_readback" in report.missing


def test_probe_never_invokes_a_model():
    # BuilderOnly.invoke raises if called; a passing probe proves it wasn't.
    probe_adapter(BuilderOnly())


def test_empty_object_probes_as_fully_incapable():
    report = probe_adapter(object())
    assert report.missing == (
        "non_interactive_invoke",
        "output_files_and_exit_code",
        "closable_stdin",
        "resolved_model_readback",
        "local_usage_record",
        "operator_credentials",
    )
    assert not report.usable_as_builder
    assert not report.usable_as_verifier


def test_credentials_raising_is_treated_as_incapable_not_propagated():
    class Flaky(BuilderOnly):
        def operator_credentials(self):
            raise RuntimeError("boom")

    report = probe_adapter(Flaky())
    assert report.operator_credentials is False


def test_adapter_and_probe_agree_on_verifier_eligibility():
    """§9 new integration test: both concrete adapters declare
    `supports_resolved_model_readback = True`, so both must probe as
    verifier-eligible -- catches a future adapter forgetting the flag."""

    for adapter in (ClaudeAdapter(), CodexAdapter()):
        report = probe_adapter(adapter)
        assert report.verifier_eligible is True, adapter.name
