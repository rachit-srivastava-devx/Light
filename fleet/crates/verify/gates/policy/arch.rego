package fleet.policy.arch

import rego.v1

# There must be one runtime entry point in the judged tree.
deny contains msg if {
	count(input.main_rs) > 1
	msg := sprintf("arch: refuse more than one main.rs (found %d)", [count(input.main_rs)])
}

deny contains msg if {
	input.main_rs_count > 1
	msg := sprintf("arch: refuse more than one main.rs (found %d)", [input.main_rs_count])
}

# A package that may run in production must use the real blake3 dependency
# when the tree evidence identifies a hand-rolled permutation.
package_has_blake3(pkg) if {
	pkg.runtime_dependencies[_] == "blake3"
}

package_has_blake3(pkg) if {
	pkg.runtime_deps[_] == "blake3"
}

package_lacks_blake3(pkg) if {
	not package_has_blake3(pkg)
}

tree_has_hand_rolled_permutation if {
	input.hand_rolled_permutation == true
}

tree_has_hand_rolled_permutation if {
	input.tree.hand_rolled_permutation == true
}

tree_has_hand_rolled_permutation if {
	input.tree[_].hand_rolled_permutation == true
}

deny contains msg if {
	pkg := input.packages[_]
	package_lacks_blake3(pkg)
	tree_has_hand_rolled_permutation
	msg := sprintf("arch: package %s lacks runtime blake3 while the tree contains a hand-rolled permutation", [pkg.name])
}
