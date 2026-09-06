# ADR 0012 — `pysbd` (new runtime dependency) for teach-mode beat splitting

**Status:** accepted, ADOPT-WITH-CAVEAT. **Date:** 2026-08-28. **Independent package
due-diligence (2026-08-28, same session):** confirmed pysbd is the most accurate sentence-boundary
option available and is not archived, but its last release is ~5 years old, failing the ~18-month
freshness bar on its own. Verdict: keep it (see Context/Decision for why), but pin the exact
version (done below) and record the staleness honestly (this section) rather than an ADR that
lists only the upside.

## Context

Track C (`backend/relay-py`) needs to chunk a teach-mode LLM reply into speakable "beats" of at most
two sentences each (acceptance contract C4 — voice-shaped output, not a monologue; the client plays
beats sequentially and can barge in between them). A hand-rolled sentence splitter (regex on
`.`/`!`/`?`) breaks on exactly the inputs a teach explanation is full of: "Dr.", "e.g.", "U.S.",
decimals like "3.14", ellipses, and quoted speech — each would wrongly end a beat mid-thought. This
matches a standing owner directive (mid-flight, this session): prefer a trustworthy existing package
over hand-rolling something with known edge cases, and vet it (license, freshness, adoption, CVEs)
rather than adopting on faith.

## Decision

Add **`pysbd`** (Pragmatic Sentence Boundary Disambiguation) 0.3.4 as a runtime dependency of
`orb-relay` (`backend/relay-py/pyproject.toml`), used in `proxy/teach.py` to split a teach reply into
sentences before grouping them into <=2-sentence beats.

**Vetting evidence (checked live against PyPI/GitHub this session, not from memory):**

- License: MIT (PyPI classifier + `pip show`) — permissive, compatible.
- Runtime dependencies: **zero** (`pip show` `Requires:` is empty) — no transitive supply-chain
  surface, and it is a pure-Python rule-based tool (no `eval`, no network calls, no deserialization
  of untrusted structures beyond the input string) — negligible CVE surface.
- Adoption: 931 GitHub stars, not archived, repository pushed as recently as 2024-08-20.
- **Freshness gap, disclosed:** the last PyPI release is `0.3.4` from **2021-02-11** — well past the
  ~18-month freshness bar the owner directive asks every dependency to clear. The two lightweight
  alternatives the directive also named (`syntok`, last release 2022-03; `blingfire`, last release
  2021-09, no declared PyPI license) are equally or more stale, which is evidence this is a solved,
  stable problem for a narrow rule-based tool rather than an abandoned one — reinforced by GitHub
  push activity three years after the last PyPI release. The fresher named alternative, spaCy
  (actively released within days as of this ADR), was rejected for this use: its statistical
  `senter` needs a downloaded trained pipeline plus numpy/cython/thinc/blis as transitive
  dependencies — disproportionate weight for one narrow chunking call in a product whose own
  AGENTS.md rung explicitly asks for a thin-cloud, 20K-user footprint with explicit, minimal
  dependencies, not a 50M-user platform's NLP stack.
- **Correctness, proven not assumed:** live-tested against exactly the edge cases named in the
  directive ("Dr.", "e.g.", "U.S.", "3.14", ellipses, quoted speech) before adoption — `pysbd`
  correctly keeps each of those mid-sentence and only splits at real sentence boundaries. A
  hand-rolled `.`/`!`/`?` regex splitter (the original approach before this ADR) gets every one of
  those cases wrong.

## Consequences

- One new runtime dependency, pinned **exact** as `pysbd==0.3.4` (not `>=`, not a caret range) in
  `pyproject.toml`'s `[project.dependencies]` (same section as `fastapi`/`pydantic`/`httpx`),
  installed into `backend/relay-py/.venv`. Exact-pinned deliberately: a floating range on a package
  that is not actively releasing is how a silent break arrives later with no changelog to explain
  it. Bump only by deliberate, tested action, never automatically.
- Zero transitive dependencies means this does not meaningfully change the relay's deploy size or
  attack surface.
- **Staleness, disclosed plainly:** last PyPI release `0.3.4` was **2021-02-11** (~5.5 years before
  this ADR). Accepted anyway because (a) sentence-boundary rules for a rule-based tool are a stable,
  largely-solved problem — the repo itself was pushed to as recently as 2024-08-20, evidence of
  "feature-complete," not "abandoned" — and (b) it is live-verified (not assumed) to correctly
  handle exactly the edge-case class that matters here and that a hand-rolled regex splitter fails:
  abbreviations ("Dr.", "e.g.", "U.S."), decimals ("3.14"), ellipses, and quoted speech.
  **Independent due-diligence (2026-08-28) also checked and ruled out both alternatives this ADR
  originally floated as a fallback:** `syntok` is dead (~4.5 years stale, worse than pysbd) and
  `blingfire` has been quiet ~20 months with no declared PyPI license — neither is a safer
  landing spot than staying on pysbd today. **No maintained, comparably-accurate lightweight
  alternative currently exists** for this narrow need; spaCy remains rejected for the
  disproportionate-dependency reason above.
- **Named revisit triggers** (review this ADR, don't just keep re-approving it silently): pysbd
  breaks or mis-segments under a future Python interpreter upgrade; a real defect or security issue
  surfaces with no upstream fix forthcoming; PyPI marks the project archived; or a maintained
  alternative reaches comparable accuracy and clears the freshness bar (re-run the same
  live-edge-case test this ADR ran before switching, not a license/date check alone).
- This does not change beat *content* — `pysbd` only decides sentence boundaries; grouping sentences
  into <=2-sentence beats and classifying a beat's kind (explain vs. checks-understanding) remain
  this product's own deterministic logic (`proxy/teach.py`), consistent with the rest of this
  codebase's "LLM/tool fills language, code owns structure" boxing pattern.
