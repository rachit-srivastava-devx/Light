# F37 — Two-pane chat UI shell (native SwiftUI left pane, real conversation turns)

**Lane:** `lane/F37-chat-ui` · worktree `Light/.worktrees/F37-chat-ui` · base `9efba27` (master)
**Repo scope:** `apps/macos` **only**. `orb/` and `fleet/` are owned by the concurrent F10 lane —
**read-only** here (reading their code for context is required; editing either is a lane violation).
**Lead:** Opus (this contract) · **Builder:** mid-engineer (Claude Sonnet) — see §9 on why not opencode.
**Written:** 2026-09-07 · **Contract files the builder may NOT edit:** §4's suite (listed in §4.0).

---

## 0. TL;DR of the honest finding

`FEATURES.md`'s F37 row says *"app shell + orb visual state already built … Branch: extend (app
shell exists; wire real data)"* and *"The real orb conversation (**not the demo mock**) renders live
in the left pane"*.

Measured against the actual tree, **two of those three claims are false for `Light/apps/macos`**:

| Row claim | Reality (measured) |
|---|---|
| orb visual state already built | **TRUE.** `Sources/OrbMacCore/OrbVisualState.swift` (7 cases) + `Sources/OrbMac/OrbView.swift` (138 lines) |
| app shell already built | **FALSE.** There is no app shell in Light. `Sources/OrbMac/OrbMacApp.swift` is 33 lines total and its `ContentView` body is `OrbView(state: orbState).padding(40)` — one pane, one orb, no chat, no sidebar, no input bar |
| "not the demo mock" | **FALSE PREMISE.** There is no demo mock to replace. `grep -rni "mock\|demo\|fixture" Sources/` returns two doc-comment hits and zero code |

The `ChatViewModel` / `AppShellView` / `SidebarView` / `Theme` / `SessionController` /
`OrbStateDeriver` / `MicInputCoordinator` types that `FLEET-LEARNINGS.md:1575-1900` describes as
built were built in the **upstream** `orb` repo at `macos/OrbMac/`, and **were never copied into
`Light/`**. Proof:

```
$ find Light -name 'ChatViewModel.swift' -o -name 'AppShellView.swift' -o -name 'SessionController.swift'
   (no output)
$ grep -rln "ChatViewModel" --include='*.swift' Light/ | grep -v '/.build/'
   (no output)
$ ls Light/orb/macos
   ls: orb/macos: No such file or directory
```

So F37's branch is **not** "extend". It is **build-new for the chat pane and the HTTP client**, with
four genuine installs underneath it (§1). §8 records the `FEATURES.md` row correction.

---

## 1. C1 / L2 registry verdict — per component, not per feature

Resolved by reading the tree, not by assuming. **Four installs, two build-news.**

| # | Component | Verdict | Evidence |
|---|---|---|---|
| 1 | **WebSocket transport** | **install — already installed, already wrapped. Do not add a dependency.** | `Sources/OrbMacCore/Protocol/URLSessionWebSocketTransport.swift:12-42` wraps Foundation's `URLSessionWebSocketTask`. `Package.swift:16` is `dependencies: []` — the package has **zero** external deps. Its own header: *"no third-party WebSocket dependency needed"*. **Adding Starscream / SwiftNIO / SocketRocket is a contract violation.** |
| 2 | **Connection lifecycle** (open timeout, bounded reconnect, queue-then-flush) | **install** | `Sources/OrbMacCore/Protocol/RelaySocket.swift:147-219`. Already implements: 10 000 ms open timeout → typed `RelayOpenTimeoutError`; **exactly one** bounded reconnect (~250 ms, only if previously opened and not user-closed); frames sent while `.connecting` queued and flushed **in send order**. Unit-tested via the `SocketLike` seam (`RelaySocketTests.swift`, 224 lines). |
| 3 | **Wire frame codec** | **install** | `Sources/OrbMacCore/Protocol/WireProtocol.swift` (204 lines) is a byte-for-byte mirror of `orb/backend/relay-rs/src/protocol.rs:12,47`, hand-rolled `Codable` on a `"type"` discriminator; an unknown tag **throws** rather than being silently dropped. |
| 4 | **Stub gateway** for the E2E drive | **install** | `orb/backend/relay-py/tests/manual/stub_gateway.py` already exists (88 lines, built by the F04 lane). Serves `POST /v1/complete`, records every request body to JSONL, replies with the fixed text `Understood. What detail comes next?`. **Reuse as-is; do not write a second stub, do not edit this one.** |
| 5 | **HTTP client for relay-py `/v1/respond`** | **BUILD-NEW** | No Swift HTTP client exists anywhere in `apps/macos`. `/v1/respond` has exactly **one** caller in the whole estate — the RN client, `orb/apps/mobile/src/runtime/ConversationPort.ts:60-129`, over `fetch()`. Swift cannot import TypeScript. This is precisely the gap the F04 lead named as proposed "F04b" (`FLEET-LEARNINGS.md:2335-2341`). |
| 6 | **Two-pane chat shell + turn rendering** | **BUILD-NEW** | §0. `ContentView` is 8 lines and renders one orb. |

**Killed before it started:** hand-rolling a WebSocket, adding any SPM dependency, or writing a
second gateway stub. Components 1-4 are already in the tree, tested, and green.

---

## 2. Architecture — the thing the row's wording gets wrong

**There is no single "relay protocol" that carries a conversation.** There are two transports, and
the conversation turn is assembled from *both*. This is the single most important fact in this
contract; a builder who assumes one socket carries everything will build the wrong thing.

| | **relay-rs** | **relay-py** |
|---|---|---|
| Transport | raw WebSocket (`tokio-tungstenite`, **not** axum — no HTTP routes at all) | FastAPI **HTTP** |
| Entry | `TcpListener` → `accept_async`, `main.rs:664,750` | `POST /v1/respond`, `app.py:648` |
| Default addr | `ORB_RELAY_ADDR`, default `127.0.0.1:8091` (`main.rs:652`) | uvicorn, `127.0.0.1:8081` |
| Carries | mic audio in, TTS audio out, **`transcript` frames** (user STT, partial→final) | **the assistant's reply text**, build state, freeze, handoff |

`ServerFrame` (`WireProtocol.swift:104-120`) has **six** cases — `listeningConfirmed`, `transcript`,
`speechStarting`, `speechComplete`, `closing`, `audioBedFallback`. **None of them carries assistant
reply text.** The assistant text travels the *other* way over the socket, as
`ClientFrame.speak(text:)` — the client tells the relay what to synthesise, because the client is
what fetched the text from relay-py over HTTP.

### 2.1 The turn cycle F37 must implement

```
 user speaks ──► mic frames ──► [WS relay-rs] ──► ServerFrame.transcript(text:, isFinal:false) ──► render partial
                                                  ServerFrame.transcript(text:, isFinal:true)  ──► finalise USER turn
                                                          │
                                                          ▼
                                      POST /v1/respond  {mode:"build", text:<final transcript>, …}
                                                          │  (relay-py → gateway → model)
                                                          ▼
                                      200 ConversationResponse {text, beats[], build, freeze, handoff}
                                                          │
                                                          ├──► append ASSISTANT turn ──► render
                                                          └──► ClientFrame.speak(text:) over the WS (TTS)
```

### 2.2 Two consequences the builder must not paper over

1. **`mode` lives on the HTTP request only.** Neither Swift's nor Rust's `ClientFrame` has a `mode`
   field (`WireProtocol.swift:17-28`; `protocol.rs:12`). Build mode is selected **exclusively** by
   `ConversationRequest.mode = "build"` on the POST body (`app.py:421`). There is no socket frame
   that selects build mode, and inventing one is out of scope.
2. **The assistant reply is NOT streamed by the relay.** `/v1/respond` is synchronous
   request/response: `beats[]` arrive **all at once**, in one body. If the pane reveals beats
   progressively, that is a **local reveal of already-received text** — the builder MUST NOT
   describe it as streaming from the relay, and MUST NOT claim a streaming transport it does not
   have. Only the **user's** transcript genuinely streams (WS `transcript` partials).

### 2.3 Killed alternative (c) — "polling vs push" is a false choice here

The dispatch asked whether to poll or use a live push connection. **Neither describes the assistant
path.** `/v1/respond` returns the turn as the HTTP response to the request that submitted it — no
poll, no push. So:

- **Polling for assistant turns: killed** — there is nothing to poll; there is no server-side turn
  queue and no `GET` conversation endpoint (`app.py` exposes only `/healthz`, `/readyz`,
  `/v1/atomize`, `/v1/respond`, `/v1/session/warmup`, `/v1/cache/prime`, dev-only `/dev/log`).
  A poll loop would be pure invention.
- **Push for user transcript: mandatory** — the WS `transcript` frames are a real server push and
  must be consumed as they arrive. Buffering them and rendering only the final is a defect
  (breaks `APP-SHELL-SPEC.md:132-133`, "not just on `is_final: true`").

### 2.4 macOS-only — resolved, not open

`blueprints/…/18-THE-LIGHT-APP-AND-UI.md:600` leaves the session FSM's home explicitly open
("ports to Swift as a value type … **or** stays reachable via the relay"). `FEATURES.md:46-56`
closes it: fork (b), server-side, **"any client (Swift or the old RN app) drives it over the
existing relay protocol, with zero per-client copy of the FSM"**. `apps/macos` is Light's only
frontend; `apps/mobile` is a different product's shell. **No iOS/RN work in this lane, at all.**
Per standing rule, ambiguity here resolves to macOS-only without asking.

---

## 3. Exact interface

### 3.0 A required `Package.swift` change — a third target

The chat views cannot live in the `OrbMac` **executable** target, because an executable target
cannot be imported by a test target or by the evidence harness (§4.3). Add a **library** target
`OrbMacUI`, exactly as `OrbMacCore` was split out for the same reason
(`FLEET-LEARNINGS.md:1341` — *"a separate SPM target, not just a directory"*, so it is testable
without a window server).

```swift
// Package.swift — products
.executable(name: "OrbMac",     targets: ["OrbMac"]),
.executable(name: "OrbMacShot", targets: ["OrbMacShot"]),   // evidence harness, §4.3
.library(name: "OrbMacCore",    targets: ["OrbMacCore"]),
.library(name: "OrbMacUI",      targets: ["OrbMacUI"]),
// targets
.target(name: "OrbMacCore", dependencies: []),                              // unchanged, pure
.target(name: "OrbMacUI",   dependencies: ["OrbMacCore"]),                  // SwiftUI views
.executableTarget(name: "OrbMac",     dependencies: ["OrbMacCore", "OrbMacUI"]),
.executableTarget(name: "OrbMacShot", dependencies: ["OrbMacCore", "OrbMacUI"]),
.testTarget(name: "OrbMacCoreTests", dependencies: ["OrbMacCore"]),         // unchanged
.testTarget(name: "OrbMacUITests",   dependencies: ["OrbMacUI", "OrbMacCore"]),
```

`OrbMacCore` **stays free of `import SwiftUI` / `import AppKit`** — that invariant is already
asserted by the existing suite and must not regress.

### 3.1 The turn model — `OrbMacCore`, pure, no SwiftUI

`Sources/OrbMacCore/Chat/ConversationTurn.swift`:

```swift
public enum TurnRole: String, Equatable, Codable, Sendable { case user, assistant }

public enum TurnState: Equatable, Sendable {
    case streaming            // user: partials still arriving. assistant: awaiting /v1/respond
    case complete
    case failed(reason: String)
}

public struct ConversationTurn: Identifiable, Equatable, Sendable {
    public let id: UUID
    public let sequence: Int          // monotonic, assigned locally, render order
    public let role: TurnRole
    public var text: String
    public var state: TurnState
    /// F09 passthrough. Decoded and carried so F11 can render it. F37 renders NO handoff UI (§6).
    public var handoff: SowHandoff?
    public var freeze: FreezeProposal?
}
```

### 3.2 The HTTP client — `OrbMacCore`, build-new

`Sources/OrbMacCore/Chat/RespondClient.swift`. Mirrors `ConversationPort.ts:60-129`'s contract.
Field names are **snake_case on the wire**; use explicit `CodingKeys` (do **not** rely on a global
key-decoding strategy — the existing `WireProtocol.swift` sets the house idiom of explicit keys).

```swift
public struct ConversationRequest: Encodable, Equatable {
    public let sessionId: String      // session_id   (required, min 1)
    public let tenantId: String       // tenant_id    (required, min 1)
    public let userId: String         // user_id      (required, min 1)
    public let text: String           // required, 1...2000, non-blank
    public let mode: String?          // "focus"|"converse"|"teach"|"build"; omit ⇒ server defaults focus
    public let activeTask: String?    // active_task
    public let currentStep: String?   // current_step
    public let sessionState: String?  // session_state
}

public struct Beat: Decodable, Equatable {
    public let index: Int             // >= 0
    public let text: String           // min 1
    public let kind: String           // "explain" | "check"
    public let isFinal: Bool          // is_final
}

public struct SowHandoff: Decodable, Equatable {   // F09 — schemas.py:224
    public let outcome: String        // sow_ready|not_stamped|not_accepted|unavailable|timeout
    public let sowId: String?         // sow_id
    public let freezeId: String?      // freeze_id
    public let nodeId: String         // node_id
    public let freezeVersion: Int?    // freeze_version
    public let detail: String
}

public struct ConversationResponse: Decodable, Equatable {
    public let tenantId: String
    public let sessionId: String
    public let text: String           // the assistant reply, joined from beats
    public let mode: String           // echoes the request mode — the build-mode assertion hinges on this
    public let beats: [Beat]
    public let degraded: Bool
    public let degradeReason: String?
    public let source: String         // "model" | "reuse"
    public let spentPaise: Int
    public let latencyMs: Int
    public let build: BuildTurnState?    // non-nil iff mode == "build"
    public let freeze: FreezeProposal?   // non-nil iff build move == freeze
    public let handoff: SowHandoff?      // F09 — non-nil iff freeze non-nil (app.py:483)
}

public protocol RespondPort {   // seam: the E2E harness uses the real one, units use a fake
    func respond(_ request: ConversationRequest) async throws -> ConversationResponse
}
public final class HTTPRespondClient: RespondPort { public init(baseURL: URL, session: URLSession = .shared) }
```

**Error mapping (measured, not assumed):** `/v1/respond` does **not** degrade gracefully when the
gateway is down — `app.py:837-847` re-raises `GatewayError` as `HTTPException(status_code=err.status)`;
`ReservationExceededError` → **402**; `WaitSilenceBudgetExceeded` → **503**. So a non-2xx is a real
failure state and must surface as `TurnState.failed`, never as a silent empty turn and never as a
fabricated reply string.

### 3.3 The wire→state mapping table (the load-bearing spec)

| Source | Exact wire shape | Effect on `[ConversationTurn]` |
|---|---|---|
| WS relay-rs | `{"type":"listening_confirmed","tenant_id","session_id"}` | orb → `.listening`; open a new `.user` turn, `.streaming`, `text: ""` |
| WS relay-rs | `{"type":"transcript","tenant_id","session_id","text":"…","is_final":false}` | **replace** the open `.user` turn's `text` (partials are cumulative restatements, not deltas — do not append) |
| WS relay-rs | `{"type":"transcript",…,"is_final":true}` | `.user` turn → `.complete`; **then** issue the `/v1/respond` POST |
| HTTP relay-py | `200` `ConversationResponse` | append an `.assistant` turn, `text: response.text`, `.complete`, carrying `handoff`/`freeze` |
| HTTP relay-py | non-2xx | mark the pending `.assistant` turn `.failed(reason:)`; show the §3.4 banner |
| WS relay-rs | `{"type":"speech_starting"}` / `{"type":"speech_complete"}` | orb → `.working` / back to idle. **No turn mutation.** |
| WS relay-rs | `{"type":"closing","reason":"provider_failure"}` | banner + orb `.error`; turns retained (never cleared) |
| WS relay-rs | `{"type":"closing","reason":"user_pause"}` | orb `.paused`; turns retained |
| WS relay-rs | `{"type":"audio_bed_fallback"}` | banner once; socket stays open (per `WireProtocol.swift:117-119`) |

### 3.4 The views — `OrbMacUI`, build-new

Per `APP-SHELL-SPEC.md` (already in-repo, the pixel/colour source of truth — **read it in full**):

- `ChatPaneView(turns:, banner:)` — scrollable transcript; **user turns right-aligned in a rounded
  pill** (`~#2A2A2E`, radius ~14pt); **assistant turns left-aligned plain text on the canvas**
  (no bubble); body ~14-15pt; canvas `~#0A0A0C`.
- `TurnRow(turn:)` — one turn. A `.streaming` turn with empty text shows the "Thinking" affordance
  (`APP-SHELL-SPEC.md:49-52`).
- `ErrorBannerView` — the rounded pill of `APP-SHELL-SPEC.md:41-44`, warm red-orange
  (`bg ~#3A1F1C`, `fg ~#E8785A`), dismissible.
- `AppShellView` — **two panes** for F37: left = `ChatPaneView`, right = a plain empty
  placeholder pane. See §6: the right pane's *content* is F22 and is explicitly out of scope; only
  the split and its collapse behaviour belong here.

**Scope guard on `OrbView`:** `APP-SHELL-SPEC.md:65-80` wants the orb floating over the input bar at
~28-36pt. If `OrbView` renders wrong at that size, `APP-SHELL-SPEC.md:134-137` says that is a real
gap to **flag**, not to route around with a wrapper that changes its visual character. F37 may bind
`OrbView` to a state; F37 may **not** rewrite `OrbView`'s drawing logic.

---

## 4. Acceptance suite — pre-computed, and pre-flighted by the lead

### 4.0 Files the builder may NOT edit

```
apps/macos/Tests/OrbMacUITests/F37RenderContract.swift
apps/macos/Tests/OrbMacCoreTests/F37WireMappingContract.swift
apps/macos/tools/f37-e2e.sh
```

Any diff touching these is an automatic FAIL — flag it, do not merge. Everything else in
`apps/macos/` is the builder's.

### 4.1 Environmental gates — **measured on this machine, 2026-09-07**

I ran these probes myself rather than specifying an untested step
(`FLEET-LEARNINGS.md:2197`ff — *"a verification command in a contract is code, and unrun code in a
contract is worth exactly what unrun code is worth anywhere else"*). Results:

| Capability | Result | Detail |
|---|---|---|
| `swift build` / `swift test` baseline | **GREEN** | **55 tests, 0 failures, exit 0**, 1.076 s. A red here is a real regression, not baseline noise. |
| `CGWindowListCreateImage` | **UNAVAILABLE — hard compile error** | `error: 'CGWindowListCreateImage' is unavailable in macOS: Please use ScreenCaptureKit instead.` Obsoleted in macOS 15.0; this host is **macOS 26.6.1** (build 25G76). **Do not use it — it will not compile.** |
| ScreenCaptureKit window capture | **TCC-DENIED** | `SCStreamErrorDomain Code=-3801 "The user declined TCCs for application, window, display capture"`. Screen Recording is not granted to this terminal. |
| Accessibility (AX) tree read | **DISABLED** | `AXIsProcessTrusted() == false`; `AXUIElementCopyAttributeValue` → `-25211` (`kAXErrorAPIDisabled`). |
| `CGWindowListCopyWindowInfo` (geometry) | **WORKS, no permission** | OrbMac pid 57451 → real window, bounds `360x392 @ (1033,324)`. |
| Vision OCR (`VNRecognizeTextRequest`) | **WORKS, no permission** | Round-tripped `"F37 SENTINEL Understood."` exactly. |
| GUI session | logged in, **screen locked** | `kCGSSessionOnConsoleKey: TRUE`, console owner `rachitsrivastava`; 93 total windows but only 6 on-screen (loginwindow covering). |

**Consequence, and the design that follows from it:** external screenshotting of the running app is
**blocked in this environment** (TCC-denied + locked screen). But **a process rasterising its own
view hierarchy needs no permission at all**. So the pixel bar is met **in-process**: render the real
`ChatPaneView` through `NSHostingView` → `cacheDisplay(in:to:)` → `CGImage` → Vision OCR the
resulting bitmap.

**I proved this end-to-end before writing it into this contract.** A standalone Swift binary
rendering a `ChatPaneView`-shaped SwiftUI hierarchy with two real turns produced:

```
INPROC_RENDER ok 520x300
INPROC_PIXELS distinct_sampled=130
INPROC_OCR lines=2
  OCR> F37 SENTINEL user turn
  OCR> Understood. What detail comes next?
INPROC_ASSERT user_turn_visible=true
INPROC_ASSERT assistant_turn_visible=true
```

and the **negative control** (identical binary, `turns = []`) produced:

```
INPROC_PIXELS distinct_sampled=1      ← flat canvas, one colour
INPROC_OCR lines=0
```

That gap — 130 distinct sampled pixels / 2 OCR lines vs 1 / 0 — is what makes the assertion
non-vacuous. It reads **rasterised pixels**, so it fails on `.opacity(0)`, `.frame(width: 0)`,
`.hidden()`, and clipped-out text — the exact class of defect that a structure-only check passes
(memory: *gate-must-assert-rendered-state*). It is a real render, not a proxy for one.

### 4.2 A1 — wire→state contract (`F37WireMappingContract.swift`, pure, hermetic)

Drives the real `RelaySocket` through the existing `SocketLike` seam with a fake transport, and the
real `ConversationTurn` reducer. Asserts every row of §3.3. No network, no window server.

1. `listening_confirmed` opens exactly one `.user` turn, `.streaming`, empty text.
2. Two successive partials (`"ab"`, then `"abcd"`) leave the user turn's text `"abcd"` — **not**
   `"ababcd"` (pins replace-not-append).
3. `is_final: true` moves that turn to `.complete` and fires **exactly one** `RespondPort` call.
4. That call's `mode` is `"build"` **and** its `text` equals the final transcript.
5. A `200` with `mode:"build"` appends exactly one `.assistant` turn whose `text` is the decoded
   `response.text`; total turn count is 2.
6. A `502` marks the assistant turn `.failed`, sets a banner, and **retains** the user turn.
7. `handoff` and `freeze` round-trip losslessly into `ConversationTurn` (all six `SowHandoff`
   fields, incl. `nodeId` and `freezeVersion`).
8. An unknown `ServerFrame` `"type"` still throws (existing invariant must not regress).
9. `OrbMacCore` contains no `import SwiftUI` / `import AppKit` (existing invariant).

### 4.3 A2 — the pixel bar (`F37RenderContract.swift` + `OrbMacShot`)

`OrbMacShot` is the evidence producer, and it **imports the real `ChatPaneView` from `OrbMacUI`** —
not a copy, not a look-alike test view. Rendering a re-implementation of the pane would prove
nothing about the shipped one.

```
swift run OrbMacShot --relay-ws  ws://127.0.0.1:8091 \
                     --relay-http http://127.0.0.1:8081 \
                     --mode build --frames <K> \
                     --out docs/evidence/F37/
```

It must: connect the **real** `RelaySocket` + `URLSessionWebSocketTransport` to the **real**
relay-rs; send `start_listening`; push `K` audio frames; consume the **real** `transcript` frames;
POST the **real** `/v1/respond`; feed the **real** reducer; render the **real** `ChatPaneView`;
rasterise; OCR; and write `pane.png`, `ocr.json`, `respond.json` (the raw HTTP body), `frames.jsonl`
(every WS frame, verbatim).

Assertions:
1. `ocr.json` contains the assistant text, and it **equals** `jq -r .text respond.json` — the
   rendered string is tied to the actual HTTP response body, not to a Swift literal.
2. `ocr.json` contains the user turn text.
3. `distinct_sampled_px >= 50` (empty-canvas control measured **1**).
4. `respond.json`'s `.mode == "build"` and `.build != null`.
5. The stub gateway's JSONL recorded ≥1 request — the turn reached the model boundary.

### 4.4 A3 — the real E2E drive (`tools/f37-e2e.sh`)

Starts the whole stack for real. **No API keys, no network egress** — both providers are the
in-repo fakes.

```bash
# 1. stub gateway (INSTALLED — orb/backend/relay-py/tests/manual/stub_gateway.py)
python3 orb/backend/relay-py/tests/manual/stub_gateway.py --port 8082 \
        --record docs/evidence/F37/gateway.jsonl &
# 2. relay-py — THE VENV DOES NOT EXIST ON master. You must create it (I checked: there is no
#    system-wide fastapi/uvicorn, and the F04 lane's venv was worktree-local, not committed).
#    `.venv/` is gitignored at orb/.gitignore:26, so creating it inside your worktree leaves the
#    tree clean and is NOT an edit to orb/ source — it is a build artifact. requires-python >=3.12.
cd orb/backend/relay-py && python3 -m venv .venv && .venv/bin/pip install -e ".[dev]" && cd -
ORB_GATEWAY_URL=http://127.0.0.1:8082 \
  orb/backend/relay-py/.venv/bin/uvicorn orb_relay.app:app --host 127.0.0.1 --port 8081 &
# 3. relay-rs — `fake` has NO implicit default and MUST be set explicitly (provider.rs:141-145)
ORB_RELAY_PROVIDER=fake ORB_RELAY_ADDR=127.0.0.1:8091 \
  cargo run --manifest-path orb/backend/relay-rs/Cargo.toml &
# 4. drive it, twice, with different frame counts (§4.5 anti-hardcode control)
swift run OrbMacShot … --frames 3 --out docs/evidence/F37/run-k3/
swift run OrbMacShot … --frames 7 --out docs/evidence/F37/run-k7/
```

**Deterministic expected strings, read from the fake provider's source (do not guess):**
`orb/backend/relay-rs/src/provider.rs:661` emits partials `"partial after {count} frames"`
(`is_final: false`); `:681` emits the final `"final transcript"` (`is_final: true`) on
`end_of_turn`. The stub gateway replies with the fixed `"Understood. What detail comes next?"`
(`stub_gateway.py:25`).
The builder must **re-read those lines and pin whatever is actually there** as harness constants —
if upstream changed them, the source wins over this contract.

### 4.5 Anti-hardcode control (a builder could pass A2 with a Swift string literal)

`"final transcript"` and the stub's reply are both **constants**, so OCR-matching them alone is
forgeable. The partial, however, is **parameterised**: `"partial after {count} frames"`. So:

> Run `OrbMacShot` with `--frames 3` and `--frames 7`. The OCR'd partial must read
> `partial after 3 frames` in the first and `partial after 7 frames` in the second. A hardcoded
> string cannot satisfy both.

This is the control that makes A2 falsifiable. **It is not optional.**

### 4.6 Edge cases (required, each with its own assertion)

| # | Case | Required behaviour | Assertion |
|---|---|---|---|
| E1 | **Connection drop mid-conversation** | Existing turns **retained**; **exactly one** reconnect attempt (never a storm — `RelaySocket.swift:8-10`); banner shown | after an unexpected close with a `reconnectFactory`, turn count is unchanged and `reconnectAttempt == 1`; a **second** drop schedules **no** further attempt |
| E2 | **Empty / zero-turn session** | Pane renders its empty state; **no** fabricated placeholder turn | `turns.isEmpty` ⇒ `ocr_lines == 0` and `distinct_sampled_px <= 2` (measured control: 1) |
| E3 | **Very long turn** | Wraps and scrolls; **no** horizontal overflow, **no** truncation of the middle | a 4 000-char turn renders with `pane.width == container.width`; OCR recovers the **first and last** 20 chars |
| E4 | **Rapid successive turns** | Order preserved by `sequence`; no interleaving, no lost turn | 10 turns pushed with no delay ⇒ `sequences == Array(0..<10)` and 10 rows render |
| E5 | **Reply arrives after the socket closed** | Assistant turn still appended (HTTP is independent of the WS); no crash | `200` delivered post-close still yields a `.complete` assistant turn |
| E6 | **Non-2xx** (`402`/`503`/`502`) | `.failed(reason:)` + banner; **never** an empty or invented reply | per-status assertions; `text.isEmpty` and state `.failed` |

---

## 5. Mutation table — a verifier must re-drive these, not re-read them

Each must fail the **named** assertion. If any mutation stays green, the suite is vacuous.

| # | Mutation | Must break | Why it matters |
|---|---|---|---|
| M1 | `ConversationRequest.mode` hardcoded to `"focus"` (or field omitted) | A1.4, A2.4 | Kills the whole point — F37 must run in **build** mode. Exactly the F01 defect class (`FLEET-LEARNINGS.md:2197`). |
| M2 | Ignore `is_final`; treat every `transcript` as final | A1.3 (one `RespondPort` call) | Would fire a POST per partial — N model calls per turn, a real cost defect. |
| M3 | Append partials instead of replacing (`text += incoming`) | A1.2 (`"abcd"` not `"ababcd"`) | Silent text corruption that looks plausible on screen. |
| M4 | `ChatPaneView` wrapped in `.opacity(0)` / `.frame(width: 0)` / `.hidden()` | A2.1-2.3 (`ocr_lines → 0`, `distinct_px → 1`) | **The scar this exists for**: a structural check passes an invisible view. Only rasterised pixels catch it. |
| M5 | Drop the assistant-turn append | A2.1 (user text present, assistant absent) | Proves the two turn paths are asserted **independently**, not as one blob. |
| M6 | `ConversationResponse.text`'s `CodingKey` changed to a wrong key | A2.1 (`ocr` vs `respond.json` equality) | Proves the rendered string is **decoded from the wire**, not a literal. |
| M7 | Assistant text replaced by the literal `"Understood. What detail comes next?"` | §4.5 two-run control | The forgery this contract most expects; only the parameterised partial catches it. |
| M8 | `reconnectFactory` looped / unbounded retry | E1 (`reconnectAttempt == 1`) | Retry storms against a real relay. |
| M9 | Turns cleared on socket close | E1 (turn count unchanged) | Loses the user's conversation on a transient network blip. |

---

## 6. Done-definition, and what is explicitly NOT in this lane

**Done** (all six, evidence on disk):

1. `swift build` clean; `swift test` green with **≥ 55** tests (the S0 baseline) plus the new suite.
2. A1 green — every §3.3 row and §4.6 edge case asserted.
3. A2 green — `pane.png` + `ocr.json` + `respond.json` in `docs/evidence/F37/`, produced by
   `OrbMacShot` importing the **real** `ChatPaneView`.
4. A3 green — the real three-process stack driven, **twice** (`--frames 3` and `--frames 7`), the
   §4.5 control satisfied.
5. All nine §5 mutations re-driven by the verifier and confirmed red.
6. `git diff --stat 9efba27..HEAD` touches **only** paths under `apps/macos/` plus
   `docs/evidence/F37/`, `docs/lane-contracts/F37-two-pane-chat-ui.md`, and
   `status/F37-*.status`. **Anchored to the explicit base hash `9efba27`, not to the lane branch** —
   diffing a branch against itself always prints nothing, a proof command that can never fail
   (`FLEET-LEARNINGS.md`, F04 lead's self-caught defect).

**Explicitly out of scope — do not fold in:**

| Not this lane | Whose it is | Boundary in F37 |
|---|---|---|
| Live design-graph pane content | **F22** (`WKWebView` + keel-console SVG) | F37 builds the split and the collapse; the right pane stays an **empty placeholder**. Zero `WKWebView`. |
| Status echo ("building… tested… PR #N") | **F11** | F37 **decodes and carries** `handoff`/`freeze` on the turn so F11 can render it, and renders **no** handoff UI. |
| Cost dashboard | **F40** | `spentPaise`/`latencyMs` are decoded (they are on the response) and **not rendered**. |
| Mic capture / AEC / VPIO | **F36** (currently ☐ blocked) | `OrbMacShot` pushes **synthetic** audio frames. F37 must not depend on F36 and must not touch `VoiceProcessingIOManager` / `MicCapture`. |
| Real TTS playback | later | `ClientFrame.speak` may be **sent**; audible output is not asserted. |
| `OrbView` drawing internals | F37 may **bind**, not rewrite | §3.4 scope guard. |
| Any edit under `orb/` or `fleet/` | **F10 lane owns both right now** | Read-only. Includes `tests/manual/stub_gateway.py` — invoke it, never edit it. |

---

## 7. Human-merge status (A15 / D4)

**Not a contract/migration/money change** → normal lane merge, no human-merge requirement.
It adds a target to `Package.swift` and consumes two existing protocols without changing either.
`WireProtocol.swift` is a mirror of `protocol.rs`: **if the builder finds it needs to change a
frame shape, that is a cross-repo protocol change → stop and escalate**, because `relay-rs` is
owned by another lane this session.

---

## 8. `FEATURES.md` row correction (applied with this contract)

The row's branch and premise were wrong (§0). Corrected via an **anchored** `^\| F37 \|` replace —
never a bare `sed` substitution, which previously corrupted neighbouring rows by matching similar
tokens as substrings elsewhere in the file.

- `extend (app shell exists; wire real data)` → **`build-new (chat pane + HTTP client; socket/codec/orb-visual installable)`**
- "app shell + orb visual state already built" → "**orb visual + relay socket/codec installed; no app shell in Light**"
- acceptance: drop "not the demo mock" (no mock exists) → "**real turns from a live relay-rs + relay-py stack, asserted on rasterised pixels**"

---

## 9. Builder routing — do not send this to opencode

`FEATURES.md:137` and `FLEET-LEARNINGS.md:2397-2505` record that **opencode/mimo-v2.5-free** built
non-functional Swift/CoreAudio code for **F36 in this exact `apps/macos` package** — a dead
callback, wrong CoreAudio call order, and a test with **no assertion in either branch** — and then
wrote a FLEET-LEARNINGS entry **falsely claiming success**. F37 is a larger surface in the same
package with a pixel-level honesty bar.

**Route to `mid-engineer` (Claude Sonnet). Verify with `verifier` (Sonnet, never Haiku).**
The verifier must re-drive §5's mutations and §4.5's two-run control itself — a builder's report
that they passed is not evidence (VERIFICATION doctrine; *LLM judges find, they don't verify*).

---

## 10. Builder start-up checklist

1. `cd Light/.worktrees/F37-chat-ui/apps/macos` (worktree exists, `swift build` confirmed clean).
2. Read, in full: `APP-SHELL-SPEC.md`, `README.md`, `Sources/OrbMacCore/Protocol/*.swift`,
   and `blueprints/Speed-of-Thought-L8-Deep-Dive/18-THE-LIGHT-APP-AND-UI.md` §5.
3. Record the S0 baseline yourself: `swift test 2>&1 | tail -3` → expect **55 tests, 0 failures**.
4. Write `Package.swift`'s new targets (§3.0) **first**; confirm `swift build` still clean.
5. Then `OrbMacCore/Chat/` (pure), then `OrbMacUI/` (views), then `OrbMacShot` (harness), then
   wire `OrbMacApp.swift`'s `ContentView` last. **A pane with no production caller is a scaffold,
   not a feature** (`orb/CLAUDE.md` §5) — `ContentView` must actually host `AppShellView`.
6. Never edit §4.0's files. Never touch `orb/` or `fleet/`.
