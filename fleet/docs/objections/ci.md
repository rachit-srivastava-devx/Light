# CI and release objections

- The worktree has no Git remote, so `Formula/fleet.rb` intentionally uses
  `OWNER/REPOSITORY` placeholders. Replace those values before publishing the formula;
  the release workflow emits `fleet-${GITHUB_REF_NAME}-darwin-universal.tar.gz`.
- This local machine does not have `cargo-audit`. `verify.sh` reports that stage as
  `SKIPPED WITH A REASON` and exits 3 (environment fault); CI installs the tool before
  invoking the same gate.
