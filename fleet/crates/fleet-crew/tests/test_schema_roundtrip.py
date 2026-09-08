import pytest
from pydantic import ValidationError

from crew.schema import Attestation, DeliveryAttestation, InTotoStatement, Receipt, Submission


def _valid_receipt() -> dict:
    return {
        "schema_version": "1.0",
        "seq": 0,
        "prev_hash": "GENESIS",
        "hash": "blake3:" + "a" * 64,
        "ts_wall": "2026-09-07T00:00:00Z",
        "event": "run_start",
        "actor": "operator",
        "body": {},
    }


def test_receipt_round_trips_and_rejects_extra_field() -> None:
    """§9 new test: a valid Receipt payload parses; an undeclared key raises
    pydantic's ValidationError -- `extra="forbid"` is enforced, not just
    declared."""

    receipt = Receipt(**_valid_receipt())
    assert receipt.event == "run_start"

    with pytest.raises(ValidationError):
        Receipt(**{**_valid_receipt(), "unexpected_field": True})


def test_receipt_rejects_malformed_hash() -> None:
    with pytest.raises(ValidationError):
        Receipt(**{**_valid_receipt(), "hash": "not-a-hash"})


def test_receipt_rejects_non_iso_timestamp() -> None:
    with pytest.raises(ValidationError):
        Receipt(**{**_valid_receipt(), "ts_wall": "not a timestamp"})


def _valid_attestation() -> dict:
    return {
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{"name": "artifact", "digest": {"blake3": "b" * 64}}],
        "predicateType": "https://fleet.local/DeliveryAttestation/v1",
        "predicate": {
            "tier": "T-min",
            "elements": {},
            "receipts": [],
            "builder": {"id": "claude"},
        },
    }


def test_attestation_round_trips_and_rejects_extra_field() -> None:
    attestation = Attestation(**_valid_attestation())
    assert attestation.predicate.tier == "T-min"
    assert DeliveryAttestation is Attestation
    assert InTotoStatement is Attestation

    with pytest.raises(ValidationError):
        Attestation(**{**_valid_attestation(), "extra": 1})


def test_attestation_rejects_null_object_element() -> None:
    payload = _valid_attestation()
    payload["predicate"] = {**payload["predicate"], "elements": {"sow": None}}
    with pytest.raises(ValidationError):
        Attestation(**payload)


def test_submission_round_trips_and_rejects_extra_field() -> None:
    submission = Submission(schema_version="1.0", kind="done", body={"note": "ok"})
    assert submission.kind == "done"

    with pytest.raises(ValidationError):
        Submission(schema_version="1.0", kind="done", body={}, extra="nope")
