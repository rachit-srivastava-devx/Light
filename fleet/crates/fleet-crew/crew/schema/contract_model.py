"""Base strict-parsing configuration shared by every contract mirror."""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class ContractModel(BaseModel):
    """Common strict configuration for contract payloads."""

    model_config = ConfigDict(extra="forbid", populate_by_name=True)
