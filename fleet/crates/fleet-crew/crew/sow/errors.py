"""The intent layer's one typed exception."""

from __future__ import annotations


class SOWRefusal(ValueError):
    """A mechanically detected intent defect.

    ``exit_code`` is deliberately typed according to the repository contract.
    The attached receipt is the event a parent ledger writer can persist
    before returning control to its caller.
    """

    exit_code = 7

    def __init__(self, reason: str):
        self.reason = reason
        self.receipt = {
            "event": "refusal",
            "exit_code": self.exit_code,
            "body": {"reason": reason},
        }
        super().__init__(reason)
