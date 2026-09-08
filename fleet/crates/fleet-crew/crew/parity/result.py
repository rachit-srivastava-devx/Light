"""The complete outcome of one Welch's-TOST parity run."""

from __future__ import annotations

from dataclasses import dataclass

from .types import ALPHA, DESIGN_EFFECT_SIZE, POWER


@dataclass(frozen=True)
class ParityResult:
    verdict: str
    prompters: tuple[str, str]
    difference: float
    ci_90: tuple[float | None, float | None]
    margin: float
    margin_justification: str
    n: int
    checked: int
    total: int
    n_by_prompter: dict[str, int]
    required_n: int
    tost_p_value: float | None
    lower_test_p_value: float | None
    upper_test_p_value: float | None
    alpha: float = ALPHA
    target_power: float = POWER
    design_effect_size: float = DESIGN_EFFECT_SIZE

    def as_dict(self) -> dict[str, object]:
        return {
            "verdict": self.verdict,
            "prompters": list(self.prompters),
            "difference": self.difference,
            "ci_90": {"low": self.ci_90[0], "high": self.ci_90[1]},
            "margin": self.margin,
            "margin_justification": self.margin_justification,
            "n": self.n,
            "checked": self.checked,
            "total": self.total,
            "n_by_prompter": self.n_by_prompter,
            "required_n": self.required_n,
            "tost": {
                "p_value": self.tost_p_value,
                "lower_one_sided_p_value": self.lower_test_p_value,
                "upper_one_sided_p_value": self.upper_test_p_value,
                "alpha": self.alpha,
            },
            "power_design": {
                "target_power": self.target_power,
                "effect_size_f": self.design_effect_size,
                "required_n": self.required_n,
                "basis": "blueprint S4b: one-df medium-effect design",
            },
        }
