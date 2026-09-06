#!/usr/bin/env bash
set -eu

command -v witness >/dev/null 2>&1 || {
  echo "witness-smoke: witness is absent" >&2
  exit 3
}
command -v cosign >/dev/null 2>&1 || {
  echo "witness-smoke: cosign is absent" >&2
  exit 3
}

smoke_dir=$(mktemp -d)
trap 'rm -rf "$smoke_dir"' EXIT

run_dir="$smoke_dir/run"
passphrase_file="$smoke_dir/key-passphrase"
attestation="$smoke_dir/witness.attestation"
policy="$smoke_dir/policy.json"
signed_policy="$smoke_dir/policy.signed.json"
artifact="$run_dir/witness-smoke-artifact.txt"
passphrase='local-witness-smoke-passphrase'

mkdir -p "$run_dir"
printf '%s\n' "$passphrase" > "$passphrase_file"

cd "$smoke_dir"
COSIGN_PASSWORD="$passphrase" cosign generate-key-pair >/dev/null

witness --log-level warn run \
  --workingdir "$run_dir" \
  --step local-smoke \
  --attestations environment \
  --signer-file-key-path "$smoke_dir/cosign.key" \
  --signer-file-key-passphrase-path "$passphrase_file" \
  --outfile "$attestation" \
  -- sh -c 'printf "%s\n" witness-smoke > witness-smoke-artifact.txt'

public_key_b64=$(openssl base64 -A < "$smoke_dir/cosign.pub")
keyid=$(shasum -a 256 "$smoke_dir/cosign.pub" | awk '{print $1}')
printf '{"expires":"2035-12-17T23:57:40Z","steps":{"local-smoke":{"name":"local-smoke","attestations":[{"type":"https://witness.dev/attestations/material/v0.1"},{"type":"https://witness.dev/attestations/environment/v0.1"},{"type":"https://witness.dev/attestations/command-run/v0.1"},{"type":"https://witness.dev/attestations/product/v0.1"}],"functionaries":[{"type":"publickey","publickeyid":"%s"}]}},"publickeys":{"%s":{"keyid":"%s","key":"%s"}}}\n' \
  "$keyid" "$keyid" "$keyid" "$public_key_b64" > "$policy"

witness --log-level warn sign \
  --signer-file-key-path "$smoke_dir/cosign.key" \
  --signer-file-key-passphrase-path "$passphrase_file" \
  --infile "$policy" \
  --outfile "$signed_policy"

witness --log-level warn verify \
  --publickey "$smoke_dir/cosign.pub" \
  --policy "$signed_policy" \
  --attestations "$attestation" \
  --artifactfile "$artifact"

# NEGATIVE ARM. The block above proves only that witness ACCEPTS a good artifact, which proves
# nothing about the attestation: a verifier that accepts everything would also pass it. Tamper the
# artifact and require the SAME command to fail. (Added by the lead -- the first draft of this
# smoke test had no failing arm, the same shape as D19's vacuous policy.)
printf '%s\n' "tampered after attestation" > "$artifact"
if witness --log-level warn verify \
  --publickey "$smoke_dir/cosign.pub" \
  --policy "$signed_policy" \
  --attestations "$attestation" \
  --artifactfile "$artifact" >/dev/null 2>&1; then
  echo "witness-smoke: FAIL -- verify ACCEPTED a tampered artifact (vacuous attestation)" >&2
  exit 8
fi

echo "witness-smoke: PASS (accepts genuine, rejects tampered)"
