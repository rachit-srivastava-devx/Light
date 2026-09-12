# BLUEPRINT — user_cli

## 1. Identity and LLD path

- Node id / label / tag: `user_cli / User / CLI / external`
- LLD authority: `docs/LLD/LLD.md §21`, `docs/LLD/LLD-META-L8-ADDENDUM.md §H`, and `docs/LLD/lld-full-detail.architecture.json:components[id=user_cli]`
- Why this node exists: Accept operator requests, questions, pauses, feedback, and approvals without owning workflow decisions.
- Incoming edges: `questions -> user_cli` with at most three concise questions; `plan_review -> user_cli` walkthrough and feedback; `approval -> user_cli` PR and explanation.
- Outgoing edges: `user_cli -> ingest` request, pause, or feedback.
- Build status: `partial`
- Exact workflow path: `user_cli/connectors -> ingest -> store/control -> intent -> route -> scan`.

## 2. Responsibility and non-goals

**Owns:** command parsing, human/JSON presentation, request IDs, input size limits, and conversion of user actions into typed ingress messages.

**Does not own:** intent classification or workflow selection, policy, grants, persistence, model choice, Git mutation, or external publication. Those belong to `intent`, `control`, `route`, `store`, and `broker`.

## 3. Boundary and authority

The terminal is an untrusted presentation boundary. Parse with `clap`; reject unknown flags, empty effectful commands, oversized UTF-8 input, and shell metacharacters passed as command text. The CLI may request pause, resume, approval, or cancellation but cannot authorize an effect without a controller-issued grant. It writes no ledger directly; `control` receives a typed `CliEvent` and persists it through `store`.

## 4. Crate/package layout

```text
crates/user-cli/
  Cargo.toml                 # crate manifest, about 20 lines
  src/lib.rs                 # typed command/event exports, about 35 lines
  src/parse.rs               # clap parser and bounded text, about 70 lines
  src/render.rs              # JSON/human projection, about 70 lines
  tests/real_binary.rs       # env CARGO_BIN_EXE_fleet CLI proof, about 65 lines
```

`src/lib.rs` is wiring only. No module owns a state transition or side effect.

## 5. Public API contract

```rust
pub enum CliCommand { Submit { request: String }, Pause, Resume, Cancel, Approve { grant: String }, Status }
pub struct CliEvent { pub request_id: String, pub actor: String, pub command: CliCommand }
pub struct InputEnvelope { pub source: String, pub delivery_id: String, pub schema_version: u64, pub actor: String, pub text: String, pub payload_digest: String, pub auth_metadata_ref: Option<String> }
pub enum CliError { EmptyRequest, TooLarge { bytes: usize }, InvalidGrant, UnknownCommand, Render(String) }
pub fn parse<I, T>(args: I) -> Result<CliCommand, CliError> where I: IntoIterator<Item=T>, T: Into<std::ffi::OsString> + Clone;
pub fn to_event(command: CliCommand, request_id: String, actor: String) -> Result<CliEvent, CliError>;
pub fn render<T: serde::Serialize>(value: &T, json: bool) -> Result<String, CliError>;
```

`to_event` must also expose an explicit `InputEnvelope` adapter for `user_cli -> ingest`; the
canonical fields are not replaced by the local `CliEvent` shape.

Preconditions: request text is valid UTF-8 and at most 64 KiB; postcondition: parsing creates no effect. All failures are returned, never panicked.

## 6. Data model, invariants, and failure taxonomy

| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `CliEvent` | nonempty request ID and actor; command is typed | anonymous or ambiguous audit input | `Refusal`, 7 |
| `Submit` | trimmed request has 1–65536 bytes | empty/oversized work | 7 / 6 |
| `Approve` | grant is opaque nonempty ID; never inferred | CLI-created authority | 7 |
| rendered report | JSON is valid and counts are explicit | misleading human-only output | 8 on serialization mismatch |

IDs and timestamps are supplied by `control`; CLI does not invent durable time. Rendering is `Send + Sync`; parsing is pure.

## 7. Reuse map and adoption decisions

| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `src/cli/root.rs:8-77` | local LV; `clap` derive already used | extract subcommand vocabulary | clap handles platform argument edge cases | real binary parses every command |
| `src/interactive/commands.rs:7-57` | local LV | preserve slash-command outcomes | avoid a second command grammar | unknown/empty/Unicode tests |
| `clap` | upstream docs https://docs.rs/clap/latest/clap/; version pinned by workspace lock | parse argv | maintained parser | `cargo check` and binary smoke |
| `serde` | upstream docs https://serde.rs/ | JSON projection | standard serialization | malformed value refusal |

The existing `src/dispatch/route_cmd.rs:25-41` is composition evidence only; its output is not the CLI node's authority.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror  = "2"
```

## 8. Behavior matrix

### `parse` and `to_event`

| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | `Submit` returns `EmptyRequest`, exit 7; status may be valid without a request |
| wrong type / Unicode | clap rejects invalid argv; valid Unicode is preserved and counted in UTF-8 bytes |
| huge / negative | text over 65536 bytes refuses; numeric flags use checked parsing and reject negative values |
| duplicate / concurrent | duplicate request IDs are accepted by CLI but `store` deduplicates; no local retry side effect |
| partial output / timeout | render returns `Render`; stderr progress does not masquerade as JSON |
| stale / unavailable | approval is only submitted; controller returns stale-grant refusal |

## 9. Tiny implementation steps

1. In `src/lib.rs`: Define `CliCommand`, `CliEvent`, and `CliError` typed exports; `cargo check -p user-cli` exits 0.
2. In `src/parse.rs`: Implement the clap parser covering all 34 commands (29 visible + 5 hidden: `__pipeline_probe`, `__planahead_probe`, `__agent`, `__spawn_probe`, `__capacity_probe`); `cargo test -p user-cli parse_all_34_commands_are_recognized` passes.
3. In `src/parse.rs`: Add byte-length bounds check rejecting empty and >65536-byte requests; `cargo test -p user-cli parse_rejects_empty_submit` and `cargo test -p user-cli parse_rejects_oversized_request` pass.
4. In `src/render.rs`: Implement JSON and human-readable projections asserting `checked` and `total` fields survive round-trip; `cargo test -p user-cli render_is_valid_json` passes.
5. In `tests/real_binary.rs`: Use `env!("CARGO_BIN_EXE_fleet")` to invoke the real binary; assert `--version`, `route --json`, and unknown command exit codes; `cargo test --test real_binary` passes.

## 10. Test matrix

**Unit tests:** `parse_rejects_empty_submit`, `parse_preserves_unicode`, `render_is_valid_json` — public functions and observable errors.

**Integration/contract tests:** `cli_event_matches_ingest_contract` — serialized `CliEvent` is accepted by `ingest` with request, pause, and feedback variants.

**Hidden tests:** controller-authored oversized input, forged approval text, and duplicate request ID — catches bypassed limits and authority leakage.

**Property tests:** parser/render round-trip for 1,000 fixed-seed Unicode strings under 64 KiB; no output parser panic.

**Differential tests:** compare extracted visible command names against `src/cli/root.rs:18-76`; divergence is allowed only for explicitly deprecated aliases recorded in the test.

**Real-binary/effect test:** `cargo test --test real_binary`; real `fleet --version` and `fleet route --json`, no fake executable.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `user_cli::tests::parse_rejects_empty_submit` | `parse(["submit", ""])` — submit subcommand with an empty string argument | `Err(CliError::EmptyRequest)` | catches "return constant Submit success" stub — a zero-byte input must refuse |
| `user_cli::tests::parse_rejects_oversized_request` | `parse(["submit", &"x".repeat(65537)])` — submit with a 65 537-byte request body | `Err(CliError::TooLarge { bytes: 65537 })` | catches removing the byte-length bounds check; oversized input must not reach `to_event` |
| `user_cli::tests::render_is_valid_json` | `render(&CliEvent { request_id: "r1", actor: "a1", command: CliCommand::Status }, true)` | `Ok(s)` where `s` parses as valid JSON containing `"request_id"` and `"actor"` keys | catches render stub returning `"{}"` — required fields and denominators must survive round-trip |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `parse` in `src/parse.rs` | Return `Ok(CliCommand::Submit { request: "ok".into() })` for any input without inspecting the argument | `user_cli::tests::parse_rejects_empty_submit` and `user_cli::tests::parse_rejects_oversized_request` | Input bounds are enforced at parse time; a constant-success parser skips both the empty and size checks |
| `to_event` in `src/lib.rs` | Drop the `Pause` command variant, returning `Err(UnknownCommand)` instead of `Ok(CliEvent { command: Pause })` | `cli_event_matches_ingest_contract` (pause variant) | Lifecycle control cannot disappear; the ingest contract test sends a pause and asserts the typed event is present |
| `render` in `src/render.rs` | Return `Ok("{}".to_string())` unconditionally ignoring the serialized value | `user_cli::tests::render_is_valid_json` | Required fields and denominators must survive round-trip; an empty object fails the JSON key assertions |
| `parse` in `src/parse.rs` | Accept any non-empty string as a valid `Approve` grant without format validation | forged grant hidden test | CLI cannot mint authority; structural validation must reject arbitrary strings |

The independent reviewer manually applies the dropped-pause mutation and records the failing test.

Safety mutation floor: `caught/total >= 75%`; reviewer manually enables one bypass mutation and confirms kill.

## 12. Verification recipe and denominators

```bash
cargo test -p user-cli --no-fail-fast
cargo clippy -p user-cli --all-targets -- -D warnings
find crates/user-cli -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
cargo test --test cli_parses_every_subcommand --no-fail-fast
cargo test --test real_binary --no-fail-fast
# Mutation floor: caught/total >= 75%
```

Expected evidence: parser cases `checked=34,total=34`; property cases `checked=1000,total=1000`; real binary invocations `checked=3,total=3`; zero skipped. Any absent command is a blocker, not a pass.

## 13. Definition of done

All of the following must be true — each is checkable by inspection or command output, no subjective criteria:

- `cargo test -p user-cli --no-fail-fast` exits 0 with `test result: ok` in output.
- `user_cli::tests::parse_rejects_empty_submit` appears in test output and passes.
- `user_cli::tests::parse_rejects_oversized_request` appears in test output and passes.
- `user_cli::tests::render_is_valid_json` appears in test output and passes.
- `cargo test --test real_binary --no-fail-fast` exits 0 (real `fleet --version` and `fleet route --json` invoked via `env!("CARGO_BIN_EXE_fleet")`).
- `cargo clippy -p user-cli --all-targets -- -D warnings` exits 0 (zero warnings).
- No source file under `crates/user-cli/src/` exceeds 80 lines (`find crates/user-cli/src -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0).
- Mutation floor: `caught/total >= 75%` across parse and render targets.

## 14. Failure stories and review questions

**Failure → Cause → Fix:** approval appeared successful but no grant existed → CLI treated text as authority → submit approval intent and require controller receipt.

1. Can a constant parser or empty renderer pass? 2. Is the exact real binary exercised? 3. Are user actions separate from controller authority?
