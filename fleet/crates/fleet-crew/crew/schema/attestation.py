"""Pydantic mirror of ``../contracts/attestation.v1.json``."""

from __future__ import annotations

from typing import Any, Literal

from pydantic import Field, field_validator

from .contract_model import ContractModel


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

    @field_validator("sow", "blind_suite", "independent_verification", "blast_radius", "cost")
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
