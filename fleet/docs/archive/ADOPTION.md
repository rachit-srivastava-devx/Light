# Tool adoption evidence

Run on 2026-08-24 from the repository root on macOS arm64. “Adopted” means an
installation was followed by a real repository smoke command with recorded
output. Installation alone and `--help` output do not qualify.

| Tool | Installed? | Verdict |
|---|---|---|
| `witness` | Yes — `brew install witness` exited `0` (`witness 0.12.0`) | **ADOPTED** — local cosign keypair, real in-toto attestation, and `witness verify` all passed. |
| `conftest` | Yes — `brew install conftest` exited `0` (`conftest 0.69.0`) | **UNADOPTED-WITH-REASON** — the complete fixture smoke cannot run because `arch.rego` does not parse; two policy pairs do pass with explicit legacy-Rego compatibility. |
| `opa` | Yes — `brew install opa` exited `0` (`opa 1.19.1`) | **UNADOPTED-WITH-REASON** — installed and used ONCE interactively (`opa fmt --v0-v1` during the `D19` migration); no script or source invokes it. `conftest` embeds OPA's engine and IS adopted — that is conftest working, not `opa`. Corrected 2026-08-24 by detector `M5`. |
| `cosign` | Yes — `brew install cosign` exited `0` (`cosign 3.1.3`) | **ADOPTED** |
| `rekor` (`rekor-cli`) | `brew install rekor` failed because no formula exists; `brew install rekor-cli` exited `0` (`rekor-cli 1.5.4`) | **EXCLUDED BY THE ARCHITECTURE** — not merely unadopted. `rekor-cli --help` shows `--rekor_server` defaulting to `https://rekor.sigstore.dev`: a HOSTED SERVICE. The keyless constraint is no API key, no login, and no hosted service on a required path, so putting a transparency log on the attestation path would break the property the design exists to hold. Running a local Rekor would reintroduce a server the operator must administer. The claim is withdrawn rather than satisfied. Corrected 2026-08-24 (detector `M5` found it was invoked by nothing; the architecture explains why it should stay that way). — real public-log upload and inclusion verification succeeded. |
| `semgrep` | Yes — already present (`semgrep 1.174.0`) | **ADOPTED**, B11, 2026-09-03 — `bin/semgrep-gate.sh` is a required `verify.sh` stage. A prior session (S1, 2026-08-28) claimed this same adoption, but its gate script and delta doc were never committed (confirmed via `git log --all`), so the claim was never true in the checked-out tree; see `docs/delta.d/B11.md` for the full audit and this session's from-scratch review of every finding. |
| `trivy` | Yes — already present (`trivy 0.74.0`) | **ADOPTED (secret scanner only)**, B11, 2026-09-03 — `bin/trivy-gate.sh` is a required `verify.sh` stage running `trivy fs --scanners secret`, self-tested against fake-but-real-shaped AWS/GitHub credentials before being trusted. `vuln`/`misconfig` scanning is a separate, larger commitment (DB download on first run) tracked open, not silently dropped — see `docs/delta.d/B11.md`. Same never-committed history as `semgrep` above. |

## `witness`

The reproducible smoke test was `bin/witness-smoke.sh` (this script no longer exists in the tree
after the `keel/`→`crates/` migration; not relocated, kept here only as a historical pointer). It
uses a disposable `mktemp -d`, a local encrypted cosign keypair, and no Fulcio,
OIDC, Archivista, timestamp authority, or Rekor service. The Witness policy is
also signed locally with that key so `witness verify` exercises both attestation
and policy signature verification.

Exact commands:

```text
smoke_dir=$(mktemp -d)
run_dir="$smoke_dir/run"
mkdir -p "$run_dir"
passphrase_file="$smoke_dir/key-passphrase"
printf '%s\n' 'local-witness-smoke-passphrase' > "$passphrase_file"
cd "$smoke_dir"
COSIGN_PASSWORD='local-witness-smoke-passphrase' cosign generate-key-pair
witness --log-level warn run --workingdir "$run_dir" --step local-smoke --attestations environment --signer-file-key-path "$smoke_dir/cosign.key" --signer-file-key-passphrase-path "$passphrase_file" --outfile "$smoke_dir/witness.attestation" -- sh -c 'printf "%s\\n" witness-smoke > witness-smoke-artifact.txt'
public_key_b64=$(openssl base64 -A < "$smoke_dir/cosign.pub")
keyid=$(shasum -a 256 "$smoke_dir/cosign.pub" | awk '{print $1}')
printf '{"expires":"2035-12-17T23:57:40Z","steps":{"local-smoke":{"name":"local-smoke","attestations":[{"type":"https://witness.dev/attestations/material/v0.1"},{"type":"https://witness.dev/attestations/environment/v0.1"},{"type":"https://witness.dev/attestations/command-run/v0.1"},{"type":"https://witness.dev/attestations/product/v0.1"}],"functionaries":[{"type":"publickey","publickeyid":"%s"}]}},"publickeys":{"%s":{"keyid":"%s","key":"%s"}}}\n' "$keyid" "$keyid" "$keyid" "$public_key_b64" > "$smoke_dir/policy.json"
witness --log-level warn sign --signer-file-key-path "$smoke_dir/cosign.key" --signer-file-key-passphrase-path "$passphrase_file" --infile "$smoke_dir/policy.json" --outfile "$smoke_dir/policy.signed.json"
witness --log-level warn verify --publickey "$smoke_dir/cosign.pub" --policy "$smoke_dir/policy.signed.json" --attestations "$smoke_dir/witness.attestation" --artifactfile "$run_dir/witness-smoke-artifact.txt"
verify_status=$?
printf 'verify exit=%s\n' "$verify_status"
exit "$verify_status"
```

Exact output:

```text
Private key written to cosign.key
Public key written to cosign.pub
verify exit=0
```

The run produced a 10,672-byte DSSE/in-toto attestation and `witness verify`
exited `0`. The checked artifact was created by the witnessed command, not
pre-seeded in the working directory.

## `conftest`

The repository-provided command was run first:

```text
./policy/run.sh
```

Exact output:

```text
Error: running test: load: loading policies: load: 2 errors occurred during loading:
/Users/rachitsrivastava/youtube/Principal Engineering/fleet-wt-adopt/policy/measured_nothing.rego:5: rego_parse_error: `if` keyword is required before rule body
/Users/rachitsrivastava/youtube/Principal Engineering/fleet-wt-adopt/policy/measured_nothing.rego:5: rego_parse_error: `contains` keyword is required for partial set rules
Error: running test: load: loading policies: load: 2 errors occurred during loading:
/Users/rachitsrivastava/youtube/Principal Engineering/fleet-wt-adopt/policy/measured_nothing.rego:5: rego_parse_error: `if` keyword is required before rule body
/Users/rachitsrivastava/youtube/Principal Engineering/fleet-wt-adopt/policy/measured_nothing.rego:5: rego_parse_error: `contains` keyword is required for partial set rules
policy: GOOD fixture rejected: /Users/rachitsrivastava/youtube/Principal Engineering/fleet-wt-adopt/policy/fixtures/measured_nothing.good.json
```

Exit: `8`.

To distinguish syntax-version incompatibility from policy behavior, the six
bad/good commands were also run with the explicit legacy parser and all
namespaces:

```text
conftest test --no-color --rego-version v0 --all-namespaces --policy policy/measured_nothing.rego policy/fixtures/measured_nothing.bad.json
conftest test --no-color --rego-version v0 --all-namespaces --policy policy/measured_nothing.rego policy/fixtures/measured_nothing.good.json
conftest test --no-color --rego-version v0 --all-namespaces --policy policy/attest.rego policy/fixtures/attest.bad.json
conftest test --no-color --rego-version v0 --all-namespaces --policy policy/attest.rego policy/fixtures/attest.good.json
conftest test --no-color --rego-version v0 --all-namespaces --policy policy/arch.rego policy/fixtures/arch.bad.json
conftest test --no-color --rego-version v0 --all-namespaces --policy policy/arch.rego policy/fixtures/arch.good.json
```

Exact output:

```text
FAIL - policy/fixtures/measured_nothing.bad.json - fleet.policy.measured_nothing - measured_nothing: checked must be greater than zero

1 test, 0 passed, 0 warnings, 1 failure, 0 exceptions
measured_nothing.bad exit=1

1 test, 1 passed, 0 warnings, 0 failures, 0 exceptions
measured_nothing.good exit=0
FAIL - policy/fixtures/attest.bad.json - fleet.policy.attest - attest: missing predicate.elements.oracle_independence

1 test, 0 passed, 0 warnings, 1 failure, 0 exceptions
attest.bad exit=1

1 test, 1 passed, 0 warnings, 0 failures, 0 exceptions
attest.good exit=0
Error: running test: load: loading policies: load: 2 errors occurred during loading:
policy/arch.rego:16: rego_parse_error: unexpected package keyword
	package_has_blake3(package) {
	                   ^
policy/arch.rego:16: rego_parse_error: unexpected package keyword: expected rule head name
	package_has_blake3(package) {
	                   ^
arch.bad exit=1
Error: running test: load: loading policies: load: 2 errors occurred during loading:
policy/arch.rego:16: rego_parse_error: unexpected package keyword
	package_has_blake3(package) {
	                   ^
policy/arch.rego:16: rego_parse_error: unexpected package keyword: expected rule head name
	package_has_blake3(package) {
	                   ^
arch.good exit=1
```

## `opa`

Commands:

```text
opa eval --v0-compatible -d policy/measured_nothing.rego -i policy/fixtures/measured_nothing.bad.json 'data.fleet.policy.measured_nothing.deny'
opa eval --v0-compatible -d policy/measured_nothing.rego -i policy/fixtures/measured_nothing.good.json 'data.fleet.policy.measured_nothing.deny'
```

Exact output:

```text
{
  "result": [
    {
      "expressions": [
        {
          "value": [
            "measured_nothing: checked must be greater than zero"
          ],
          "text": "data.fleet.policy.measured_nothing.deny",
          "location": {
            "row": 1,
            "col": 1
          }
        }
      ]
    }
  ]
}
bad fixture exit=0
{
  "result": [
    {
      "expressions": [
        {
          "value": [],
          "text": "data.fleet.policy.measured_nothing.deny",
          "location": {
            "row": 1,
            "col": 1
          }
        }
      ]
    }
  ]
}
good fixture exit=0
```

`--v0-compatible` is required because the repository's policy uses legacy Rego
syntax.

## `cosign`

The exact smoke script used a `mktemp -d` outside the repository, generated a
local key pair, signed the repository's `docs/CONFORMANCE.md`, and verified it:

```text
smoke_dir=$(mktemp -d)
cd "$smoke_dir"
COSIGN_PASSWORD= cosign generate-key-pair
cosign signing-config create --no-default-fulcio --no-default-oidc --no-default-rekor --no-default-tsa --out signing-config.json
COSIGN_PASSWORD= cosign sign-blob --key cosign.key --signing-config signing-config.json --bundle conformance.bundle "/Users/rachitsrivastava/youtube/Principal Engineering/fleet-wt-adopt/docs/CONFORMANCE.md"
COSIGN_PASSWORD= cosign verify-blob --key cosign.pub --bundle conformance.bundle --insecure-ignore-tlog "/Users/rachitsrivastava/youtube/Principal Engineering/fleet-wt-adopt/docs/CONFORMANCE.md"
```

Exact output:

```text
Private key written to cosign.key
Public key written to cosign.pub
Using payload from: /Users/rachitsrivastava/youtube/Principal Engineering/fleet-wt-adopt/docs/CONFORMANCE.md
Signing artifact...
Wrote bundle to file conformance.bundle
WARNING: Skipping tlog verification is an insecure practice that lacks transparency and auditability verification for the blob.
Verified OK
```

## `rekor` (`rekor-cli`)

`brew install rekor` produced `No available formula with the name "rekor"`; the
Homebrew formula is `rekor-cli`, which installed successfully. A local cosign
signature was uploaded to and then inclusion-verified against the public
`https://rekor.sigstore.dev` service:

```text
smoke_dir=$(mktemp -d)
cd "$smoke_dir"
COSIGN_PASSWORD= cosign generate-key-pair >/dev/null 2>&1
cosign signing-config create --no-default-fulcio --no-default-oidc --no-default-rekor --no-default-tsa --out signing-config.json >/dev/null 2>&1
COSIGN_PASSWORD= cosign sign-blob --key cosign.key --signing-config signing-config.json --bundle conformance.bundle "/Users/rachitsrivastava/youtube/Principal Engineering/fleet-wt-adopt/docs/CONFORMANCE.md" >/dev/null 2>&1
jq -r '.messageSignature.signature' conformance.bundle | base64 -D > conformance.sig
rekor-cli --format json upload --rekor_server https://rekor.sigstore.dev --retry 0 --timeout 30s --store_tree_state=false --artifact "/Users/rachitsrivastava/youtube/Principal Engineering/fleet-wt-adopt/docs/CONFORMANCE.md" --signature conformance.sig --public-key cosign.pub
rekor-cli --format json verify --rekor_server https://rekor.sigstore.dev --retry 0 --timeout 30s --artifact "/Users/rachitsrivastava/youtube/Principal Engineering/fleet-wt-adopt/docs/CONFORMANCE.md" --signature conformance.sig --public-key cosign.pub
```

Exact output:

```text
{"AlreadyExists":false,"Location":"/api/v1/log/entries/108e9186e8c5677a492b127c6827fe7dbea3623d6855f6f0865a4a7c83920ffb59c8166c3b709335","Index":2578429824}
{"RootHash":"35f50d9f51520a301fc947e2977a97227c5855fc55ffe1855b7a3e8e21240194","EntryUUID":"108e9186e8c5677a492b127c6827fe7dbea3623d6855f6f0865a4a7c83920ffb59c8166c3b709335","Index":2456525562,"Size":2456526592,"Hashes":["f3a40beb552dd937a8b39a9122cda75f65ab368cc894d9078127101f2a814e51","9aa2111ca8a07599d1f349678351dc1c92543fad77b92f411682c9544e5163f3","0a748ebe7383c7b1deca597605c482ebde224cdbb4d406e95cdc43175c89960c","9a5a8cf1d82d010260c5f9f93ebcb3784134f3168c4d4bb8feb2ad3930433e7e","13fb6c23c611e8c06dfdfe99c754d39d45c511268c2bf68f6d65affb4d4c2442","7b92c813a735e7fbec672368c191de795b7b03ec5e8be05a97b0aac5b68ccf1f","a0bba6f055603e06c60a8eae1f3597f01d69d32a2dae47efeef43f7318545029","4282908f23826a4ccd19bb4001171fd3073c3b1b657d515da0ccc7ab99e9d6cc","281989cdde95a839336a18df61b0abe475a94f586bcaba1a592905ab4a35203e","1f9df2f26cc366482ba979ca14dca872d8c19b417181336033b68247ae532268","e672d626b73f1e104266251a2b583d3bf494c5c9bbbd7ebd9e84fd3b8670f715","9e1d317cd8baa5349787e95a9872c9b1cdef6d33ca6bbd37c24b2d521c74d4df","56f55059108066573e728851137ad423d1188190c045e07a7338cfc61706af85","efae7c6b29386dc3f2a4205806b3d317979b8f9de3106920763696df2a1a2f75","e91c277ce07471f4c6f1f0ad6f5b7d280bf289f0d428312e3efeb83316f58a94","b2e78e441d7964fe457017608d55fb405bbf32eb4f2b954e0a70f8ddd9cbde10","afed4df3b0787983fec6da9e498f81c72675c7876de9ee397dfff142104514b4","48b74843e888a0827c1f636b94e5ed61f2993aa6bbcbfa52fabdc5cb990a3942","d028ed3350c9bec066f3a740067f95726f92b9ea88e65b72cf86f9015f3b4797","6fcd3f27febc442f3fb71e7145d94a93a3264e9662551a2daae581ac4ff3d692","4a775b30a56d7137a7024c22d8905f1b30fe9a17b1a75a896d1218f80d494485","c47fc30ac78b1ebf5e2a8613f2ab0e4592bbcd5744198587b95b6c56b0fde706"],"Checkpoint":"rekor.sigstore.dev - 1193050959916656506\n2456526592\nNfUNn1FSCjAfyUfil3qXInxYVfxV/+GFW3o+jiEkAZQ=\n\n— rekor.sigstore.dev wNI9ajBFAiBwcerJ+EhO0Xq5UQ8Pg5P0oHRxgOKLTCQ4i5Z738TZQgIhAJqod79KG+uXZ1AywqLvWtowijOPnJUq+qSjCkmbMj1/\n"}
```

No policy, fleet code, verifier, test, or contract file was changed to
manufacture a pass.
