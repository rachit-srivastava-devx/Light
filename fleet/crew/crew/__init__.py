"""The Python intent layer for Fleet.

SOW symbols are loaded lazily so ``python -m crew.sow`` does not execute the
module once during package import and again as the requested entry point.
"""

import importlib

__all__ = [
    "AtomicLeaf",
    "Alternative",
    "Challenge",
    "Clarification",
    "MachinePredicate",
    "SOWRefusal",
    "Sow",
    "build_sow",
    "validate_sow",
]


def __getattr__(name: str):
    if name in __all__:
        return getattr(importlib.import_module(".sow", __name__), name)
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
