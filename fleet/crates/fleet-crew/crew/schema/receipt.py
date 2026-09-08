"""Pydantic mirror of ``../contracts/receipt.v1.json``."""

from __future__ import annotations

from datetime import datetime
from typing import Any, Literal

from pydantic import Field, field_validator

from .contract_model import ContractModel


class Receipt(ContractModel):
    schema_version: Literal["1.0"]
    seq: int = Field(ge=0)
    prev_hash: str = Field(pattern=r"^(GENESIS|blake3:[0-9a-f]{64})$")
    hash: str = Field(pattern=r"^blake3:[0-9a-f]{64}$")
    ts_wall: str
    event: Literal[
        "run_start", "artifact_frozen", "attested", "refusal", "gate_verdict", "run_end"
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
            datetime.fromisoformat(value)
        except ValueError as exc:
            raise ValueError("ts_wall must be an ISO-8601 date-time") from exc
        return value
