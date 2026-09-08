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

M = 72
INK = HexColor("#0A0A0A")
INK2 = HexColor("#1A1A1A")
MUTED = HexColor("#5C6066")
MUTED2 = HexColor("#8A8F96")
RULE = HexColor("#E5E5E5")
PAPER = HexColor("#FFFFFF")
PAPER2 = HexColor("#FAFAF8")
PAPER3 = HexColor("#F4F4F1")
ACCENT = HexColor("#1E6FFF")
ACCENT_SOFT = HexColor("#E8F0FF")
WARN = HexColor("#C0392B")
OK = HexColor("#0A7C53")

FONT_ROOT = "/Users/rachitsrivastava/.agents/skills/canvas-design/canvas-fonts"
pdfmetrics.registerFont(TTFont("Display", os.path.join(FONT_ROOT, "InstrumentSans-Regular.ttf")))
pdfmetrics.registerFont(TTFont("Display-Bold", os.path.join(FONT_ROOT, "InstrumentSans-Bold.ttf")))
pdfmetrics.registerFont(TTFont("Editorial-Italic", os.path.join(FONT_ROOT, "IBMPlexSerif-Italic.ttf")))
pdfmetrics.registerFont(TTFont("Mono", os.path.join(FONT_ROOT, "GeistMono-Regular.ttf")))
pdfmetrics.registerFont(TTFont("Mono-Bold", os.path.join(FONT_ROOT, "GeistMono-Bold.ttf")))


def text(c, x, y, value, font="Display", size=11, color=INK, align="left"):
    c.setFillColor(color)
    c.setFont(font, size)
    if align == "right":
        c.drawRightString(x, y, value)
    elif align == "center":
        c.drawCentredString(x, y, value)
    else:
        c.drawString(x, y, value)


def wrap(value, font, size, width):
    result = []
    for source_line in value.split("\n"):
        words = source_line.split()
        current = ""
        for word in words:
            trial = word if not current else f"{current} {word}"
            if stringWidth(trial, font, size) <= width:
                current = trial
            else:
                if current:
                    result.append(current)
                current = word
        if current:
            result.append(current)
    return result


def paragraph(c, x, y, value, width, font="Display", size=10, leading=13, color=MUTED, max_lines=None):
    lines = wrap(value, font, size, width)
    if max_lines is not None:
        lines = lines[:max_lines]
    for i, value_line in enumerate(lines):
        text(c, x, y - i * leading, value_line, font, size, color)
    return y - len(lines) * leading


def rect(c, x, y, w, h, fill=PAPER, stroke=RULE, sw=0.8):
    c.setFillColor(fill)
    c.setStrokeColor(stroke)
    c.setLineWidth(sw)
    c.rect(x, y, w, h, fill=1, stroke=1)


def line(c, x1, y1, x2, y2, color=RULE, sw=0.8):
    c.setStrokeColor(color)
    c.setLineWidth(sw)
    c.line(x1, y1, x2, y2)


def arrow(c, points, color=ACCENT, sw=1.8, head=8):
    c.setStrokeColor(color)
    c.setFillColor(color)
    c.setLineWidth(sw)
    path = c.beginPath()
    path.moveTo(points[0][0], points[0][1])
    for px, py in points[1:]:
        path.lineTo(px, py)
    c.drawPath(path, fill=0, stroke=1)
    x1, y1 = points[-2]
    x2, y2 = points[-1]
    angle = math.atan2(y2 - y1, x2 - x1)
    p1 = (x2 - head * math.cos(angle - 0.48), y2 - head * math.sin(angle - 0.48))
    p2 = (x2 - head * math.cos(angle + 0.48), y2 - head * math.sin(angle + 0.48))
    tri = c.beginPath()
    tri.moveTo(x2, y2)
    tri.lineTo(p1[0], p1[1])
    tri.lineTo(p2[0], p2[1])
    tri.close()
    c.drawPath(tri, fill=1, stroke=0)


def tag(c, x, y, value, color=MUTED):
    text(c, x, y, value.upper(), "Mono-Bold", 8.5, color)


def band(c, y, h, number, title_value, descriptor):
    text(c, M, y + h - 20, number, "Mono-Bold", 10, ACCENT)
    text(c, M + 36, y + h - 20, title_value.upper(), "Mono-Bold", 10, INK)
    text(c, M + 275, y + h - 20, descriptor, "Mono", 8.7, MUTED)
    line(c, M, y + h - 34, W - M, y + h - 34, INK, 0.9)


def compact_card(c, x, y, w, h, number, title_value, owner, trigger, tools, output, kind="agent"):
    fill = ACCENT_SOFT if kind == "agent" else PAPER3 if kind == "gate" else PAPER2
    border = WARN if kind == "human" else INK if kind == "gate" else RULE
    rect(c, x, y, w, h, fill, border, 1.1 if kind in ("human", "gate") else 0.8)
    strip = WARN if kind == "human" else INK if kind == "gate" else ACCENT
    c.setFillColor(strip)
    c.rect(x, y + h - 34, w, 34, fill=1, stroke=0)
    text(c, x + 10, y + h - 22, number, "Mono-Bold", 8.2, PAPER)
    text(c, x + 39, y + h - 22, title_value.upper(), "Display-Bold", 10.2, PAPER)
    yy = y + h - 52
    tag(c, x + 12, yy, "OWNER", ACCENT if kind != "human" else WARN)
    yy = paragraph(c, x + 12, yy - 14, owner, w - 24, "Display-Bold", 8.7, 10.5, INK, 2) - 3
    tag(c, x + 12, yy, "TRIGGER")
    yy = paragraph(c, x + 12, yy - 14, trigger, w - 24, "Mono", 7.6, 9.4, INK2, 3) - 3
    tag(c, x + 12, yy, "TOOLS / METHOD")
    paragraph(c, x + 12, yy - 14, tools, w - 24, "Mono", 7.4, 9.2, MUTED, 6)
    line(c, x + 12, y + 31, x + w - 12, y + 31, RULE, 0.6)
    tag(c, x + 12, y + 17, "EMITS", OK)
    paragraph(c, x + 68, y + 17, output, w - 80, "Display-Bold", 7.7, 9.1, INK2, 2)


def rail_block(c, x, y, w, title_value, lines, dark=False):
    h = 45 + len(lines) * 39
    fill = INK if dark else PAPER2
    rect(c, x, y, w, h, fill, INK, 0.9)
    text(c, x + 14, y + h - 24, title_value.upper(), "Mono-Bold", 8.7, ACCENT if dark else INK)
    line(c, x + 14, y + h - 34, x + w - 14, y + h - 34, MUTED2 if dark else RULE, 0.6)
    yy = y + h - 56
    for i, value in enumerate(lines, 1):
        text(c, x + 14, yy, f"{i:02d}", "Mono-Bold", 7.4, ACCENT)
        paragraph(c, x + 40, yy, value, w - 54, "Display", 7.8, 9.4, PAPER if dark else MUTED, 3)
        yy -= 39


def lane(c, x, y, w, h, code, title_value, strategy, steps, special=False):
    rect(c, x, y, w, h, PAPER3 if special else PAPER2, RULE, 0.8)
    label_w = 250
    c.setFillColor(INK if special else ACCENT)
    c.rect(x, y, label_w, h, fill=1, stroke=0)
    text(c, x + 14, y + h - 23, code, "Mono-Bold", 8, ACCENT if special else PAPER)
    text(c, x + 14, y + h - 44, title_value.upper(), "Display-Bold", 10.2, PAPER)
    paragraph(c, x + 14, y + h - 62, strategy, label_w - 28, "Mono", 7.2, 8.8, MUTED2 if special else PAPER, 5)

    step_x = x + label_w
    step_w = (w - label_w) / len(steps)
    for i, (head, body) in enumerate(steps):
        sx = step_x + i * step_w
        if i:
            line(c, sx, y + 10, sx, y + h - 10, RULE, 0.7)
        text(c, sx + 13, y + h - 24, head.upper(), "Mono-Bold", 7.8, ACCENT if not special else INK)
        paragraph(c, sx + 13, y + h - 43, body, step_w - 26, "Display", 7.7, 9.4, INK2, 6)
        if i < len(steps) - 1:
            arrow(c, [(sx + step_w - 20, y + h / 2), (sx + step_w + 7, y + h / 2)], ACCENT, 1.1, 5)


def matrix_cell(c, x, y, w, h, code, bottleneck, control):
    rect(c, x, y, w, h, PAPER, RULE, 0.6)
    text(c, x + 10, y + h - 19, code, "Mono-Bold", 7.4, WARN)
    text(c, x + 39, y + h - 19, bottleneck.upper(), "Display-Bold", 8.2, INK)
    paragraph(c, x + 10, y + h - 38, control, w - 20, "Display", 7.2, 8.8, MUTED, 4)


def build():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    c = canvas.Canvas(OUT, pagesize=(W, H), pageCompression=1)
    c.setTitle("DevX Labs - Company-scale AI-native SDLC Low-Level Design")
    c.setAuthor("DevX Labs")
    c.setSubject("Multi-repository, microservice, infrastructure, release, and learning control graph")
    c.setFillColor(PAPER)
    c.rect(0, 0, W, H, fill=1, stroke=0)

    text(c, M, H - 46, "DEVX LABS / CTO REFERENCE ARCHITECTURE", "Mono-Bold", 9.5, ACCENT)
    text(c, W - M, H - 46, "LLD-02 / 25 AUG 2026 / A0 / ONE PAGE", "Mono", 9, MUTED, "right")
    line(c, M, H - 63, W - M, H - 63, INK, 1)
    title_a = "One ticket. Many repositories."
    text(c, M, H - 110, title_a, "Display-Bold", 31, INK)
    text(c, M + stringWidth(title_a, "Display-Bold", 31) + 16, H - 110, "One controlled change.", "Editorial-Italic", 31, ACCENT)
    paragraph(c, M, H - 136, "A company-scale AI-native SDLC operating system: reconcile five dependency graphs, fan work out to isolated repositories, fan evidence in to one release decision, and convert every failure into a governed organizational capability.", W - 2 * M, "Display", 10.8, 14, MUTED, 2)

    legend_y = H - 169
    text(c, M, legend_y, "READING KEY", "Mono-Bold", 7.8, MUTED)
    legend = [(ACCENT, "AGENT"), (INK, "DETERMINISTIC CONTROL"), (WARN, "HUMAN AUTHORITY"), (OK, "EVIDENCE"), (PAPER3, "SHARED PLATFORM")]
    lx = M + 92
    for color, label_value in legend:
        c.setFillColor(color)
        c.rect(lx, legend_y - 2, 12, 8, fill=1, stroke=0)
        text(c, lx + 18, legend_y, label_value, "Mono", 7.2, MUTED)
        lx += 18 + stringWidth(label_value, "Mono", 7.2) + 26
    text(c, W - M, legend_y, "WORK ID = TICKET VERSION + CHANGE-SET ID + CHILD REPO SHA + EVIDENCE DIGEST", "Mono-Bold", 7.7, INK, "right")

    central_x = 335
    central_w = W - 2 * central_x
    left_x = M
    rail_w = 235
    right_x = W - M - rail_w

    control_y, control_h = 1765, 360
    band(c, control_y, control_h, "01", "INTENT + IMPACT CONTROL", "the parent change-set is the durable unit of work; no repository can finish it alone")
    card_y = control_y + 20
    card_h = 292
    gap = 16
    card_w = (central_w - 5 * gap) / 6
    cards = [
        ("01", "Ready event", "Product owner + webhook gateway", "Jira Ready-for-Agent; Linear status; GitHub issue label", "Jira Automation / Linear webhook / GitHub App; HMAC; schema validation; replay guard", "normalized event + version", "gate"),
        ("02", "Claim + route", "Lifecycle controller", "EventBridge -> SQS FIFO; event accepted", "Temporal / Step Functions / LangGraph; lease; idempotency; checkpoint; cancel superseded runs", "one active run", "gate"),
        ("03", "Parent change-set", "Change coordinator agent", "ticket claimed at exact version", "CS-ID; objective; AC/NFR; risk; budget; target env; child states; global rollback", "versioned change manifest", "agent"),
        ("04", "Five-graph resolve", "Topology + impact agent", "change manifest has candidate capabilities", "Backstage; Sourcegraph/SCIP; Nx/Bazel; OpenAPI/AsyncAPI/Pact/Confluent; Terraform/dbt; OTel/Application Signals", "affected DAG + confidence", "agent"),
        ("05", "Path + risk", "Policy classifier", "static/runtime/interface graphs reconciled", "single repo / cross repo / API-event / data / IaC / incident; blast radius; compliance; reversibility", "required controls + owners", "gate"),
        ("06", "Clarify + approve plan", "Planner agent + product/architecture/security humans", "unknown owner; graph conflict; missing AC/NFR; destructive or high-risk path", "ticket questions; ADR; plan.md; release/rollback plan; CODEOWNERS reservation", "approved parent + child plans", "human"),
    ]
    for i, values in enumerate(cards):
        x = central_x + i * (card_w + gap)
        compact_card(c, x, card_y, card_w, card_h, *values)
        if i < len(cards) - 1:
            arrow(c, [(x + card_w + 2, card_y + card_h / 2), (x + card_w + gap - 2, card_y + card_h / 2)], ACCENT, 1.4, 6)

    rail_block(c, left_x, 1405, rail_w, "CONTEXT PLANE / FIVE GRAPHS", [
        "Ownership: Backstage domain, system, component, API, resource, owner, lifecycle, repo.",
        "Static: Sourcegraph/SCIP references; Nx affected; Bazel deps/rdeps; SBOM.",
        "Interface: OpenAPI, AsyncAPI, Pact matrix, Confluent schema compatibility.",
        "Infra/data: Terraform state/plan graph; Kubernetes resources; dbt/OpenLineage lineage.",
        "Runtime: OpenTelemetry service graph or CloudWatch Application Signals topology/SLOs.",
    ])
    rail_block(c, left_x, 1110, rail_w, "EXECUTION FABRIC", [
        "Sandboxes: Codex cloud / Claude Code / K8s Job; one scoped identity per child.",
        "Isolation: git worktree; ephemeral namespace; Testcontainers/WireMock; TTL cleanup.",
        "Shared acceleration: artifact registry; remote build cache; dependency proxy.",
        "Secrets: GitHub App/OIDC + Vault/Secrets Manager; short-lived and least privilege.",
    ], dark=True)
    rail_block(c, right_x, 1450, rail_w, "AUTHORITY PLANE", [
        "Controller owns state. Agents propose patches, findings, plans, and receipts.",
        "Org rulesets + CODEOWNERS + environment protection own merge/release authority.",
        "Approval binds exact SHA, evidence digest, risk tier, actor, and expiry.",
        "Risk policy controls models, tools, egress, budget, retries, and escalation.",
    ], dark=True)
    rail_block(c, right_x, 1110, rail_w, "EVIDENCE LEDGER", [
        "Input: ticket/version, context sources, graph snapshot, unknowns, plan/ADR.",
        "Build: child SHA, tool calls, tests checked/total, scans, artifacts/provenance.",
        "Decision: findings, waivers, approvals, merge group, deploy and rollback receipts.",
        "Outcome: SLO window, user impact, incident link, learning proposal and adoption.",
    ])

    exec_y, exec_h = 1100, 625
    band(c, exec_y, exec_h, "02", "MULTI-REPOSITORY EXECUTION", "interface-first fan-out; isolated child work; parent fan-in waits for complete system evidence")
    planner_x, planner_y, planner_w, planner_h = central_x, exec_y + 22, 275, 555
    compact_card(c, planner_x, planner_y, planner_w, planner_h, "07", "Child DAG", "Decomposition + scheduling agent", "approved parent plan", "Create child tickets per repo/service; reserve reviewers; topological edges; file scopes; concurrency groups; budget; cancellation; release order", "ready child leaves", "agent")
    lanes_x = planner_x + planner_w + 22
    fanin_w = 290
    lanes_w = central_x + central_w - lanes_x - fanin_w - 22
    lane_h = 126
    lane_gap = 12
    lane_specs = [
        ("A", "CONTRACT / SCHEMA", "Publish compatibility first; use versioning instead of unnecessary synchronized deploys.", [("SPEC", "OpenAPI/AsyncAPI diff; Pact consumer contract; Confluent compatibility."), ("TEST FIRST", "Provider verification + consumer fixtures; negative/boundary cases."), ("PR", "Versioned artifact; deprecation window; owner approval."), ("RECEIPT", "Pact matrix/registry result; deployment dependency facts.")], False),
        ("B", "PRODUCER SERVICE", "Provider remains backward compatible while old and new consumers coexist.", [("CHILD", "Repo ticket + reviewer + exact dependency SHAs/interface version."), ("WORKTREE", "Implementor: smallest TODO -> test -> edit -> lint/type."), ("VERIFY", "Unit/integration/contract; Sonar; CodeQL; SBOM."), ("PR", "Draft PR + impact report + evidence + rollback hook.")], False),
        ("C", "CONSUMERS / UI", "Fan out to every live consumer: web, mobile, batch, and downstream service.", [("CHILDREN", "Nx/Bazel/SCIP + runtime callers -> one child per owner/repo."), ("WORKTREES", "Parallel workers; Playwright component/E2E; axe."), ("BUDGETS", "Lighthouse CI; k6 smoke; bundle/API latency."), ("PRS", "Independent review; screenshots/traces; compatibility proof.")], False),
        ("D", "DATA + INFRA", "Serialize irreversible resources; expand/contract for data and plan/apply for IaC.", [("DATA", "Additive schema -> dual read/write -> backfill -> verify -> contract."), ("IAC", "Terraform refresh/plan; state lock; OPA/Sentinel; Infracost."), ("CAPACITY", "Kubernetes ResourceQuota; AWS Service Quotas; load model; headroom."), ("PR", "DBA/platform/security approval; saved plan; restore test.")], True),
    ]
    for idx, spec in enumerate(lane_specs):
        ly = planner_y + planner_h - (idx + 1) * lane_h - idx * lane_gap
        lane(c, lanes_x, ly, lanes_w, lane_h, *spec)
    fanin_x = lanes_x + lanes_w + 22
    compact_card(c, fanin_x, planner_y, fanin_w, planner_h, "08", "Parent fan-in", "Change coordinator + evidence controller", "all required child PRs and compatibility receipts exist", "Verify graph closure; child SHA set; no missing owner; no zero-input gate; integration manifest; parent status projection to Jira/Linear", "integration candidate or stop", "gate")
    arrow(c, [(planner_x + planner_w, planner_y + planner_h / 2), (lanes_x - 7, planner_y + planner_h / 2)], ACCENT, 1.8, 7)
    arrow(c, [(lanes_x + lanes_w, planner_y + planner_h / 2), (fanin_x - 7, planner_y + planner_h / 2)], ACCENT, 1.8, 7)

    release_y, release_h = 745, 315
    band(c, release_y, release_h, "03", "SYSTEM ASSURANCE + COORDINATED RELEASE", "validate the combined change, preserve compatibility, and expose progressively")
    release_card_y = release_y + 20
    release_card_h = 247
    release_gap = 14
    release_w = (central_w - 6 * release_gap) / 7
    releases = [
        ("09", "Impact review", "Independent reviewer agent", "parent fan-in complete", "Recompute surface; browser/API journeys; threat delta; unchanged behavior", "findings closed", "agent"),
        ("10", "System environment", "Environment allocator", "review candidate", "Ephemeral K8s namespace; pinned artifacts; Testcontainers; synthetic data; TTL", "integration URL", "gate"),
        ("11", "CI firewall", "Deterministic assurance", "PR + merge_group", "Reusable Actions: unit/contract/E2E; Sonar; CodeQL; Trivy; ZAP; Lighthouse; k6; SLSA", "checked/total green", "gate"),
        ("12", "Human decision", "CODEOWNERS + risk owners", "all required evidence green", "Product intent; architecture; security; DBA/platform; exact SHA; waiver expiry", "approve / reject", "human"),
        ("13", "Merge coordinator", "Protected merge authority", "all child approvals valid", "GitHub org rulesets + merge queue; dependency order; retest; abort partial merge", "main SHAs + artifacts", "gate"),
        ("14", "Deployment DAG", "Release controller", "artifacts promoted", "Pact can-i-deploy; expand before consumers; provider before new consumer; contract after grace", "ordered stage plan", "gate"),
        ("15", "Canary + observe", "Argo Rollouts + SRE/human", "stage gates + release approval", "Flags; canary/blue-green; SLO analysis; OTel/App Signals; auto rollback", "healthy prod or incident", "human"),
    ]
    for i, values in enumerate(releases):
        x = central_x + i * (release_w + release_gap)
        compact_card(c, x, release_card_y, release_w, release_card_h, *values)
        if i < len(releases) - 1:
            arrow(c, [(x + release_w + 1, release_card_y + release_card_h / 2), (x + release_w + release_gap - 1, release_card_y + release_card_h / 2)], ACCENT, 1.2, 5)
    arrow(c, [(fanin_x + fanin_w / 2, exec_y + 22), (fanin_x + fanin_w / 2, release_y + release_h - 42), (central_x + release_w / 2, release_y + release_h - 42), (central_x + release_w / 2, release_card_y + release_card_h)], ACCENT, 1.8, 7)
    return_y = release_y - 18
    claim_x = central_x + card_w + gap
    arrow(c, [(central_x + 6 * (release_w + release_gap) + release_w / 2, release_card_y), (central_x + 6 * (release_w + release_gap) + release_w / 2, return_y), (central_x - 18, return_y), (central_x - 18, card_y + card_h / 2), (claim_x, card_y + card_h / 2)], WARN, 1.3, 6)
    text(c, central_x + central_w / 2, return_y + 7, "ALARM / FAILED CANARY -> ROLLBACK FIRST WHERE SAFE -> VERSIONED INCIDENT OR REMEDIATION CHANGE-SET -> SAME CONTROL PATH", "Mono-Bold", 7.5, WARN, "center")

    learn_y, learn_h = 405, 290
    band(c, learn_y, learn_h, "04", "COMPANY-WIDE LEARNING + AUTONOMY", "human judgment is captured once, validated, then distributed as executable policy")
    learn_card_y = learn_y + 20
    learn_card_h = 222
    learn_gap = 14
    learn_w = (central_w - 5 * learn_gap) / 6
    learning = [
        ("16", "Learning signals", "Evidence collector", "PR comment; CI failure; incident; escaped bug; agent trace; cost spike", "GitHub/Jira; CloudWatch; OTel/LangSmith; review labels; outcome metrics", "normalized learning case", "gate"),
        ("17", "Mine root pattern", "Learning / failure-miner agent", "repeated or high-severity signal", "Cluster by cause; reproduce; compare success; locate missing capability/guardrail/context", "cause + candidate control", "agent"),
        ("18", "Propose capability", "Standards agent", "cause confirmed", "Rule, structural test, eval, skill, runbook, template, catalog fix, routing change", "versioned standards PR", "agent"),
        ("19", "Govern change", "Platform/security/architecture owner", "standards PR + eval evidence", "Shadow -> warn -> block; exception owner/expiry; canary repos; rollback threshold", "approved rollout policy", "human"),
        ("20", "Distribute", "Platform controller", "policy approved", "GitHub org rulesets; reusable workflows; Backstage templates; OPA bundles; Renovate; repo bots", "adoption receipts", "gate"),
        ("21", "Measure autonomy", "CTO / engineering intelligence", "rules deployed + outcomes observed", "First-pass green; escape rate; cost/accepted change; DORA; queue P95; false positives", "promote, hold, or reduce autonomy", "human"),
    ]
    for i, values in enumerate(learning):
        x = central_x + i * (learn_w + learn_gap)
        compact_card(c, x, learn_card_y, learn_w, learn_card_h, *values)
        if i < len(learning) - 1:
            arrow(c, [(x + learn_w + 1, learn_card_y + learn_card_h / 2), (x + learn_w + learn_gap - 1, learn_card_y + learn_card_h / 2)], ACCENT, 1.2, 5)
    arrow(c, [(central_x + 5 * (learn_w + learn_gap) + learn_w, learn_card_y + 30), (right_x + rail_w + 15, learn_card_y + 30), (right_x + rail_w + 15, control_y + 130), (right_x + rail_w, control_y + 130)], ACCENT, 1.2, 6)

    bott_y, bott_h = 110, 250
    band(c, bott_y, bott_h, "05", "BOTTLENECK + FAILURE CONTROL MATRIX", "capacity and coordination limits are scheduled resources, not surprises discovered during deployment")
    matrix_y = bott_y + 16
    matrix_h = 90
    matrix_gap = 8
    matrix_w = (W - 2 * M - 4 * matrix_gap) / 5
    bottlenecks = [
        ("B1", "Stale topology", "Reconcile catalog + static + contract + runtime graphs. Unknown owner/disagreement -> clarify; never guess."),
        ("B2", "Partial cross-repo", "Parent fan-in blocks release; cancel or compensate child runs; one green PR is not completion."),
        ("B3", "Contract / event break", "Additive versioned change; Pact Matrix; AsyncAPI/schema-registry compatibility and deprecation window."),
        ("B4", "Database cutover", "Expand -> dual read/write -> backfill -> verify counts/checksums -> switch -> grace -> contract."),
        ("B5", "IaC lock / drift", "Serialize by Terraform state; refresh and re-plan before exact apply; force-unlock only by operator."),
        ("B6", "CI / runner saturation", "Weighted queue by risk/age; affected tests + remote cache; concurrency groups; queue P50/P95."),
        ("B7", "Environment scarcity", "Namespace allocator; quota preflight; TTL cleanup; immutable fixture/artifact IDs; fail if zero capacity."),
        ("B8", "Cloud capacity / quota", "Service Quotas + K8s request/limit/headroom; load model and reservation before rollout."),
        ("B9", "Agent runaway / deadlock", "Checkpoint; bounded retry; lease timeout; cost/tool budget; alternate worker; kill + escalation."),
        ("B10", "Review / incident overload", "Reserve risk owners early; SLA queues; rollback first; encode repeated findings centrally."),
    ]
    for i, values in enumerate(bottlenecks):
        row = i // 5
        col = i % 5
        x = M + col * (matrix_w + matrix_gap)
        y = matrix_y + (1 - row) * (matrix_h + 8)
        matrix_cell(c, x, y, matrix_w, matrix_h, *values)

    inv_y = 82
    text(c, M, inv_y, "NON-NEGOTIABLES", "Mono-Bold", 7.4, ACCENT)
    invariants = "ONE CONTROLLER OWNS STATE  /  AGENTS HAVE NO MERGE OR PRODUCTION AUTHORITY  /  CHECKED=0 FAILS  /  APPROVALS BIND EXACT CONTENT  /  EVERY EXCEPTION HAS OWNER + EXPIRY  /  EVERY DEPLOY HAS ROLLBACK + OBSERVATION WINDOW"
    text(c, M + 115, inv_y, invariants, "Mono-Bold", 6.8, INK)
    text(c, W - M, inv_y, "PRIMARY FLOW BLUE / HUMAN AUTHORITY RED / FAILURE RETURN RED", "Mono", 6.7, MUTED, "right")
    line(c, M, 65, W - M, 65, RULE, 0.7)
    text(c, M, 52, "OFFICIAL SOURCE REGISTER", "Mono-Bold", 6.7, MUTED)
    sources = ("S1 openai.com/index/harness-engineering + open-source-codex-orchestration-symphony  /  S2 backstage.io/docs/features/software-catalog/system-model  /  S3 docs.github.com organization-rulesets + reusable-workflows + merge-queue  /  S4 docs.pact.io/pact_broker/can_i_deploy + asyncapi.com/docs + docs.confluent.io/schema-registry/schema-evolution  /  S5 developer.hashicorp.com/terraform state/locking + plan + policy  /  S6 kubernetes.io/resource-quotas + AWS Service Quotas  /  S7 argo-rollouts.readthedocs.io analysis + rollback  /  S8 docs.aws.amazon.com/CloudWatch/Application-Signals + opentelemetry.io service graph  /  S9 slsa.dev/spec/v1.0  /  S10 SonarQube + CodeQL + Playwright + Lighthouse CI + k6 + OWASP ZAP")
    paragraph(c, M + 119, 52, sources, W - 2 * M - 119, "Mono", 5.8, 7.0, MUTED, 3)
    line(c, M, 23, W - M, 23, RULE, 0.7)
    text(c, M, 10, "DEVX LABS - THE DEVX DOCTRINE v1.0 - CONFIDENTIAL", "Mono", 6.6, MUTED)
    text(c, W - M, 10, "REFERENCE STACK: ADAPT TO CLOUD, REGULATORY, RESIDENCY, AND TOOL ENTITLEMENTS", "Mono", 6.6, MUTED, "right")

    c.showPage()
    c.save()
    print(OUT)


if __name__ == "__main__":
    build()
