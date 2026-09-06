# A configured lane that had never once worked

`bin/lanes.conf` listed DevToolBox as an adopted failover lane. It has never answered a single
call. `bin/freelane.sh` built one hardcoded OpenAI body — `{"model","messages"}` — for every lane
regardless of what that lane accepts, and DevToolBox requires `{"prompt": ...}`. Every request it
ever received came back `HTTP 400 Missing "prompt" field`.

Proven side by side before changing anything:

| request | result |
|---|---|
| `{"model","messages":[…]}` (what freelane sent) | **HTTP 400** `{"error": "Missing \"prompt\" field"}` |
| `{"prompt":"…","model":"…"}` (what it wants) | **HTTP 200** `{"response":"KEYLESS_OK"}` |
| through `bin/freelane.sh` as wired | `all 1 lanes unavailable; tried=…:http-400` |

This is owner principle #1 exactly — *adopting a tool is not the tool working*. The lane was
probed by hand with `curl`, confirmed keyless, written into the config with a provenance comment,
and never once exercised **through the code path that would use it**. The probe and the consumer
sent different bytes.

## The fix, and the part that matters

Lane records take an optional third field: `url|model[|dialect]`. `openai` (default) and `prompt`.

The load-bearing decision is that an **unknown dialect is refused, not defaulted**. Silently
falling back to `openai` is precisely what hid this for as long as it was hidden — a lane sent the
wrong body fails with an HTTP error indistinguishable from the service being down. It now says:

```
freelane: ignoring lane with unknown dialect 'klingon' (known: openai prompt): https://…
```

A second latent bug surfaced while fixing the first: the parser did `data["error"].get("message")`,
but DevToolBox returns `{"error": "…"}` as a **bare string**. `.get` on a `str` raises
`AttributeError`, so a perfectly clear gateway message was being reported as
`unparseable-response`. Both string and dict error shapes are handled now.

## Verified

End to end, the lane that never worked:

```
KEYLESS_OK
[resolved_model=llama-3.2-3b-instruct requested=llama-3.2-3b-instruct lane=1/1
 tried=devtoolbox-api…:answered usage={"prompt_tokens": null, …}]   exit 0
```

Usage is `null`, not `0` — the endpoint reports no token counts, and an unmeasured lane is not a
free lane.

Parser unit-tested directly on six inputs, including the one that matters: an **openai body parsed
under the `prompt` dialect exits 11 (unparseable) rather than silently returning a wrong answer**.
Real `bin/lanes.conf` end to end: 3 lanes parse, first fails to a dead host, second answers with
real token counts — failover across a mixed-dialect config works.

## Not covered

`FLEET_MUTANTS=0 bash verify.sh` timed out at 580s (load average 53; four codex workers are
building concurrently) and reported a `fmt` FAIL in `keel/fleet/src/intent.rs` — a **different
agent's in-flight edit**, not this change, which touches no Rust. `cargo fmt --check` is clean on
re-run. `shellcheck -S error` and `bash -n` pass on `bin/freelane.sh`. The full gate has not been
observed green for this change yet.

No detector asserts that every lane in `lanes.conf` answers **through `freelane.sh`** rather than
through a hand `curl`. That is the check that would have caught this, and it does not exist.
