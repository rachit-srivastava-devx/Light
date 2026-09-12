package fleet.policy.attest

import rego.v1

# The eighth element is intentionally non-droppable at every attestation tier.
required_elements := [
	"sow",
	"blind_suite",
	"independent_verification",
	"adequacy",
	"blast_radius",
	"rollback",
	"cost",
	"oracle_independence",
]

deny contains msg if {
	element := required_elements[_]
	not input.predicate.elements[element]
	msg := sprintf("attest: missing predicate.elements.%s", [element])
}
