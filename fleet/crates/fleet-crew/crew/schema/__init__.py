"""Pydantic mirrors generated from the versioned Fleet JSON contracts.

The three source schemas are ``../contracts/{attestation,receipt,submission}.
v1.json``. This package intentionally contains only fields present in those
schemas, and never generates or writes a ``.v1.json`` file itself.
"""

from __future__ import annotations

from .attestation import (
    Attestation,
    AttestationBuilder,
    AttestationElements,
    AttestationPredicate,
    Digest,
    Subject,
)
from .contract_model import ContractModel
from .receipt import Receipt
from .submission import Submission

# Friendly aliases used by callers that name the in-toto statement explicitly.
DeliveryAttestation = Attestation
InTotoStatement = Attestation

__all__ = [
    "Attestation",
    "AttestationBuilder",
    "AttestationElements",
    "AttestationPredicate",
    "ContractModel",
    "DeliveryAttestation",
    "Digest",
    "InTotoStatement",
    "Receipt",
    "Subject",
    "Submission",
]
