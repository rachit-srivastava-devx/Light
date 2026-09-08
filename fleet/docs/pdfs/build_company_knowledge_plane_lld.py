from reportlab.pdfgen import canvas
from reportlab.lib.pagesizes import A0, landscape
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfbase.pdfmetrics import stringWidth
from reportlab.lib.colors import HexColor
import math
import os


ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
OUT = os.path.join(ROOT, "output/pdf/devx-ai-native-sdlc-lld.pdf")
W, H = landscape(A0)

M = 62
INK = HexColor("#0B0D0F")
INK2 = HexColor("#22262B")
MUTED = HexColor("#59616A")
MUTED2 = HexColor("#89919A")
RULE = HexColor("#D9DEE4")
PAPER = HexColor("#FFFFFF")
PAPER2 = HexColor("#F8F9FA")
BLUE = HexColor("#1769FF")
BLUE2 = HexColor("#EAF1FF")
CYAN = HexColor("#008D9A")
CYAN2 = HexColor("#E5F7F8")
GREEN = HexColor("#087A55")
GREEN2 = HexColor("#E6F4EE")
ORANGE = HexColor("#C65B08")
ORANGE2 = HexColor("#FFF0E3")
RED = HexColor("#B52D2A")
RED2 = HexColor("#FCE9E8")
VIOLET = HexColor("#6750C8")
VIOLET2 = HexColor("#F0ECFF")

FONT_ROOT = "/Users/rachitsrivastava/.agents/skills/canvas-design/canvas-fonts"
pdfmetrics.registerFont(TTFont("Display", os.path.join(FONT_ROOT, "InstrumentSans-Regular.ttf")))
pdfmetrics.registerFont(TTFont("Display-Bold", os.path.join(FONT_ROOT, "InstrumentSans-Bold.ttf")))
pdfmetrics.registerFont(TTFont("Editorial-Italic", os.path.join(FONT_ROOT, "IBMPlexSerif-Italic.ttf")))
pdfmetrics.registerFont(TTFont("Mono", os.path.join(FONT_ROOT, "GeistMono-Regular.ttf")))
pdfmetrics.registerFont(TTFont("Mono-Bold", os.path.join(FONT_ROOT, "GeistMono-Bold.ttf")))


def txt(c, x, y, value, font="Display", size=10, color=INK, align="left"):
    c.setFillColor(color)
    c.setFont(font, size)
    if align == "right":
        c.drawRightString(x, y, value)
    elif align == "center":
        c.drawCentredString(x, y, value)
    else:
        c.drawString(x, y, value)


def wrap(value, font, size, width):
    out = []
    for raw in value.split("\n"):
        if raw == "":
            out.append("")
            continue
        words = raw.split()
        current = ""
        for word in words:
            trial = word if not current else current + " " + word
            if stringWidth(trial, font, size) <= width:
                current = trial
            else:
                if current:
                    out.append(current)
                current = word
        if current:
            out.append(current)
    return out


def para(c, x, y, value, width, font="Display", size=8, leading=10, color=MUTED, max_lines=None):
    lines = wrap(value, font, size, width)
    if max_lines is not None:
        lines = lines[:max_lines]
    for i, line_value in enumerate(lines):
        txt(c, x, y - i * leading, line_value, font, size, color)
    return y - len(lines) * leading


def rect(c, x, y, w, h, fill=PAPER, stroke=RULE, sw=0.8, radius=0):
    c.setFillColor(fill)
    c.setStrokeColor(stroke)
    c.setLineWidth(sw)
    if radius:
        c.roundRect(x, y, w, h, radius, fill=1, stroke=1)
    else:
        c.rect(x, y, w, h, fill=1, stroke=1)


def line(c, x1, y1, x2, y2, color=RULE, sw=0.8, dash=None):
    c.setStrokeColor(color)
    c.setLineWidth(sw)
    c.setDash(dash or [])
    c.line(x1, y1, x2, y2)
    c.setDash([])


def arrow(c, points, color=BLUE, sw=1.7, head=7, dash=None):
    c.setStrokeColor(color)
    c.setFillColor(color)
    c.setLineWidth(sw)
    c.setDash(dash or [])
    p = c.beginPath()
    p.moveTo(*points[0])
    for point in points[1:]:
        p.lineTo(*point)
    c.drawPath(p, fill=0, stroke=1)
    c.setDash([])
    x1, y1 = points[-2]
    x2, y2 = points[-1]
    angle = math.atan2(y2 - y1, x2 - x1)
    a = (x2 - head * math.cos(angle - 0.5), y2 - head * math.sin(angle - 0.5))
    b = (x2 - head * math.cos(angle + 0.5), y2 - head * math.sin(angle + 0.5))
    tri = c.beginPath()
    tri.moveTo(x2, y2)
    tri.lineTo(*a)
    tri.lineTo(*b)
    tri.close()
    c.drawPath(tri, fill=1, stroke=0)


def section(c, y, h, number, title, note):
    txt(c, M, y + h - 16, number, "Mono-Bold", 9, BLUE)
    txt(c, M + 34, y + h - 16, title.upper(), "Mono-Bold", 9, INK)
    txt(c, M + 285, y + h - 16, note, "Mono", 7.5, MUTED)
    line(c, M, y + h - 29, W - M, y + h - 29, INK, 0.9)


def card(c, x, y, w, h, title, subtitle, body, color=BLUE, fill=PAPER, footer=None):
    rect(c, x, y, w, h, fill, RULE, 0.8, 5)
    c.setFillColor(color)
    c.rect(x, y + h - 30, 5, 30, fill=1, stroke=0)
    txt(c, x + 14, y + h - 20, title.upper(), "Display-Bold", 9.3, INK)
    if subtitle:
        txt(c, x + w - 12, y + h - 20, subtitle, "Mono", 6.7, color, "right")
    line(c, x + 12, y + h - 34, x + w - 12, y + h - 34, RULE, 0.6)
    para(c, x + 13, y + h - 49, body, w - 26, "Display", 7.4, 9.2, MUTED)
    if footer:
        line(c, x + 12, y + 27, x + w - 12, y + 27, RULE, 0.5)
        para(c, x + 13, y + 17, footer, w - 26, "Mono-Bold", 6.5, 7.5, color, 2)


def numbered(c, x, y, w, items, color=BLUE, size=7.1, leading=9.1, step=31):
    yy = y
    for i, item in enumerate(items, 1):
        txt(c, x, yy, f"{i:02d}", "Mono-Bold", 6.7, color)
        para(c, x + 27, yy, item, w - 27, "Display", size, leading, MUTED, 3)
        yy -= step


def fleet_step(c, x, y, w, h, code, title, command, detail, status="LIVE"):
    palette = {
        "LIVE": (GREEN, GREEN2),
        "PARTIAL": (ORANGE, ORANGE2),
        "EXTERNAL": (RED, RED2),
    }
    color, fill = palette[status]
    rect(c, x, y, w, h, fill, color, 0.8, 4)
    txt(c, x + 9, y + h - 16, code, "Mono-Bold", 6.2, color)
    txt(c, x + w - 9, y + h - 16, status, "Mono-Bold", 5.6, color, "right")
    txt(c, x + 9, y + h - 32, title.upper(), "Display-Bold", 7.5, INK)
    para(c, x + 9, y + h - 45, detail, w - 18, "Display", 6.25, 7.5, MUTED, 4)
    line(c, x + 8, y + 18, x + w - 8, y + 18, color, 0.4)
    para(c, x + 9, y + 10, command, w - 18, "Mono-Bold", 5.5, 6.2, color, 2)


def build():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    c = canvas.Canvas(OUT, pagesize=(W, H), pageCompression=1)
    c.setTitle("DevX Labs - Company Knowledge Plane for an AI-Native SDLC")
    c.setAuthor("DevX Labs")
    c.setSubject("Implementation-level graph, retrieval, synchronization, agent, and evidence architecture")
    c.setFillColor(PAPER)
    c.rect(0, 0, W, H, fill=1, stroke=0)

    txt(c, M, H - 42, "DEVX LABS / CTO IMPLEMENTATION REFERENCE", "Mono-Bold", 9, BLUE)
    txt(c, W - M, H - 42, "LLD-03 / 25 AUG 2026 / A0 / SEARCHABLE", "Mono", 8.3, MUTED, "right")
    line(c, M, H - 58, W - M, H - 58, INK, 1)
    txt(c, M, H - 101, "The company knowledge plane", "Display-Bold", 29, INK)
    txt(c, M + 440, H - 101, "every SDLC agent actually uses.", "Editorial-Italic", 29, BLUE)
    para(c, M, H - 128, "A deployable low-level design for multi-repository impact analysis, scoped agent context, evidence-bound delivery, and company-wide learning. Fleet supplies the local enforcement kernel; the enterprise plane below adds durable events, cross-repo topology, temporal truth, semantic retrieval, tenancy, and operations.", W - 2 * M, "Display", 10, 13, MUTED, 2)

    legend_y = H - 158
    legends = [(BLUE, "EVENT / PRIMARY FLOW"), (CYAN, "KNOWLEDGE / RETRIEVAL"), (VIOLET, "AGENT"), (RED, "REFUSAL / HUMAN"), (GREEN, "EVIDENCE")]
    lx = M
    for color, label in legends:
        c.setFillColor(color)
        c.circle(lx + 4, legend_y + 2, 4, fill=1, stroke=0)
        txt(c, lx + 14, legend_y, label, "Mono-Bold", 6.6, MUTED)
        lx += stringWidth(label, "Mono-Bold", 6.6) + 45
    txt(c, W - M, legend_y, "RULE: NO AGENT GETS DATABASE OR MERGE CREDENTIALS", "Mono-Bold", 7, RED, "right")

    left = M
    rail_w = 300
    gap = 18
    main_x = left + rail_w + gap
    main_w = W - 2 * M - 2 * rail_w - 2 * gap
    rail_r = main_x + main_w + gap

    s1_y, s1_h = 1775, 390
    section(c, s1_y, s1_h, "01", "CHANGE SIGNALS + CONTINUOUS INGESTION", "webhooks are hints; reconciliation establishes truth")
    y = s1_y + 18
    h = 314
    source_w = 205
    source_gap = 12
    sources = [
        ("WORK", "Jira / Linear / GitHub", "ticket.version; state; AC/NFR; owner; links; risk labels", BLUE, BLUE2),
        ("CODE", "GitHub App + Backstage", "push/PR/install webhooks; catalog-info.yaml; CODEOWNERS; repo ACL", BLUE, BLUE2),
        ("STATIC", "Sourcegraph SCIP", "symbols, definitions, references; precise indexers. Fleet tree-sitter fallback", CYAN, CYAN2),
        ("INTERFACE", "OpenAPI / AsyncAPI / Pact", "routes, events, schemas, consumer-provider matrix, compatibility", CYAN, CYAN2),
        ("SUPPLY", "lockfiles / SBOM / SLSA", "package dependencies, image digest, provenance, vulnerability identity", ORANGE, ORANGE2),
        ("DATA + INFRA", "OpenLineage + Terraform", "dataset-job lineage; state/plan resources; K8s workloads, quotas, owners", ORANGE, ORANGE2),
        ("RUNTIME", "OTel graph / App Signals", "observed calls, latency/error/SLO, deploy markers, incidents", GREEN, GREEN2),
    ]
    src_total = len(sources) * source_w + (len(sources) - 1) * source_gap
    src_x = main_x + (main_w - src_total) / 2
    for i, (code, title, body, color, fill) in enumerate(sources):
        x = src_x + i * (source_w + source_gap)
        card(c, x, y + 128, source_w, 160, title, code, body, color, fill, "EMITS VERSIONED SOURCE OBSERVATIONS")
        arrow(c, [(x + source_w / 2, y + 128), (x + source_w / 2, y + 108)], color, 1.1, 5)

    pipe_y = y + 10
    pipe_h = 94
    stages = [
        ("WEBHOOK GATEWAY", "GitHub App HMAC; dedupe delivery_id; 1h installation token; enqueue < 2 s"),
        ("KAFKA + SCHEMA REGISTRY", "CloudEvents/Avro; key=tenant|repo; at-least-once; DLQ; retention 7 d"),
        ("TEMPORAL WORKFLOWS", "crawl -> clone -> parse -> enrich -> project; durable retry; per-repo concurrency=1"),
        ("DEBEZIUM OUTBOX", "DB mutation + outbox in one transaction; event id dedupe; ordered by aggregate_id"),
        ("RECONCILER", "nightly full scan; webhook gap repair; tombstone deleted entities; publish coverage"),
    ]
    stage_gap = 15
    stage_w = (main_w - (len(stages) - 1) * stage_gap) / len(stages)
    for i, (title, body) in enumerate(stages):
        x = main_x + i * (stage_w + stage_gap)
        card(c, x, pipe_y, stage_w, pipe_h, title, f"I{i+1}", body, BLUE if i < 3 else GREEN, PAPER2)
        if i < len(stages) - 1:
            arrow(c, [(x + stage_w + 2, pipe_y + pipe_h / 2), (x + stage_w + stage_gap - 2, pipe_y + pipe_h / 2)], BLUE, 1.2, 5)

    card(c, left, 1960, rail_w, 165, "Fleet: code inspected", "ACTUAL KERNEL", "Rust: clap 4; rusqlite 0.32; tree-sitter 0.24 for Rust/Python/Bash; rmcp 3.1.4; serde/schemars; blake3; fs2; tokio. Python crew: pydantic 2; numpy 2; statsmodels; Codex and Claude CLI adapters.", BLUE, PAPER2, "LOCAL, FILE-BACKED, SINGLE-REPOSITORY")
    card(c, left, 1780, rail_w, 165, "Fleet: graph semantics", "graph.rs", "SQLite: projects, files, symbols, edges, aliases. Identity = path/name/arity plus rename aliases. Unique name+arity resolves CALLS. Reverse recursive traversal finds dependents. DEFAULT_DEPTH=8; indexed_count=0 refuses.", CYAN, CYAN2, "VERDICT PUBLISHES CHECKED / TOTAL")
    card(c, rail_r, 1960, rail_w, 165, "Fleet: gaps, not claims", "READ BEFORE BUILD", "No cloud DB, bus, catalog, vectors, cross-repo graph, API/event/data/infra/runtime lineage, SSO tenancy, or HA. Three grammars only; ambiguous calls are omitted. MCP impact and lesson_recall are placeholders.", RED, RED2, "ENTERPRISE PLANE = PROPOSED EXTENSION")
    card(c, rail_r, 1780, rail_w, 165, "Contracts to preserve", "FLEET -> ENTERPRISE", "Worker output stays submission-only; parent stamps actor/time/model. Hash-linked receipts become immutable objects + signed checkpoints. Human acceptance stays external. checked=0 fails. Refusals remain typed.", GREEN, GREEN2, "KEEP ENFORCEMENT; REPLACE DISCOVERY/STORAGE")

    s2_y, s2_h = 1220, 515
    section(c, s2_y, s2_h, "02", "CANONICAL KNOWLEDGE PLANE", "one temporal identity model; multiple purpose-built read projections")
    top_y = s2_y + 220
    store_h = 245
    store_gap = 16
    store_w = (main_w - 3 * store_gap) / 4
    stores = [
        ("POSTGRES 17 / AUTHORITY", "RDS/Aurora + RLS", "Canonical URN: urn:devx:{tenant}:{kind}:{namespace}:{name}@{version}\n\nTables: source_document, snapshot, entity, entity_version, edge, chunk, ingestion_run, change_set, context_lease, receipt, feedback, policy_version, outbox.\n\nEvery row: tenant_id, source_uri, source_version/SHA, observed_at, valid_from/to, confidence, provenance_hash.", BLUE, BLUE2, "PARTITION tenant_id + observed_month; DEFAULT DENY RLS"),
        ("NEO4J / GRAPH PROJECTION", "derived; rebuildable", "Nodes: Org, Team, Domain, System, Repo, Service, API, Event, Symbol, Package, Dataset, Job, Resource, SLO, Incident, Policy.\n\nEdges: OWNS, CONTAINS, CALLS, IMPORTS, PRODUCES, CONSUMES, READS, WRITES, DEPLOYS_TO, OBSERVED_CALL, VIOLATED, SUPERSEDES.\n\nEdge carries valid interval, source, confidence and source snapshot.", CYAN, CYAN2, "USE WHEN graph > 50M edges OR recursive CTE p95 > 300 ms"),
        ("PGVECTOR / SEMANTIC", "same tenant boundary", "Do not embed every AST node. Embed exported/hot symbols, docs, contracts, ADRs, runbooks, incident lessons, top fan-in nodes; cold symbols resolve through graph.\n\n768-d halfvec; HNSW; exact tenant/repo filters; iterative scans; model_version + content_hash. PostgreSQL FTS + vector fused with RRF(k=60).", VIOLET, VIOLET2, "ADD OPENSEARCH ONLY IF LEXICAL p95 > 500 ms FOR 14 d"),
        ("S3 / IMMUTABLE EVIDENCE", "object lock where required", "Raw webhook payloads; source snapshots; SCIP indexes; catalog files; contracts; SBOM/provenance; plans; test reports; screenshots/videos; attestations; signed ledger checkpoints.\n\nKey: tenant/source/yyyy/mm/dd/content_hash. PostgreSQL stores URI + digest, never opaque binary blobs.", GREEN, GREEN2, "KMS PER TENANT CLASS; RETENTION FROM POLICY, NOT PROMPTS"),
    ]
    for i, (title, subtitle, body, color, fill, footer) in enumerate(stores):
        x = main_x + i * (store_w + store_gap)
        card(c, x, top_y, store_w, store_h, title, subtitle, body, color, fill, footer)

    # Ingestion feeds the authority; the authority projects to graph, vector and evidence stores.
    ingest_anchor_x = main_x + 3 * (stage_w + stage_gap) + stage_w / 2
    authority_x = main_x + store_w / 2
    arrow(c, [(ingest_anchor_x, pipe_y), (ingest_anchor_x, top_y + store_h + 24), (authority_x, top_y + store_h + 24), (authority_x, top_y + store_h)], BLUE, 1.2, 6)
    projection_bus_y = top_y + store_h + 10
    for idx in (1, 2, 3):
        target_x = main_x + idx * (store_w + store_gap) + store_w / 2
        arrow(c, [(authority_x, projection_bus_y), (target_x, projection_bus_y), (target_x, top_y + store_h)], GREEN, 0.9, 5, [3, 2])

    model_y = s2_y + 20
    model_h = 180
    model_gap = 14
    model_w = (main_w - 2 * model_gap) / 3
    card(c, main_x, model_y, model_w, model_h, "Structural extraction", "NO BLIND CHUNKING", "Code: SCIP first; tree-sitter fallback. One chunk per symbol = signature + doc + enclosing type + imports + stable symbol ID + direct edge IDs. Large body splits on AST blocks at <=1,200 tokens with parent link. Docs split by H1/H2 at <=800 tokens. Exclude generated/vendor/binaries. Contracts are parsed fields, not prose chunks.", CYAN, PAPER2, "SOURCE HASH MAKES INGEST IDEMPOTENT")
    card(c, main_x + model_w + model_gap, model_y, model_w, model_h, "Temporal truth + freshness", "BITEMPORAL READ", "A query pins ticket_version, graph_snapshot_id and repo_sha map. Active entity version is valid_from <= snapshot < valid_to. Push target: event accepted <30 s; changed repo symbol graph <5 min; catalog/contracts <15 min; runtime graph <5 min. If requested SHA is absent, enqueue priority index and return PENDING—not guessed context.", BLUE, PAPER2, "NIGHTLY FULL RECONCILIATION; WEEKLY ORPHAN AUDIT")
    card(c, main_x + 2 * (model_w + model_gap), model_y, model_w, model_h, "Projection consistency", "AT-LEAST-ONCE, IDEMPOTENT", "Postgres transaction writes entity versions + outbox. Debezium publishes event_id. Each projector UPSERTs by tenant_id + projection + source_version + entity_urn and records high-water mark. Context Gateway serves only snapshots whose required projectors reached the same watermark. Lag or conflicting owners appears as unknown/stale coverage and can force clarification.", GREEN, PAPER2, "NO DISTRIBUTED TRANSACTION ACROSS STORES")

    # All projections join at one authorized Context Builder query surface.
    retrieval_bus_y = 1190
    for idx in range(4):
        source_x = main_x + idx * (store_w + store_gap) + store_w / 2
        line(c, source_x, model_y, source_x, retrieval_bus_y, CYAN, 0.8, [3, 2])
    line(c, main_x + store_w / 2, retrieval_bus_y, main_x + 3 * (store_w + store_gap) + store_w / 2, retrieval_bus_y, CYAN, 0.8, [3, 2])

    card(c, left, 1400, rail_w, 270, "Traversal depth budget", "ONLINE QUERY", "D0 exact anchors: ticket links, CODEOWNERS, paths, symbols.\nD1 ownership/policy: 2 hops.\nD2 runtime service neighbors: 2 hops / 30 d window.\nD3 infra + data lineage: 3 hops.\nD4 API/event producer-consumer: 4 hops.\nD5 static reverse CALLS: start 3, expand to max 8 while frontier <=500.\nHard cap: 2,000 nodes / 10,000 edges / 50 repos.", CYAN, CYAN2, "TRUNCATION RETURNS FRONTIER + REASON; >50 REPOS -> ARCH REVIEW")
    card(c, left, 1220, rail_w, 165, "Confidence precedence", "CONFLICT POLICY", "Signed contract/state > exact SCIP edge > catalog declaration > package manifest > observed runtime edge > lexical/LLM inference. Never silently merge contradictions. Preserve all observations, select one effective edge by policy, and emit conflict_count.", ORANGE, ORANGE2, "LOW CONFIDENCE CANNOT AUTHORIZE A DESTRUCTIVE CHANGE")
    card(c, rail_r, 1400, rail_w, 270, "Scale math", "SIZE WHAT EXISTS", "Example: 2,000 repos; 20M symbols; embed 20%=4M high-value chunks. pgvector vector storage is ~4*d+8 bytes: 768-d float ~=3.1 KB/chunk (~12.3 GB raw); halfvec ~=1.5 KB (~6.2 GB raw), before HNSW. All 20M symbols and edges stay structural. Re-embed only content_hash changes. Cache context packs by snapshot + query hash for 15 min.", VIOLET, VIOLET2, "VECTOR EVERYTHING IS A COST AND RECALL ANTI-PATTERN")
    card(c, rail_r, 1220, rail_w, 165, "Tenant isolation", "AUTHZ BEFORE RETRIEVAL", "Postgres RLS default-deny; partition tenant_id. Separate DB/KMS key for regulated tenants. Neo4j database-per-regulated tenant; otherwise mandatory tenant predicates in generated queries. Never share an ANN index across security classes. Retrieval filters precede graph/lexical/vector ranking.", RED, RED2, "AUDIT SUBJECT, PURPOSE, TOOL, ROW COUNT, DIGEST")

    s3_y, s3_h = 650, 530
    section(c, s3_y, s3_h, "03", "TICKET -> CONTEXT LEASE -> MULTI-REPO AGENT EXECUTION", "agents receive a cited view of truth, never the databases")
    query_y = s3_y + 260
    query_h = 220
    query_left_w = 250
    query_mid_w = 930
    query_right_w = main_w - query_left_w - query_mid_w - 32
    card(c, main_x, query_y, query_left_w, query_h, "Resolve change", "INPUT CONTRACT", "ticket_id + ticket_version\nactor + purpose\nrisk_tier + environment\nrepo hints / service hints\nacceptance criteria + NFRs\nrequested tool scopes\nmax_tokens + max_cost\n\nReject superseded version, missing owner, or empty acceptance criteria.", BLUE, BLUE2, "EMITS change_set_id")
    retrieval_x = main_x + query_left_w + 16
    card(c, retrieval_x, query_y, query_mid_w, query_h, "Deterministic retrieval pipeline", "CONTEXT-BUILDER SERVICE", "", CYAN, PAPER2)
    arrow(c, [(main_x + main_w / 2, retrieval_bus_y), (main_x + main_w / 2, query_y + query_h)], CYAN, 1.2, 6)
    retrieval_steps = [
        "Authorize subject + purpose with OIDC/OAuth resource indicator; materialize repo allowlist.",
        "Resolve exact ticket/path/symbol/API/event anchors at pinned SHAs; report unresolved anchors.",
        "Expand typed graph using per-edge depth budgets; reverse dependency direction for impact.",
        "Run lexical top 100 + pgvector top 100 only inside authorized candidate set.",
        "Fuse with Reciprocal Rank Fusion k=60; cross-encoder rerank top 40.",
        "Select with MMR + topology coverage: owners, contracts, tests, runtime, infra, rollback.",
        "Pack <=24k tokens: 35% changed surface, 25% dependencies, 15% tests, 10% standards, 10% runtime, 5% unknowns.",
        "Sign manifest: snapshot, SHA map, source citations, coverage {checked,total,unknown,stale}, truncation and expiry.",
    ]
    numbered(c, retrieval_x + 14, query_y + query_h - 51, query_mid_w - 28, retrieval_steps, CYAN, 6.8, 8.4, 20)
    lease_x = retrieval_x + query_mid_w + 16
    card(c, lease_x, query_y, query_right_w, query_h, "Context lease", "JWT / 30 MIN", "Claims:\nsub, tenant, role, purpose\nchange_set_id\nrepo_allowlist\ntool_scopes\ngraph_snapshot_id\nrepo_sha_map\nrisk_tier\nmax_tokens / max_cost\nexp / jti\n\nOne run, one lease. Revoke on ticket mutation, scope change, or human stop.", VIOLET, VIOLET2, "AUDIENCE = knowledge-mcp")
    arrow(c, [(main_x + query_left_w, query_y + query_h / 2), (retrieval_x - 4, query_y + query_h / 2)], BLUE, 1.5, 6)
    arrow(c, [(retrieval_x + query_mid_w, query_y + query_h / 2), (lease_x - 4, query_y + query_h / 2)], CYAN, 1.5, 6)

    # Fleet sits here: one evidence-enforcing execution pod per child repo/worktree.
    fleet_y = s3_y + 10
    fleet_h = 242
    rect(c, main_x, fleet_y, main_w, fleet_h, HexColor("#F4F7FD"), BLUE, 1.5, 7)
    txt(c, main_x + 14, fleet_y + fleet_h - 20, "FLEET PER-CHANGE EXECUTION POD", "Display-Bold", 10.5, INK)
    txt(c, main_x + 238, fleet_y + fleet_h - 20, "ONE CHILD REPO / ONE CLEAN WORKTREE / ONE TASK-BOUND SOW / ONE EVIDENCE CHAIN", "Mono-Bold", 6.5, BLUE)
    txt(c, main_x + main_w - 14, fleet_y + fleet_h - 20, "GREEN LIVE  /  ORANGE PARTIAL  /  RED EXTERNAL", "Mono-Bold", 6.2, MUTED, "right")

    flow_y = fleet_y + 118
    flow_h = 86
    step_gap = 7
    steps = [
        ("F0", "Run envelope", "Temporal call", "ticket/version + child repo/SHA + task + agent + ContextPack/lease", "PARTIAL"),
        ("F1", "Plan", "fleet plan", "Name intent, role, skills, lane and commands; execute nothing.", "LIVE"),
        ("F2", "SOW", "fleet sow --task", "Require leaves/AC, cited challenge, >=2 alternatives, estimate, edge cases.", "LIVE"),
        ("F3", "Human accept", "fleet sow accept", "Bind exact SOW id, human actor and time. Generator exits 9 before approval.", "LIVE"),
        ("F4", "Impact + context", "graph index / impact / mcp", "Local graph is live; company ContextPack bridge and real MCP impact are not wired.", "PARTIAL"),
        ("F5", "Preflight", "roles / route / skills / meter", "Validate clean Git, agent allowlist, role separation, lane, quota and lease scope.", "PARTIAL"),
        ("F6", "Dispatch", "fleet swarm dispatch", "Accepted SOW -> run_with_evidence; fault/unknown cannot earn credit.", "LIVE"),
        ("F7", "Worker", "spawn_agent + fd 3", "Codex/Claude/freelane adapter; worker returns submission only; parent stamps facts.", "LIVE"),
        ("F8", "Freeze", "git diff + BLAKE3", "Reject dirty start/no new work; freeze content-addressed artifact and run receipt.", "LIVE"),
        ("F9", "Verify", "distinct verifier", "Reproduce; blind suite; adequacy; blast files; rollback proof; observed cost.", "LIVE"),
        ("F10", "Attest + decide", "in-toto + O1/O2", "Attestation, two oracle records, 2x2 adjudication, ratchet, scorecards, ledger.", "LIVE"),
        ("F11", "PR/release handoff", "GitHub/CI/deploy", "Fleet emits artifact + receipts; PR, merge, deploy and prod authority remain outside.", "EXTERNAL"),
    ]
    step_w = (main_w - 28 - (len(steps) - 1) * step_gap) / len(steps)
    for i, values in enumerate(steps):
        x = main_x + 14 + i * (step_w + step_gap)
        fleet_step(c, x, flow_y, step_w, flow_h, *values)
        if i < len(steps) - 1:
            arrow(c, [(x + step_w + 1, flow_y + flow_h / 2), (x + step_w + step_gap - 1, flow_y + flow_h / 2)], BLUE, 0.9, 4)

    # Runtime internals show where context, agents, evidence and external authority connect.
    runtime_y = fleet_y + 18
    runtime_h = 82
    runtime_gap = 10
    runtime_specs = [
        ("CONTEXT SIDECAR", "Company MCP lease -> scoped files, graph, standards. Fleet local rmcp remains repo-prefix scoped.", CYAN, CYAN2),
        ("LEAD / PLANNER", "SOW, clarification, decomposition and lifecycle evidence. No implementation write authority.", VIOLET, VIOLET2),
        ("BUILDER ADAPTER", "Codex / Claude / freelane runs in the child worktree. Only fd3 submission crosses back.", VIOLET, VIOLET2),
        ("VERIFIER + ORACLES", "Different verifier identity; O1 lead and O2 verifier; checked/total cannot be zero.", ORANGE, ORANGE2),
        ("EVIDENCE KERNEL", "Hash-linked ledger, frozen artifact, in-toto attestation, rollback, meter, ratchet, scorecards.", GREEN, GREEN2),
        ("EXTERNAL AUTHORITY", "GitHub App/Actions/rulesets/merge queue -> human approval -> staging/prod/monitoring.", RED, RED2),
    ]
    runtime_w = (main_w - 28 - (len(runtime_specs) - 1) * runtime_gap) / len(runtime_specs)
    for i, (title, body, color, fill) in enumerate(runtime_specs):
        x = main_x + 14 + i * (runtime_w + runtime_gap)
        card(c, x, runtime_y, runtime_w, runtime_h, title, f"R{i+1}", body, color, fill)
        if i < len(runtime_specs) - 1:
            arrow(c, [(x + runtime_w + 1, runtime_y + runtime_h / 2), (x + runtime_w + runtime_gap - 1, runtime_y + runtime_h / 2)], color, 0.8, 4)

    arrow(c, [(lease_x + query_right_w / 2, query_y), (lease_x + query_right_w / 2, fleet_y + fleet_h + 8), (main_x + 65, fleet_y + fleet_h + 8), (main_x + 65, fleet_y + fleet_h)], VIOLET, 1.2, 6)
    txt(c, main_x + 80, fleet_y + fleet_h + 4, "TEMPORAL STARTS ONE FLEET POD PER READY CHILD; FLEET RETURNS ARTIFACT_ID + ATTESTATION + RECEIPT DIGEST", "Mono-Bold", 6.2, BLUE)

    card(c, left, 905, rail_w, 240, "Where Fleet sits", "EXECUTION, NOT ORCHESTRATION", "Temporal/Symphony owns ticket polling, retries, cancellation and the parent DAG. The Knowledge Plane owns company context. For each ready child node, the orchestrator creates a clean worktree/pod and invokes Fleet. Fleet owns local policy, worker isolation, verification, evidence and rollback. GitHub/humans own merge and release.", BLUE, BLUE2, "N PARENT CHILDREN = N INDEPENDENT FLEET RUNS")
    card(c, left, 650, rail_w, 240, "Fleet lifecycle map", "TYPESTATE + RECEIPTS", "Intake -> Specified -> Reviewed(H) -> Decomposed -> Contracted -> Briefed -> Leased -> Building -> Built -> Verifying -> Verified -> Attested -> Accepted(H) -> Observed -> reopen. Refused branches from intake/spec/build/verify/attest. Current CLI and swarm project these states; full automatic wiring remains partial.", VIOLET, VIOLET2, "ILLEGAL TRANSITIONS FAIL; EVIDENCE REQUIRED")
    card(c, rail_r, 905, rail_w, 240, "Fleet input / output", "RUN ENVELOPE CONTRACT", "IN: change_set_id, child_id, ticket/version, task-bound accepted SOW id, repo/worktree path, pinned base SHA, agent/role, ContextPack manifest + lease, risk/budget. OUT: artifact_id, BLAKE3 diff, attestation URI/digest, ledger checkpoint, verification/oracle verdict, adequacy checked/total, blast files, rollback result, cost and refusal code.", GREEN, GREEN2, "ENTERPRISE ADAPTER PERSISTS OUTPUT TO POSTGRES + S3")
    card(c, rail_r, 650, rail_w, 240, "Known wiring gaps", "DO NOT OVERCLAIM", "Fleet is local/file-backed. graph/impact is single-repo and three-language. MCP impact and lesson_recall return capability metadata, not company results. The run path does not yet consume ContextPack, invoke MCP/skills automatically, create worktrees, raise PRs, call Sonar/CodeQL, publish remote receipts, coordinate multi-repo fan-in, deploy, or observe production. Those are explicit adapters/work items.", RED, RED2, "IMPLEMENTATION MUST CLOSE EACH GAP WITH AN ACCEPTANCE TEST")

    s4_y, s4_h = 245, 365
    section(c, s4_y, s4_h, "04", "OPERATE, MEASURE, AND TEACH THE COMPANY", "knowledge changes through governed evidence, not agent memory")
    op_y = s4_y + 18
    op_h = 300
    op_gap = 14
    op_w = (main_w - 4 * op_gap) / 5
    ops = [
        ("DEPLOYMENT", "AWS reference", "EKS: gateway, Temporal workers, indexers, projectors, reconcilers. MSK + Schema Registry. RDS PostgreSQL/pgvector. Neo4j Aura/cluster. S3 Object Lock. ElastiCache for leases/cache. GitHub App + OIDC; Secrets Manager; KMS.", BLUE, BLUE2),
        ("SLO + CAPACITY", "measured denominator", "Webhook accept p95 <2 s. Changed-repo graph p95 <5 min. ContextPack p95 <2 s cached / <8 s cold. Fresh snapshot availability >=99.9%. Every result publishes checked/total/stale. Backpressure by tenant, repo, risk and cost.", CYAN, CYAN2),
        ("OBSERVABILITY", "OpenTelemetry", "Trace ticket -> workflow -> ingestion_run -> context query -> tool call -> child run -> PR -> deploy. Metrics: projection lag, unresolved anchors, graph conflicts, context cache hit, token/cost per accepted change, first-pass green, escaped defects.", GREEN, GREEN2),
        ("LEARNING LOOP", "proposal only", "PR review, CI, incident and agent traces create LearningCase. Miner finds reproducible cause. Standards agent proposes rule/eval/skill/runbook/catalog repair. Human owner approves shadow -> warn -> block rollout. Adoption receipts link outcome to policy version.", VIOLET, VIOLET2),
        ("DISASTER + SECURITY", "prove recovery", "Postgres PITR; S3 versioning/Object Lock; rebuild Neo4j/vector from source snapshots + outbox. Quarterly restore drill. Rotate App keys. Redact secret/PII before embedding. Prompt-injection scanning on untrusted docs. Egress allowlist; no cross-tenant cache keys.", RED, RED2),
    ]
    for i, (title, subtitle, body, color, fill) in enumerate(ops):
        x = main_x + i * (op_w + op_gap)
        card(c, x, op_y, op_w, op_h, title, subtitle, body, color, fill, "OWNER + SLO + RUNBOOK + RESTORE EVIDENCE")

    card(c, left, s4_y, rail_w, s4_h - 34, "Build sequence", "90-DAY IMPLEMENTATION", "1. Weeks 1-2: canonical URN/schema, GitHub App, Backstage provider, S3 raw, Postgres/RLS, outbox.\n2. Weeks 3-5: SCIP ingestion, package/contracts/IaC parsers, temporal versions, reconciliation.\n3. Weeks 6-7: pgvector/FTS retrieval, ContextPack, MCP lease gateway.\n4. Weeks 8-10: parent/child change-set, worktrees, review/evidence integration.\n5. Weeks 11-12: runtime/data edges, learning PR loop, restore and load tests.", BLUE, PAPER2, "PILOT: 20 REPOS / 5 SERVICES / 2 CROSS-REPO CHANGES")
    card(c, rail_r, s4_y, rail_w, s4_h - 34, "Go / stop gates", "FALSIFIABLE", "GO: >=95% repo ownership; >=90% changed symbols indexed <5 min; >=90% known cross-service calls represented; context citation precision >=90% on gold set; stale context <1%; zero unauthorized rows in adversarial tests; restore RTO <4 h; escaped defect rate does not worsen. STOP: any cross-tenant leak, silent zero-input pass, unverifiable context, or autonomous merge/release authority.", RED, PAPER2, "AUTONOMY EXPANDS ONLY AFTER OBSERVED OUTCOMES")

    line(c, M, 216, W - M, 216, INK, 0.9)
    txt(c, M, 201, "OFFICIAL IMPLEMENTATION SOURCES", "Mono-Bold", 7.1, INK)
    source_text = ("[1] Fleet code: keel/fleet/src/{graph,mcp,lifecycle,run}.rs; contracts/*.json; Cargo.toml; crew/pyproject.toml  |  "
        "[2] Backstage catalog, GitHub discovery, entity lifecycle: backstage.io/docs  |  [3] Sourcegraph auto-indexing, SCIP indexer spec, Cody context: sourcegraph.com/docs  |  "
        "[4] DataHub architecture: github.com/datahub-project/datahub/docs/architecture  |  [5] OpenLineage API/events/facets: openlineage.io/docs  |  "
        "[6] pgvector HNSW/IVFFlat/iterative scans/hybrid RRF/partitioning: github.com/pgvector/pgvector  |  [7] Neo4j vector/full-text indexes + authorization limitations: neo4j.com/docs  |  "
        "[8] Debezium Outbox Event Router: debezium.io/documentation/reference/stable/transformations/outbox-event-router.html  |  [9] Temporal durable execution: docs.temporal.io  |  "
        "[10] PostgreSQL 17 row security + partitioning: postgresql.org/docs/17  |  [11] GitHub App webhooks + installation auth: docs.github.com/apps  |  "
        "[12] MCP authorization/tools/tasks: modelcontextprotocol.io/specification/2025-11-25  |  [13] OpenAI Harness Engineering + Symphony: openai.com/index  |  "
        "[14] GitHub rulesets/reusable workflows/merge queue; Pact can-i-deploy; Argo Rollouts; OTel service graph; AWS Application Signals; SLSA v1.0.")
    para(c, M, 184, source_text, W - 2 * M, "Mono", 5.9, 7.4, MUTED, 6)
    line(c, M, 82, W - M, 82, RULE, 0.7)
    txt(c, M, 66, "DESIGN DECISION", "Mono-Bold", 6.7, BLUE)
    txt(c, M + 104, 66, "POSTGRES IS AUTHORITY; GRAPH/VECTOR ARE REBUILDABLE PROJECTIONS. CONTEXT IS A SIGNED, PINNED PRODUCT. AGENT OUTPUT IS A PROPOSAL. HUMAN + DETERMINISTIC CONTROLS OWN IRREVERSIBLE ACTIONS.", "Mono-Bold", 6.5, INK)
    txt(c, M, 39, "DEVX LABS - THE DEVX DOCTRINE v1.0 - CONFIDENTIAL", "Mono", 6.4, MUTED)
    txt(c, W - M, 39, "REFERENCE ARCHITECTURE — VALIDATE AGAINST SECURITY, RESIDENCY, SCALE, AND TOOL ENTITLEMENTS", "Mono", 6.4, MUTED, "right")

    c.showPage()
    c.save()
    print(OUT)


if __name__ == "__main__":
    build()
