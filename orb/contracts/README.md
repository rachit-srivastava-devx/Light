Schema ownership for `lld.v1` moved to `fleet/contracts/` (F02 lane contract §3.1, §5.5): see
`fleet/contracts/module-brief.v1.json`, `fleet/contracts/freeze.v1.json`, `fleet/contracts/lld.v1.json`
(the hand-off wrapper), `fleet/contracts/owners.v1.json`, and the shared fixture corpus at
`fleet/contracts/fixtures/lld/`. The `lld-ready` gate is fleet's; the schema the gate refuses
against must not live with the proposer it refuses (the orb) — see the killed alternative in that
lane contract's §3.1.
