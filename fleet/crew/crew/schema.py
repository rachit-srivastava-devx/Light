"""Pydantic models generated from the versioned Fleet JSON contracts.

The three source schemas are:

* ``../contracts/attestation.v1.json``
* ``../contracts/receipt.v1.json``
* ``../contracts/submission.v1.json``

This module intentionally contains only fields present in those schemas.
"""

from __future__ import annotations

from datetime import datetime
from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field, field_validator


class ContractModel(BaseModel):
    """Common strict configuration for contract payloads."""

    model_config = ConfigDict(extra="forbid", populate_by_name=True)


class Digest(ContractModel):
    blake3: str = Field(pattern=r"^[0-9a-f]{64}$")


class Subject(ContractModel):
    name: str
    digest: Digest


class AttestationElements(ContractModel):
    sow: dict[str, Any] | None = None
    blind_suite: dict[str, Any] | None = None
    independent_verification: dict[str, Any] | None = None
    adequacy: dict[str, Any] | None = None
    blast_radius: dict[str, Any] | None = None
    rollback: dict[str, Any] | None = None
    cost: dict[str, Any] | None = None
    oracle_independence: dict[str, Any] | None = None

    @field_validator(
        "sow",
        "blind_suite",
        "independent_verification",
        "blast_radius",
        "cost",
    )
    @classmethod
    def reject_null_object(cls, value: dict[str, Any] | None) -> dict[str, Any]:
        """Those contract properties are objects when present, never null."""

        if value is None:
            raise ValueError("object property cannot be null")
        return value


class AttestationBuilder(ContractModel):
    id: str


class AttestationPredicate(ContractModel):
    tier: Literal["T-min", "T-std", "T-max"]
    elements: AttestationElements
    receipts: list[str]
    builder: AttestationBuilder


class Attestation(ContractModel):
    type_: Literal["https://in-toto.io/Statement/v1"] = Field(alias="_type")
    subject: list[Subject] = Field(min_length=1)
    predicate_type: Literal["https://fleet.local/DeliveryAttestation/v1"] = Field(
        alias="predicateType"
    )
    predicate: AttestationPredicate


class Receipt(ContractModel):
    schema_version: Literal["1.0"]
    seq: int = Field(ge=0)
    prev_hash: str = Field(pattern=r"^(GENESIS|blake3:[0-9a-f]{64})$")
    hash: str = Field(pattern=r"^blake3:[0-9a-f]{64}$")
    ts_wall: str
    event: Literal[
        "run_start",
        "artifact_frozen",
        "attested",
        "refusal",
        "gate_verdict",
        "run_end",
    ]
    actor: str
    resolved_model: str | None = None
    exit_code: int | None = None
    body: dict[str, Any]

    @field_validator("ts_wall")
    @classmethod
    def validate_datetime(cls, value: str) -> str:
        if not isinstance(value, str):
            raise TypeError("ts_wall must be a string")
        try:
            datetime.fromisoformat(value.replace("Z", "+00:00"))
        except ValueError as exc:
            raise ValueError("ts_wall must be an ISO-8601 date-time") from exc
        return value


class Submission(ContractModel):
    schema_version: Literal["1.0"]
    kind: Literal["note", "done", "refuse"]
    body: dict[str, Any]


# Friendly aliases used by callers that name the in-toto statement explicitly.
DeliveryAttestation = Attestation
InTotoStatement = Attestation

__all__ = [
    "Attestation",
    "AttestationBuilder",
    "AttestationElements",
    "AttestationPredicate",
    "DeliveryAttestation",
    "Digest",
    "InTotoStatement",
    "Receipt",
    "Submission",
    "Subject",
]

