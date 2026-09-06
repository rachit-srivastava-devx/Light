# Corpus objections

The requested existing runner and ten named template detectors were absent from this worktree.
This patch therefore supplies the runner and all 95 ID files in tests/corpus, within the requested
corpus scope. Rows returning 77 are explicitly excluded from the score because their evidence is
a historical observation, runtime interaction, attribution, or human judgement that cannot be
proven from one repository tree without making the detector guess.

Mechanisable rows use concrete path/content mechanisms: unsafe shell status handling, digest-derived
temporary paths, package re-resolution, invariant comparisons, stdin-loop consumption, mktemp
templates, missing package script targets, tautological assignments, shell-function export,
path-prefix checks, temporary receipt/model storage, POSIX regex mistakes, argument shifting,
mutable CI-tree checks, BSD-only commands, and absolute evidence keys.

Any false positive discovered during tuning must be fixed in its detector, never in the code under
test. The missing-template mismatch is recorded here rather than inferred from another directory.

