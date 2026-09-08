from reportlab.pdfgen import canvas
from reportlab.lib.pagesizes import A1, landscape
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfbase.pdfmetrics import stringWidth
from reportlab.lib.colors import HexColor
import math
import os


ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
OUT = os.path.join(ROOT, "output/pdf/devx-ai-native-sdlc-lld.pdf")
W, H = landscape(A1)

M = 72
INK = HexColor("#0A0A0A")
INK2 = HexColor("#1A1A1A")
MUTED = HexColor("#5C6066")
MUTED2 = HexColor("#8A8F96")
RULE = HexColor("#E5E5E5")
RULE2 = HexColor("#F0F0F0")
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
    lines = []
    for paragraph in value.split("\n"):
        words = paragraph.split()
        line = ""
        for word in words:
            trial = word if not line else f"{line} {word}"
            if stringWidth(trial, font, size) <= width:
                line = trial
            else:
                if line:
                    lines.append(line)
                line = word
        if line:
            lines.append(line)
    return lines


def paragraph(c, x, y, value, width, font="Display", size=10, leading=13, color=MUTED, max_lines=None):
    lines = wrap(value, font, size, width)
    if max_lines is not None:
        lines = lines[:max_lines]
    for i, line in enumerate(lines):
        text(c, x, y - i * leading, line, font, size, color)
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


def arrow(c, points, color=ACCENT, sw=2.0, head=9):
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
    text(c, x, y, value.upper(), "Mono", 8.2, color)


def section_label(c, x, y, number, label, descriptor):
    text(c, x, y, number, "Mono-Bold", 10, ACCENT)
    text(c, x + 34, y, label.upper(), "Mono-Bold", 10, INK)
    text(c, x + 190, y, descriptor, "Mono", 9, MUTED)
    line(c, x, y - 13, W - M, y - 13, INK, 0.9)


def node(c, x, y, w, h, spec):
    kind = spec["kind"]
    fill = ACCENT_SOFT if kind == "agent" else PAPER3 if kind == "gate" else PAPER2
    stroke = INK if kind == "human" else RULE
    rect(c, x, y, w, h, fill, stroke, 1.1 if kind == "human" else 0.8)
    strip = WARN if kind == "human" else ACCENT if kind == "agent" else INK if kind == "gate" else MUTED
    c.setFillColor(strip)
    c.rect(x, y + h - 38, w, 38, fill=1, stroke=0)
    text(c, x + 12, y + h - 25, f'{spec["id"]:02d}', "Mono-Bold", 9, PAPER)
    text(c, x + 42, y + h - 25, spec["title"].upper(), "Display-Bold", 11.2, PAPER)
    text(c, x + w - 12, y + h - 25, spec["source"], "Mono", 7.2, MUTED2 if strip == INK else PAPER, "right")

    tx = x + 14
    tw = w - 28
    yy = y + h - 58
    tag(c, tx, yy, "TRIGGER", ACCENT if kind == "agent" else MUTED)
    yy = paragraph(c, tx, yy - 15, spec["trigger"], tw, "Mono", 8.4, 10.8, INK2, 3) - 5
    tag(c, tx, yy, "OWNER / AGENT", ACCENT if kind == "agent" else MUTED)
    yy = paragraph(c, tx, yy - 15, spec["owner"], tw, "Display-Bold", 9.4, 11.5, INK, 3) - 5
    tag(c, tx, yy, "TOOLS", ACCENT if kind == "agent" else MUTED)
    yy = paragraph(c, tx, yy - 15, spec["tools"], tw, "Mono", 8.1, 10.4, MUTED, 4) - 5
    line(c, tx, y + 40, x + w - 14, y + 40, RULE, 0.6)
    tag(c, tx, y + 25, "OUTPUT / GATE", OK if kind != "human" else WARN)
    paragraph(c, tx + 92, y + 25, spec["output"], tw - 92, "Display-Bold", 8.5, 10.2, INK2, 2)


def loop_connector(c, end_x, row_bottom, next_start_x, next_top, label):
    edge_x = W - M + 24
    mid_y = row_bottom - 33
    arrow(c, [(end_x, row_bottom + 135), (edge_x, row_bottom + 135), (edge_x, mid_y), (next_start_x - 20, mid_y), (next_start_x - 20, next_top - 135), (next_start_x, next_top - 135)], ACCENT, 2.2, 9)
    text(c, W / 2, mid_y + 9, label.upper(), "Mono-Bold", 8.2, ACCENT, "center")


def exception_cell(c, x, y, w, code, head, body):
    text(c, x, y, code, "Mono-Bold", 8.5, WARN)
    text(c, x + 34, y, head, "Display-Bold", 9.1, INK)
    paragraph(c, x + 34, y - 14, body, w - 42, "Display", 7.8, 9.7, MUTED, 3)


def build():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    c = canvas.Canvas(OUT, pagesize=(W, H), pageCompression=1)
    c.setTitle("DevX Labs - AI-native SDLC Low-Level Design")
    c.setAuthor("DevX Labs")
    c.setSubject("Ticket-to-production agentic SDLC control-flow graph")
    c.setFillColor(PAPER)
    c.rect(0, 0, W, H, fill=1, stroke=0)

    # Header
    text(c, M, H - 54, "DEVX LABS / CTO REFERENCE ARCHITECTURE", "Mono-Bold", 9.5, ACCENT)
    text(c, W - M, H - 54, "LLD-01 / 25 AUG 2026 / A1", "Mono", 9, MUTED, "right")
    line(c, M, H - 72, W - M, H - 72, INK, 1)
    headline = "Ticket to production,"
    text(c, M, H - 122, headline, "Display-Bold", 34, INK)
    text(c, M + stringWidth(headline, "Display-Bold", 34) + 18, H - 122, "closed by evidence", "Editorial-Italic", 34, ACCENT)
    paragraph(c, M, H - 151, "One implementable SDLC control flow. Every state names its trigger, actor, tool surface, output, authority, and failure return path.", W - 2 * M, "Display", 11.5, 15, MUTED, 2)
    legend_y = H - 181
    legend_x = W - M - 720
    text(c, legend_x, legend_y, "LEGEND", "Mono-Bold", 7.8, MUTED)
    legend = [(ACCENT, "AGENT"), (INK, "DETERMINISTIC"), (WARN, "HUMAN"), (MUTED, "EVENT"), (ACCENT_SOFT, "PRIMARY FLOW"), (PAPER3, "RED LINE = RETURN")]
    lx = legend_x + 62
    for color, label in legend:
        c.setFillColor(color)
        c.rect(lx, legend_y - 2, 12, 8, fill=1, stroke=0)
        text(c, lx + 18, legend_y, label, "Mono", 7.2, MUTED)
        lx += 18 + stringWidth(label, "Mono", 7.2) + 22

    # Layout
    gap = 18
    node_w = (W - 2 * M - 6 * gap) / 7
    node_h = 270
    row_ys = [H - 500, H - 858, H - 1216]
    section_ys = [H - 205, H - 563, H - 921]

    rows = [
        [
            dict(id=1, title="Ready ticket", kind="event", source="S1/S3/S4", trigger="Jira transition; Linear status; GitHub issues.labeled or assigned", owner="Product owner sets Ready-for-Agent", tools="Jira Automation / Linear webhook / GitHub App", output="eligible ticket + version"),
            dict(id=2, title="Signed event", kind="gate", source="S3", trigger="Webhook delivery", owner="Webhook gateway validates source", tools="API Gateway / Cloudflare Worker; HMAC; schema validation", output="normalized event"),
            dict(id=3, title="Claim + route", kind="gate", source="S1/S10", trigger="EventBridge -> SQS FIFO", owner="Lifecycle controller", tools="Temporal / Step Functions / LangGraph; lease; retry; DLQ", output="one active run / idempotency"),
            dict(id=4, title="Context pack", kind="agent", source="S2/S6/S9", trigger="Run claimed at exact ticket version", owner="Context agent", tools="GitHub App read token; AGENTS.md; docs; Sourcegraph MCP; catalog", output="repo set + exact SHAs"),
            dict(id=5, title="Dependency map", kind="agent", source="S6", trigger="Context completeness >= policy", owner="Dependency / impact agent", tools="Nx affected; Bazel deps/rdeps; SCIP; CODEOWNERS; SBOM", output="affected repos + DAG"),
            dict(id=6, title="Clarify gate", kind="human", source="S5/S10", trigger="Missing AC/NFR; ambiguity; conflicting docs; risk unknown", owner="Planner asks named questions", tools="Jira/Linear comment; Needs-Info state; LangGraph interrupt", output="resume or refuse"),
            dict(id=7, title="Plan + reviewer", kind="human", source="S1/S2/S3", trigger="Context and dependencies resolved", owner="Planner agent + delivery lead", tools="plan.md; ADR; test plan; risk tier; CODEOWNERS / team map", output="approved plan + reviewer assigned"),
        ],
        [
            dict(id=8, title="Leaf task DAG", kind="agent", source="S1/S2", trigger="Plan approved", owner="Decomposition agent", tools="Requirement -> smallest testable TODO; file scopes; dependency edges", output="parallel-safe task graph"),
            dict(id=9, title="Worktree pool", kind="agent", source="S1/S9", trigger="Unblocked leaf task", owner="Workspace manager", tools="git worktree; K8s Job / ECS Fargate / Codex cloud / Claude web", output="isolated branch + scoped identity"),
            dict(id=10, title="TDD worker loop", kind="agent", source="S2/S5/S9", trigger="Workspace ready", owner="Implementor agent per leaf", tools="TEST: unit / contract / integration\nEDIT: smallest TODO; lint/type; SonarLint; pre-commit", output="green leaf commit + receipt"),
            dict(id=11, title="Integrate", kind="agent", source="S1", trigger="All dependency predecessors green", owner="Integrator agent", tools="topological merge; conflict detector; test after each merge; exact SHA", output="integration branch"),
            dict(id=12, title="Draft PR", kind="event", source="S1/S3", trigger="Integration branch green", owner="PR publisher", tools="gh / GitHub API / GitLab API; ticket + plan + evidence links", output="draft PR + evidence manifest"),
            dict(id=13, title="Independent review", kind="agent", source="S3/S5/S6/S7", trigger="pull_request.opened / synchronize", owner="Separate reviewer model/session", tools="IMPACT: Sourcegraph refs + Nx/Bazel\nMANUAL: Playwright browser, trace, screenshots\nINTENT: spec, scope, unchanged behavior", output="findings or review-ready"),
            dict(id=14, title="Quality firewall", kind="gate", source="S3/S5/S7", trigger="push / PR synchronize", owner="CI controller - no model authority", tools="QUALITY: SonarQube quality gate\nSECURITY: CodeQL/Semgrep; Trivy/Snyk; Gitleaks\nUX: Playwright/axe; Lighthouse CI budgets\nPERF: k6 smoke threshold", output="required checks checked/total"),
        ],
        [
            dict(id=15, title="Human review", kind="human", source="S3/S5", trigger="Agent review clean + every required check green", owner="CODEOWNERS + security / architecture by risk", tools="GitHub review; Slack/Teams notification; exact SHA + evidence hash", output="approve / request changes"),
            dict(id=16, title="Merge queue", kind="gate", source="S3", trigger="Required approvals bound to latest push", owner="Human merge action + protected ruleset", tools="merge queue; signed commits; stale approval dismissal; no bypass", output="main SHA + build provenance"),
            dict(id=17, title="Staging", kind="event", source="S3/S8", trigger="merge_group / main push", owner="Deployment controller", tools="GitHub Environment / Argo CD / CodePipeline; ephemeral or shared stage", output="staging URL + deploy receipt"),
            dict(id=18, title="Pre-prod gates", kind="gate", source="S7/S8", trigger="Staging healthy", owner="Release assurance", tools="E2E: Playwright + Pact\nDAST: OWASP ZAP\nCAPACITY: k6 load / stress / soak\nDATA: migration dry-run + synthetic", output="release evidence bundle"),
            dict(id=19, title="Prod canary", kind="human", source="S3/S8", trigger="Release approval / change window", owner="Release owner + rollout controller", tools="Argo Rollouts / Flagger / CodeDeploy; LaunchDarkly; auto rollback", output="progressive exposure"),
            dict(id=20, title="Observe", kind="gate", source="S8", trigger="Canary step / full rollout", owner="SRE controller", tools="CloudWatch + OpenTelemetry / Datadog / Sentry; SLO + error budget", output="healthy window or alarm"),
            dict(id=21, title="Alert -> repair", kind="agent", source="S8", trigger="CloudWatch Alarm State Change -> EventBridge", owner="Incident triage agent", tools="OBSERVE: logs / metrics / traces + deploy diff\nACT: PagerDuty; rollback policy; remediation ticket", output="diagnosis + ticket; loop to 03"),
        ],
    ]

    descriptors = [
        ("01", "INTAKE + PLAN", "ticket state is the durable control plane"),
        ("02", "BUILD + ASSURE", "parallel workers; independent review; deterministic evidence"),
        ("03", "RELEASE + OPERATE", "protected merge; progressive exposure; incident feedback"),
    ]
    for row_index, row in enumerate(rows):
        section_label(c, M, section_ys[row_index], *descriptors[row_index])
        y = row_ys[row_index]
        for i, spec in enumerate(row):
            x = M + i * (node_w + gap)
            node(c, x, y, node_w, node_h, spec)
            if i < 6:
                arrow(c, [(x + node_w + 2, y + node_h / 2), (x + node_w + gap - 2, y + node_h / 2)], ACCENT, 1.8, 7)

    # Row handoffs
    loop_connector(c, M + 7 * node_w + 6 * gap, row_ys[0], M, row_ys[1] + node_h, "plan approved / reviewer reserved / tasks unblocked")
    loop_connector(c, M + 7 * node_w + 6 * gap, row_ys[1], M, row_ys[2] + node_h, "PR evidence complete / human review requested")

    # Explicit return paths
    arrow(c, [(M + 6 * (node_w + gap) + node_w / 2, row_ys[2]), (M + 6 * (node_w + gap) + node_w / 2, 408), (M + 2 * (node_w + gap) + node_w / 2, 408), (M + 2 * (node_w + gap) + node_w / 2, row_ys[0])], WARN, 1.4, 7)
    text(c, M + 4.4 * (node_w + gap), 417, "PRODUCTION SIGNAL CREATES A NEW VERSIONED TICKET - NEVER A SILENT HOTFIX", "Mono-Bold", 7.8, WARN, "center")

    # Exception rail and invariants
    rail_y = 96
    rail_h = 258
    rail_w = W - 2 * M
    rect(c, M, rail_y, rail_w, rail_h, PAPER, INK, 0.9)
    text(c, M + 18, rail_y + rail_h - 28, "EXCEPTION ROUTES / FAIL CLOSED", "Mono-Bold", 9, WARN)
    line(c, M + 18, rail_y + rail_h - 42, M + rail_w - 18, rail_y + rail_h - 42, INK, 0.8)
    left_w = rail_w * 0.72
    ex = [
        ("E1", "Duplicate / stale event", "Idempotency hit -> acknowledge; newer ticket version cancels the old run."),
        ("E2", "Needs information", "Pause checkpoint -> comment exact questions -> resume only on ticket update."),
        ("E3", "Cross-repo / blocked", "Create child tickets and DAG; dispatch only when dependency receipts exist."),
        ("E4", "Agent timeout / budget", "Checkpoint -> bounded retry -> alternate worker/model -> named human escalation."),
        ("E5", "Test / scan failure", "Return to owning worktree; never disable or reduce a gate without approval."),
        ("E6", "Review / merge conflict", "New leaf task or integrator repair; re-run review and all affected checks."),
        ("E7", "Stage / canary failure", "Stop exposure; rollback last good artifact; create incident and repair ticket."),
        ("E8", "Flaky or zero-input gate", "Quarantine with owner + expiry; checked=0 is failure, never green."),
        ("E9", "Credential / environment fault", "Typed environment receipt; do not report it as an agent failure; retry safely."),
        ("E10", "High-risk change", "Security, tenancy, money, migration, IAM, or policy adds named approval gates."),
        ("E11", "Prompt injection / policy refusal", "Untrusted input stays read-only; controller rejects unauthorized tool or scope requests."),
        ("E12", "Urgent production hotfix", "Rollback first where safe; create urgent ticket; traverse the same evidence path."),
    ]
    cell_w = (left_w - 44) / 4
    for i, item in enumerate(ex):
        col = i % 4
        row = i // 4
        exception_cell(c, M + 18 + col * cell_w, rail_y + rail_h - 66 - row * 61, cell_w - 12, *item)

    inv_x = M + left_w + 20
    inv_w = rail_w - left_w - 20
    rect(c, inv_x, rail_y + 14, inv_w - 14, rail_h - 28, INK, INK, 1)
    text(c, inv_x + 18, rail_y + rail_h - 38, "GLOBAL INVARIANTS", "Mono-Bold", 9, ACCENT)
    invariants = [
        "ticket ID + version + repo SHA = work identity",
        "one controller owns state; agents only propose",
        "untrusted ticket/MCP content cannot grant authority",
        "sandbox: scoped filesystem, egress, branch, secrets",
        "approval binds exact commit + evidence manifest",
        "production writes and exceptions require policy authority",
    ]
    yy = rail_y + rail_h - 66
    for i, value in enumerate(invariants, 1):
        text(c, inv_x + 18, yy, f"{i:02d}", "Mono-Bold", 8, ACCENT)
        paragraph(c, inv_x + 46, yy, value, inv_w - 82, "Display", 8.3, 10.2, PAPER, 2)
        yy -= 27

    # Source register and footer
    text(c, M, 66, "SOURCE REGISTER", "Mono-Bold", 7.5, MUTED)
    sources = "S1 openai.com/index/open-source-codex-orchestration-symphony  /  S2 openai.com/index/harness-engineering  /  S3 docs.github.com/copilot + protected-branches + CODEOWNERS  /  S4 docs.gitlab.com/user/duo_agent_platform + support.atlassian.com/rovo  /  S5 aws.amazon.com/blogs/security/balancing-speed-and-safety-a-control-framework-for-ai-coding-agents  /  S6 sourcegraph.com/docs/code-navigation + nx.dev/docs/features/ci-features/affected + bazel.build/query  /  S7 docs.sonarsource.com pull-request-analysis + playwright.dev/docs/ci-intro + github.com/googlechrome/lighthouse-ci + grafana.com/docs/k6  /  S8 docs.aws.amazon.com CloudWatch/EventBridge/CodeDeploy  /  S9 anthropic.com/engineering/claude-code-sandboxing + git-scm.com/docs/git-worktree  /  S10 langchain-ai.github.io/langgraph"
    paragraph(c, M + 112, 66, sources, W - 2 * M - 112, "Mono", 5.9, 7.3, MUTED, 4)
    line(c, M, 32, W - M, 32, RULE, 0.7)
    text(c, M, 16, "DEVX LABS - THE DEVX DOCTRINE v1.0 - CONFIDENTIAL", "Mono", 7, MUTED)
    text(c, W - M, 16, "REFERENCE ARCHITECTURE - VALIDATE TOOL ENTITLEMENTS AND RISK POLICY", "Mono", 7, MUTED, "right")

    c.showPage()
    c.save()
    print(OUT)


if __name__ == "__main__":
    build()
