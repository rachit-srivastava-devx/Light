"""The atomic-requirement value types: one predicate per leaf, enforced."""

from __future__ import annotations

from dataclasses import dataclass

from .errors import SOWRefusal


@dataclass(frozen=True)
class MachinePredicate:
    """One machine-evaluable acceptance expression."""

    expression: str

    def __post_init__(self) -> None:
        if not self.expression.strip():
            raise SOWRefusal("acceptance predicate is empty")


@dataclass(frozen=True)
class AtomicLeaf:
    """An atomic requirement with exactly one acceptance predicate."""

    id: str
    requirement: str
    predicates: tuple[MachinePredicate | str, ...]

    def __post_init__(self) -> None:
        if not self.id.strip() or not self.requirement.strip():
            raise SOWRefusal("atomic leaf must have an id and requirement")
        normalized = tuple(
            predicate if isinstance(predicate, MachinePredicate) else MachinePredicate(str(predicate))
            for predicate in self.predicates
        )
        object.__setattr__(self, "predicates", normalized)
        if len(normalized) != 1:
            raise SOWRefusal(
                f"leaf {self.id!r} has {len(normalized)} acceptance predicates; expected exactly one"
            )

    @property
    def acceptance_predicates(self) -> tuple[MachinePredicate, ...]:
        return self.predicates  # type: ignore[return-value]

    @property
    def acceptance_predicate(self) -> MachinePredicate:
        return self.acceptance_predicates[0]
