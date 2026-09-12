package fleet.policy.measured_nothing

import rego.v1

# A denominator is part of the verdict.  Measuring zero inputs is a failure,
# even when the producer claims that no violations were found.
deny contains msg if {
	input.checked == 0
	msg := "measured_nothing: checked must be greater than zero"
}
