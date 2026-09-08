"""Pydantic mirror of ``../contracts/submission.v1.json``."""

from __future__ import annotations

from typing import Any, Literal

from .contract_model import ContractModel


class Submission(ContractModel):
    schema_version: Literal["1.0"]
    kind: Literal["note", "done", "refuse"]
    body: dict[str, Any]
