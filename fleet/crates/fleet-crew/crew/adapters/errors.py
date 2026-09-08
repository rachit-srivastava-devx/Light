"""The adapter package's one typed exception."""

from __future__ import annotations


class AdapterError(RuntimeError):
    """A model invocation failed an adapter invariant (log-size floor, empty diff, ...)."""

    exit_code = 6
