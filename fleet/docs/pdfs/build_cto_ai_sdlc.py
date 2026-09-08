from reportlab.pdfgen import canvas
from reportlab.lib.pagesizes import A3, landscape
from reportlab.lib import colors
from reportlab.pdfbase.pdfmetrics import stringWidth
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.lib.colors import HexColor
from reportlab.lib.units import mm
import os

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
OUT = os.path.join(ROOT, "output/pdf/devx-ai-sdlc-cto-operating-model.pdf")
W, H = landscape(A3)
M = 56
TOTAL_PAGES = 9

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

FONT_DIR = "/System/Library/Fonts/Supplemental"
pdfmetrics.registerFont(TTFont("UI", os.path.join(FONT_DIR, "Verdana.ttf")))
pdfmetrics.registerFont(TTFont("UI-Bold", os.path.join(FONT_DIR, "Verdana Bold.ttf")))
pdfmetrics.registerFont(TTFont("UI-Italic", os.path.join(FONT_DIR, "Verdana Italic.ttf")))
pdfmetrics.registerFont(TTFont("UI-BoldItalic", os.path.join(FONT_DIR, "Verdana Bold Italic.ttf")))
pdfmetrics.registerFont(TTFont("Mono", "/System/Library/Fonts/SFNSMono.ttf"))
pdfmetrics.registerFont(TTFont("Mono-Italic", "/System/Library/Fonts/SFNSMonoItalic.ttf"))
def t(c, x, y, s, font="UI", size=10, color=INK, align="left"):
    c.setFillColor(color)
    c.setFont(font, size)
    if align == "right":
        c.drawRightString(x, y, s)
    elif align == "center":
        c.drawCentredString(x, y, s)
    else:
        c.drawString(x, y, s)


def wrap(text, font, size, width):
    words = text.split()
    lines, line = [], ""
    for word in words:
        trial = word if not line else line + " " + word
        if stringWidth(trial, font, size) <= width:
            line = trial
        else:
            if line:
                lines.append(line)
            line = word
    if line:
        lines.append(line)
    return lines


def para(c, x, y, text, width, font="UI", size=10, leading=14, color=INK2, max_lines=None):
    lines = wrap(text, font, size, width)
    if max_lines:
        lines = lines[:max_lines]
    for i, line in enumerate(lines):
        t(c, x, y - i * leading, line, font, size, color)
    return y - len(lines) * leading


def rule(c, x1, y, x2, color=RULE, width=0.7):
    c.setStrokeColor(color)
    c.setLineWidth(width)
    c.line(x1, y, x2, y)


def eyebrow(c, x, y, text, color=ACCENT):
    t(c, x, y, text.upper(), "Mono", 9, color)


def title(c, page, section, head, sub=None):
    eyebrow(c, M, H - M - 2, f"{section} / CTO OPERATING MODEL")
    t(c, W - M, H - M - 2, f"{page:02d} · {TOTAL_PAGES:02d}", "Mono", 9, MUTED, "right")
    rule(c, M, H - M - 22, W - M, INK, 1)
    t(c, M, H - M - 76, head, "UI-Bold", 34, INK)
    if sub:
        para(c, M, H - M - 104, sub, W - 2 * M, "UI", 12, 17, MUTED)


def footer(c, page):
    rule(c, M, 34, W - M, RULE, 0.6)
    t(c, M, 18, "DEVX LABS · THE DEVX DOCTRINE v1.0 · CONFIDENTIAL", "Mono", 8, MUTED)
    t(c, W - M, 18, f"{page:02d} / {TOTAL_PAGES:02d}", "Mono", 8, MUTED, "right")


def mixed_headline(c, x, y, parts, size=52):
    cursor = x
    for text_value, font, color in parts:
        t(c, cursor, y, text_value, font, size, color)
        cursor += stringWidth(text_value, font, size)


def box(c, x, y, w, h, fill=PAPER, stroke=RULE, sw=0.8):
    c.setFillColor(fill)
    c.setStrokeColor(stroke)
    c.setLineWidth(sw)
    c.rect(x, y, w, h, fill=1, stroke=1)


def arrow(c, x1, y1, x2, y2, color=ACCENT, width=1.2, head=6):
    c.setStrokeColor(color)
    c.setFillColor(color)
    c.setLineWidth(width)
    c.line(x1, y1, x2, y2)
    import math
    a = math.atan2(y2 - y1, x2 - x1)
    p1 = (x2 - head * math.cos(a - 0.5), y2 - head * math.sin(a - 0.5))
    p2 = (x2 - head * math.cos(a + 0.5), y2 - head * math.sin(a + 0.5))
    path = c.beginPath()
    path.moveTo(x2, y2)
    path.lineTo(*p1)
    path.lineTo(*p2)
    path.close()
    c.drawPath(path, fill=1, stroke=0)


def node(c, x, y, w, h, state, owner, artifact, gate=False, n=""):
    fill = ACCENT_SOFT if owner == "AGENT" else PAPER2 if owner == "CONTROLLER" else PAPER
    box(c, x, y, w, h, fill, INK if gate else RULE, 1 if gate else 0.8)
    c.setFillColor(ACCENT if owner == "AGENT" else INK if owner == "CONTROLLER" else WARN if gate else OK)
    c.rect(x, y, 5, h, fill=1, stroke=0)
    t(c, x + 16, y + h - 18, n, "Mono", 8, MUTED)
    lines = state.split(" / ")
    for i, line in enumerate(lines):
        t(c, x + 16, y + h - 38 - i * 14, line, "UI-Bold", 10, INK)
    t(c, x + 16, y + 18, owner, "Mono", 7.5, ACCENT if owner == "AGENT" else MUTED)
    t(c, x + w - 12, y + 18, artifact, "Mono", 7, MUTED, "right")


def stat_cell(c, x, y, w, value, label):
    t(c, x, y, value, "UI", 34, INK)
    t(c, x, y - 20, label.upper(), "Mono", 8, MUTED)


def bullet(c, x, y, label, body, width):
    c.setFillColor(ACCENT)
    c.rect(x, y + 4, 24, 2, fill=1, stroke=0)
    t(c, x + 36, y, label, "UI-Bold", 10, INK)
    return para(c, x + 36, y - 16, body, width - 36, "UI", 9, 13, MUTED)


def page_cover(c):
    c.setFillColor(PAPER)
    c.rect(0, 0, W, H, fill=1, stroke=0)
    eyebrow(c, M, H - M, "DEVX LABS / CTO OPERATING MODEL")
    t(c, W - M, H - M, f"01 · {TOTAL_PAGES:02d}", "Mono", 9, MUTED, "right")
    mixed_headline(c, M, H - 178, [("The ", "UI-Bold", INK), ("AI-native", "Times-Italic", ACCENT), (" delivery system", "UI-Bold", INK)], 56)
    para(c, M, H - 224, "A governed lifecycle from client intent to production evidence. Designed for delivery teams that use agents at machine speed without surrendering judgment, accountability, or standards.", 620, "UI", 16, 24, MUTED)
    rule(c, M, H - 312, M + 96, ACCENT, 2)
    t(c, M, H - 348, "EXACT PROCESS MAP", "Mono", 10, ACCENT)
    para(c, M, H - 374, "The graph in this document is the operating contract: each state has an owner, an entry trigger, an agent action, an evidence requirement, and an explicit authority to advance or refuse.", 440, "UI", 12, 18, INK2)

    box(c, W - M - 390, 92, 390, 218, INK, INK, 1)
    t(c, W - M - 358, 270, "THE OPERATING THESIS", "Mono", 9, ACCENT)
    para(c, W - M - 358, 232, "Humans set direction. Agents create and test changes. Controllers verify evidence. Protected systems decide what can move forward.", 320, "Times-Italic", 21, 26, PAPER)
    rule(c, W - M - 358, 140, W - M - 32, HexColor("#3A3A3A"), 0.7)
    t(c, W - M - 358, 116, "DOCTRINE v1.0 · APRIL 2026", "Mono", 8, MUTED2)

    y = 52
    t(c, M, y, "CLIENT DELIVERY / AGENTIC SDLC / CTO EDITION", "Mono", 8, MUTED)
    t(c, W - M, y, "25 AUG 2026", "Mono", 8, MUTED, "right")
    c.showPage()


def page_thesis(c):
    title(c, 2, "00", "A system of contracts, not prompts", "The lifecycle is governed by three planes. The agent is never the source of truth; it is a worker operating inside a versioned control system.")
    y = H - 184
    cols = [(M, "CONTROL PLANE", "The durable state machine", "Client records, PRD, SOW, backlog, ticket states, policies, approvals, evidence ledger, and release authority."),
            (M + 346, "EXECUTION PLANE", "The bounded worker", "Specialist agents, isolated workspaces, GitHub/GitLab tools, tests, CI, browsers, deployment runners, and incident tools."),
            (M + 692, "ASSURANCE PLANE", "The quality firewall", "Deterministic tests, security scans, architectural checks, independent review, protected branches, canaries, and telemetry.")]
    for x, label, head, body in cols:
        box(c, x, y - 240, 300, 240, PAPER2)
        eyebrow(c, x + 24, y - 28, label)
        t(c, x + 24, y - 76, head, "UI-Bold", 18, INK)
        para(c, x + 24, y - 112, body, 250, "UI", 11, 16, MUTED)
        rule(c, x + 24, y - 196, x + 276, RULE, 0.7)
        t(c, x + 24, y - 220, "SOURCE OF AUTHORITY", "Mono", 8, MUTED)
    
    t(c, M, 370, "WHAT IS AUTOMATED", "Mono", 10, ACCENT)
    t(c, M + 370, 370, "WHAT REMAINS HUMAN", "Mono", 10, WARN)
    rule(c, M, 356, M + 310, INK, 1)
    rule(c, M + 370, 356, M + 680, INK, 1)
    left = ["Discovery synthesis and ambiguity extraction", "PRD, story, task, and test-plan drafting", "Repository navigation and implementation", "Test generation, CI repair, documentation", "Review preparation and standards-drift detection"]
    right = ["Problem selection and client truth", "SOW scope, price, dates, and legal terms", "Architecture and high-blast-radius decisions", "Security, privacy, tenancy, money, migrations", "Final release, rollback, and exceptions"]
    yy = 328
    for a, b in zip(left, right):
        yy = bullet(c, M, yy, a, "Bounded by the current state, repository policy, and explicit acceptance criteria.", 310) - 8
        bullet(c, M + 370, yy + 8, b, "Accountability cannot be delegated to model completion prose.", 310)
    box(c, M + 760, 118, 318, 252, INK, INK, 1)
    t(c, M + 790, 338, "THE CTO RULE", "Mono", 9, ACCENT)
    para(c, M + 790, 298, "Let agents propose evidence and changes. Let deterministic systems verify them. Let humans approve irreversible boundaries.", 258, "Times-Italic", 20, 25, PAPER)
    c.showPage()


def page_graph(c):
    title(c, 3, "01", "The AI-native SDLC: one control-flow graph", "The master LLD. Read left to right: each column names the state, trigger hook, responsible agent or human, concrete tool surface, output artifact, and gate that permits the next state.")
    x0 = M
    y0 = 118
    col_gap = 10
    col_w = (W - 2*M - 7*col_gap) / 8
    top = H - 230
    lane_h = 142
    lane_gap = 8
    lane_ys = [top - lane_h, top - 2*lane_h - lane_gap, top - 3*lane_h - 2*lane_gap]
    lane_names = ["HUMAN AUTHORITY / STATE", "AGENT / CONTROLLER / TOOL", "HOOK / ARTIFACT / EXIT GATE"]
    lane_colors = [PAPER, ACCENT_SOFT, PAPER2]
    for ly, ln, fill in zip(lane_ys, lane_names, lane_colors):
        box(c, x0, ly, W-2*M, lane_h, fill, RULE, 0.6)
        c.setFillColor(INK if fill == PAPER else ACCENT if fill == ACCENT_SOFT else MUTED)
        c.rect(x0, ly, 6, lane_h, fill=1, stroke=0)
        t(c, x0 + 16, ly + lane_h - 18, ln, "Mono", 7.3, MUTED)

    stages = [
        ("01 / INTAKE", "Client sponsor", "problem + constraints", "intake agent", "CRM / transcript", "discovery packet", "issue.created"),
        ("02 / DISCOVER", "PO + client", "truth / unknowns", "research + product agent", "Claude / Gemini / MCP", "locked discovery", "approve / clarify"),
        ("03 / CONTRACT", "PO + CTO", "outcome / price / date", "PRD-SOW drafter", "Rovo / AI-DLC", "versioned PRD + SOW", "signed scope"),
        ("04 / DESIGN", "architect", "risk / architecture", "planner + threat agent", "ADR / CodeQL rules", "DAG + ADR + NFR", "architecture gate"),
        ("05 / ASSIGN", "delivery lead", "owner / capacity", "lifecycle controller", "Linear / Jira / GitHub App", "ticket + identity", "assign / @mention"),
        ("06 / BUILD", "delivery lead", "ambiguity only", "coding agent", "Codex / Claude / Copilot / Jules", "branch + commits", "push / PR.opened"),
        ("07 / ASSURE", "reviewers", "risk acceptance", "test + review agents", "Actions / CI / SAST / CODEOWNERS", "evidence bundle", "checks + review"),
        ("08 / OPERATE", "release owner", "exposure / rollback", "deploy + SRE agents", "protected branch / canary / SLO", "receipt + learning", "merge / alert / close"),
    ]
    for i, stage in enumerate(stages):
        x = x0 + i * (col_w + col_gap)
        # stage header aligned above the three swimlanes
        box(c, x, top + 12, col_w, 34, INK, INK, 0.8)
        t(c, x + 9, top + 25, stage[0], "Mono", 7.2, PAPER)
        # human lane
        t(c, x + 11, lane_ys[0] + lane_h - 42, stage[1], "UI-Bold", 9, INK)
        para(c, x + 11, lane_ys[0] + lane_h - 64, stage[2], col_w - 22, "UI", 8.3, 11, MUTED, 3)
        t(c, x + 11, lane_ys[0] + 15, "HUMAN DECISION", "Mono", 6.7, WARN)
        # agent lane
        t(c, x + 11, lane_ys[1] + lane_h - 42, stage[3], "UI-Bold", 8.8, ACCENT)
        para(c, x + 11, lane_ys[1] + lane_h - 64, stage[4], col_w - 22, "Mono", 7.2, 9.5, INK2, 4)
        t(c, x + 11, lane_ys[1] + 15, "WORKER / ROUTER", "Mono", 6.7, ACCENT)
        # hook / artifact lane
        t(c, x + 11, lane_ys[2] + lane_h - 42, stage[5], "UI-Bold", 8.4, INK)
        para(c, x + 11, lane_ys[2] + lane_h - 64, stage[6], col_w - 22, "Mono", 7.1, 9.5, MUTED, 4)
        t(c, x + 11, lane_ys[2] + 15, "EXIT / NEXT TRIGGER", "Mono", 6.7, OK)
        if i < 7:
            arrow(c, x + col_w + 2, lane_ys[1] + lane_h/2, x + col_w + col_gap - 2, lane_ys[1] + lane_h/2, ACCENT, 1.1, 5)

    # deterministic and human control rails
    rail_y = 82
    box(c, M, rail_y, W-2*M, 24, INK, INK, 0.8)
    t(c, M+12, rail_y+8, "GLOBAL CONTROL RAIL", "Mono", 7.3, ACCENT)
    t(c, M+172, rail_y+8, "repo policy: AGENTS.md / CLAUDE.md / WORKFLOW.md", "Mono", 7.2, PAPER)
    t(c, M+520, rail_y+8, "runtime: sandbox + scoped identity + MCP/A2A", "Mono", 7.2, PAPER)
    t(c, M+848, rail_y+8, "merge authority: protected branch", "Mono", 7.2, PAPER)
    # exception line
    t(c, M, 54, "LOOPS", "Mono", 7.5, WARN)
    para(c, M+58, 54, "ambiguity -> human clarify  |  CI/review finding -> agent repair  |  deploy/SLO failure -> rollback or incident ticket  |  repeated failure -> rule, test, skill, or eval", W-2*M-58, "UI", 7.8, 10, MUTED, 2)
    footer(c, 3)
    c.showPage()


def page_contracts(c):
    title(c, 4, "02", "Every state has a contract", "The state machine is only useful if each transition is testable. The table below is the minimum contract for a client delivery lane.")
    cols = [M, M+150, M+315, M+540, M+788, W-M]
    headers = ["STATE", "ENTRY TRIGGER", "AGENT WORK", "HUMAN AUTHORITY", "EXIT EVIDENCE"]
    y_top = H - 182
    widths = [150, 165, 225, 248, W-M-cols[4]]
    rule(c, M, y_top + 16, W-M, INK, 1)
    for i, h in enumerate(headers):
        t(c, cols[i], y_top, h, "Mono", 8, MUTED)
    rows = [
        ("DRAFT", "Client input / lead", "Extract facts, unknowns, risks", "Choose problem and sponsor", "Discovery packet"),
        ("DISCOVERY LOCKED", "Facts reviewed", "Normalize goals and constraints", "Confirm client truth", "Signed discovery record"),
        ("PRD REFINED", "Discovery complete", "Draft requirements, AC, NFRs", "Approve outcome and scope", "Versioned PRD"),
        ("SOW APPROVED", "PRD accepted", "Draft deliverables and assumptions", "Approve price, dates, exclusions", "Signed SOW"),
        ("ARCHITECTURE APPROVED", "SOW accepted", "Propose architecture, ADR, threat model", "Approve risk and design", "Approved architecture packet"),
        ("BACKLOG READY", "Architecture accepted", "Explode work into DAG", "Prioritize and size", "Linked epics / tasks"),
        ("ASSIGNED", "Ticket unblocked", "Route by repo, domain, risk", "Approve team/capacity exceptions", "Owner + agent identity"),
        ("IMPLEMENTING", "Assignment claimed", "Plan, edit, test, commit", "Steer ambiguity", "Branch + session log"),
        ("VERIFYING", "Commit pushed", "Run declared checks and repair", "Accept evidence for risk tier", "Checks with checked/total"),
        ("PR READY", "Checks green", "Summarize diff and traceability", "Select reviewers", "PR + evidence bundle"),
        ("HUMAN APPROVED", "Review complete", "Address comments", "Approve exact commit", "Required approvals"),
        ("CANARY", "Merge / deploy", "Observe health and rollback signals", "Authorize exposure", "Canary telemetry"),
        ("RELEASED", "Canary passes", "Publish notes and update records", "Release owner confirms", "Deployment receipt"),
        ("OBSERVING", "Production live", "Triage signals and drift", "Decide rollback / follow-up", "SLO and incident evidence"),
        ("CLOSED", "Outcome stable", "Update standards and evals", "Confirm acceptance and learnings", "Closure ledger"),
    ]
    y = y_top - 24
    row_h = 34
    for idx, row in enumerate(rows):
        if idx % 2 == 0:
            c.setFillColor(PAPER2)
            c.rect(M, y-row_h+7, W-2*M, row_h, fill=1, stroke=0)
        for i, val in enumerate(row):
            font = "UI-Bold" if i == 0 else "UI"
            color = INK if i == 0 else MUTED
            para(c, cols[i], y, val, widths[i]-12, font, 8.5 if i == 0 else 8, 10, color, 3)
        rule(c, M, y-row_h+5, W-M, RULE2, 0.5)
        y -= row_h
    t(c, M, 52, "CONTROL NOTE", "Mono", 8, ACCENT)
    para(c, M, 36, "A model saying 'done' never advances a state. The controller advances only when the evidence contract is present and valid.", 720, "Times-Italic", 14, 18, INK2)
    footer(c, 4)
    c.showPage()


def page_topology(c):
    title(c, 5, "03", "The agent topology", "Specialists are modular. The controller is singular. Human roles remain outside the worker pool and own the decision boundaries.")
    cx, cy = W/2, 410
    box(c, cx-105, cy-62, 210, 124, INK, INK, 1)
    t(c, cx, cy+28, "LIFECYCLE CONTROLLER", "Mono", 9, ACCENT, "center")
    t(c, cx, cy-10, "State + evidence", "UI-Bold", 18, PAPER, "center")
    t(c, cx, cy-36, "dispatch · retry · reconcile", "Mono", 8, MUTED2, "center")
    specialists = [
        (M, 525, "CLIENT INTELLIGENCE", ["Discovery", "Requirements", "SOW draft"], "Client facts and scope"),
        (M, 270, "PRODUCT PLANNING", ["Planner", "Dependency", "Assignment"], "Backlog and DAG"),
        (W-M-280, 525, "DELIVERY WORKERS", ["Architecture", "Coding", "Test / docs"], "Branch and PR"),
        (W-M-280, 270, "ASSURANCE WORKERS", ["Security", "Review", "Acceptance"], "Checks and findings"),
        (cx-140, 110, "OPERATIONS", ["Release", "SRE", "Learning"], "Telemetry and closure"),
    ]
    for x, y, label, items, artifact in specialists:
        box(c, x, y, 280, 136, PAPER2, RULE, 0.8)
        eyebrow(c, x+20, y+110, label)
        for i, item in enumerate(items):
            t(c, x+20, y+78-i*18, item, "UI-Bold", 11, INK)
        t(c, x+20, y+18, artifact.upper(), "Mono", 7.5, MUTED)
        if x < cx and y > cy:
            tx, ty = cx - 105, cy + 45
        elif x > cx and y > cy:
            tx, ty = cx + 105, cy + 45
        elif x < cx and y < cy:
            tx, ty = cx - 105, cy - 45
        elif x > cx and y < cy:
            tx, ty = cx + 105, cy - 45
        else:
            tx, ty = cx, cy - 62
        arrow(c, x+140, y+68, tx, ty, ACCENT, 0.9, 5)
    
    t(c, M, 78, "HUMAN ROLES / AUTHORITY", "Mono", 9, WARN)
    rule(c, M, 66, W-M, INK, 1)
    roles = ["CLIENT SPONSOR", "PRODUCT OWNER", "SOLUTION ARCHITECT", "DELIVERY LEAD", "SECURITY / SRE", "RELEASE OWNER"]
    role_w = (W-2*M) / len(roles)
    for i, role in enumerate(roles):
        x = M + i*role_w
        if i:
            c.setStrokeColor(RULE)
            c.line(x, 34, x, 66)
        t(c, x+12, 48, role, "Mono", 7.5, MUTED)
    footer(c, 5)
    c.showPage()


def page_github(c):
    title(c, 6, "04", "From ticket to protected merge", "The integration is an event-driven chain. Every external action is scoped, attributable, replayable, and reversible.")
    y = 430
    steps = [
        ("01", "ISSUE EVENT", "assign / label / mention", "Ticket ID + actor"),
        ("02", "CLAIM", "controller locks work", "Idempotency receipt"),
        ("03", "WORKSPACE", "ephemeral branch / runner", "Scoped identity"),
        ("04", "AGENT LOOP", "plan / edit / test", "Session log"),
        ("05", "PULL REQUEST", "draft PR + traceability", "Commit SHA"),
        ("06", "QUALITY FIREWALL", "CI + SAST + secret + policy", "Checks checked/total"),
        ("07", "REVIEW", "agent comments + CODEOWNERS", "Approvals"),
        ("08", "MERGE / CANARY", "protected branch + rollout", "Deployment receipt"),
    ]
    sw = (W-2*M-7*18)/8
    for i, (num, head, body, evid) in enumerate(steps):
        x = M + i*(sw+18)
        box(c, x, y, sw, 152, PAPER2 if i not in (5,7) else ACCENT_SOFT, RULE, 0.8)
        t(c, x+16, y+126, num, "Mono", 8, ACCENT)
        t(c, x+16, y+96, head, "UI-Bold", 11, INK)
        para(c, x+16, y+70, body, sw-32, "UI", 9, 12, MUTED)
        rule(c, x+16, y+30, x+sw-16, RULE, 0.5)
        t(c, x+16, y+14, evid, "Mono", 7, MUTED)
        if i < 7:
            arrow(c, x+sw+3, y+76, x+sw+15, y+76, ACCENT, 1, 5)
    box(c, M, 158, W-2*M, 176, PAPER3, RULE2, 0.8)
    eyebrow(c, M+24, 304, "IDENTITY AND EVENT CONTRACT")
    items = [
        ("Input", "Issue content is untrusted data; the controller separates orchestration from untrusted context."),
        ("Credentials", "Use GitHub App or proxy-scoped credentials; never expose personal tokens or production secrets to the worker."),
        ("Hooks", "Use source-control events for lifecycle triggers and runtime hooks for tests, lint, policy, and secret checks."),
        ("Review", "AI review is advisory; required approvals and protected-branch rules remain the merge authority."),
    ]
    for i, (lab, body) in enumerate(items):
        x = M + 24 + (i % 2)*490
        yy = 270 - (i//2)*64
        t(c, x, yy, lab.upper(), "Mono", 8, ACCENT)
        para(c, x, yy-16, body, 420, "UI", 9, 13, INK2)
    t(c, M, 110, "FAILURE MODEL", "Mono", 9, WARN)
    para(c, M, 90, "Failure -> typed receipt -> retry or refusal -> human escalation. A zero-input check is a failure, not a green result.", W-2*M, "Times-Italic", 16, 20, INK2)
    footer(c, 6)
    c.showPage()


def page_standards(c):
    title(c, 7, "05", "Standards become executable", "The design doctrine applies to the operating system too: one accent, explicit hierarchy, sharp boundaries, and no hidden authority.")
    # left matrix
    x0, y0, colw = M, 450, 500
    eyebrow(c, x0, y0+170, "QUALITY FIREWALL")
    rule(c, x0, y0+156, x0+colw, INK, 1)
    matrix = [
        ("Repository", "AGENTS.md / CLAUDE.md / WORKFLOW.md", "Versioned working contract"),
        ("Architecture", "ADR, dependency rules, structural tests", "No silent drift"),
        ("Code", "Lint, type, schema, contract, migration checks", "Deterministic correctness"),
        ("Security", "SAST, SCA, secrets, IaC, threat model", "Risk before merge"),
        ("Review", "Independent agent + CODEOWNERS + human", "Separation of duties"),
        ("Runtime", "Sandbox, egress allowlist, scoped identity", "Bounded execution"),
        ("Learning", "Incident -> test / rule / skill / eval", "Failure compounds into control"),
    ]
    y = y0+130
    for i, (a,b,d) in enumerate(matrix):
        if i % 2 == 0:
            c.setFillColor(PAPER2); c.rect(x0, y-18, colw, 30, fill=1, stroke=0)
        t(c, x0+12, y, a, "UI-Bold", 9, INK)
        t(c, x0+120, y, b, "Mono", 7.5, MUTED)
        t(c, x0+342, y, d, "UI", 8.5, MUTED)
        rule(c, x0, y-18, x0+colw, RULE2, 0.5)
        y -= 30
    # right evidence ledger
    x1 = M + 548
    eyebrow(c, x1, y0+170, "EVIDENCE RECORD")
    rule(c, x1, y0+156, W-M, INK, 1)
    box(c, x1, y0-42, W-M-x1, 180, INK, INK, 1)
    ledger = [
        ("work_id", "client-042 / feature-019"),
        ("state", "VERIFYING"),
        ("actor", "agent:coder-v3"),
        ("commit", "sha256:..."),
        ("checks", "checked=42 / total=42"),
        ("approvals", "security + codeowner"),
        ("decision", "controller: advance"),
    ]
    yy = y0+112
    for k,v in ledger:
        t(c, x1+22, yy, k, "Mono", 8, ACCENT)
        t(c, x1+170, yy, v, "Mono", 8.5, PAPER)
        yy -= 21
    t(c, M, 180, "RISK-TIERED AUTONOMY", "Mono", 9, ACCENT)
    rule(c, M, 166, W-M, INK, 1)
    tiers = [
        ("T0 / LOW", "Docs, formatting, isolated tests", "Auto-run; deterministic checks; sampled human review"),
        ("T1 / STANDARD", "Product code, ordinary refactors", "Agent PR; CI; independent review; human merge"),
        ("T2 / HIGH", "Auth, tenancy, data, dependencies", "Architecture + security + human approval"),
        ("T3 / IRREVERSIBLE", "Money, migrations, production, policy", "Explicit human authorization at exact commit/evidence"),
    ]
    tw = (W-2*M)/4
    for i,(a,b,d) in enumerate(tiers):
        x = M+i*tw
        box(c, x, 44, tw-16, 94, PAPER2, RULE, 0.8)
        t(c, x+14, 116, a, "Mono", 8, WARN if i>=2 else ACCENT)
        para(c, x+14, 94, b, tw-44, "UI-Bold", 9, 12, INK)
        para(c, x+14, 58, d, tw-44, "UI", 8, 11, MUTED)
    footer(c, 7)
    c.showPage()


def page_rules(c):
    title(c, 8, "06", "CTO operating rules", "A compact policy for adopting this model without confusing autonomy with authority.")
    rules = [
        ("01", "Make the ticket the durable unit", "Sessions disappear. Work state, evidence, and ownership must survive the agent."),
        ("02", "Make requirements testable", "Every accepted requirement maps to a check, a reviewer, or an explicit human decision."),
        ("03", "Keep SOW and PRD separate", "The PRD describes product truth. The SOW describes contractual truth. They must link, not blur."),
        ("04", "Use specialists plus one controller", "Separate discovery, planning, coding, assurance, and operations workers; keep state authority singular."),
        ("05", "Treat external content as untrusted", "Tickets, docs, webpages, and MCP results are inputs, not instructions with authority."),
        ("06", "Never let prose advance a gate", "Only validated evidence can advance state; absent evidence is absent, never zero."),
        ("07", "Review the exact commit", "Approval attaches to a commit SHA and its evidence bundle, not to a moving branch."),
        ("08", "Let agents repair cheap failures", "CI failures and review comments should loop back to the worker automatically."),
        ("09", "Escalate irreversible actions", "Production, security, money, tenancy, migrations, and policy require named human authority."),
        ("10", "Turn failure into a rule", "Every repeated failure becomes a test, lint rule, skill, runbook, or evaluation."),
    ]
    x, y = M, H-198
    for i,(num,head,body) in enumerate(rules):
        col = i % 2
        row = i // 2
        xx = M + col*520
        yy = y - row*92
        t(c, xx, yy, num, "Mono", 9, ACCENT)
        t(c, xx+44, yy, head, "UI-Bold", 14, INK)
        para(c, xx+44, yy-22, body, 430, "UI", 10, 14, MUTED)
        rule(c, xx, yy-54, xx+480, RULE, 0.7)
    box(c, M, 74, W-2*M, 116, INK, INK, 1)
    t(c, M+28, 160, "DECISION", "Mono", 9, ACCENT)
    para(c, M+28, 130, "Adopt the model where the cost of context switching is high and the cost of verification is low. Keep humans close to ambiguity, risk, and irreversible change.", W-2*M-56, "Times-Italic", 22, 27, PAPER)
    t(c, M+28, 92, "THE DEVX DOCTRINE · CTO EDITION", "Mono", 8, MUTED2)
    footer(c, 8)
    c.showPage()


def page_research(c):
    title(c, 9, "07", "Research register: what to assemble", "The graph is a CTO synthesis. The implementation primitives below are documented platform capabilities or company-reported practices; validate entitlements, security posture, and current product limits before rollout.")
    x0, y0 = M, H - 180
    cols = [M, M + 350, M + 700]
    blocks = [
        ("CONTROL PLANE", [
            ("GitHub Copilot agents", "Issues -> sessions -> PRs; agent review is advisory; branch protection and CODEOWNERS remain authoritative."),
            ("GitLab Duo Agent Platform", "Planner, Developer, Code Review, CI/CD and custom flows; Mention/Assign triggers; audit events."),
            ("Atlassian Rovo", "Natural language -> work items; agents can be assigned, mentioned, and connected through A2A."),
            ("Linear + Symphony", "Issue status is the scheduler; isolated workspaces, DAG dependencies, retries, and WORKFLOW.md."),
        ]),
        ("EXECUTION SURFACE", [
            ("OpenAI Codex / App Server", "Common harness protocol for CLI, IDE and web; tools, approvals, threads, turns, and workspace isolation."),
            ("Claude Code / Agent SDK", "CLAUDE.md, hooks, subagents, MCP, checkpoints, sandbox and scoped Git proxy."),
            ("AWS Kiro / AI-DLC", "Adaptive Inception, Construction and Operations; specs and steering files; approval at phase boundaries."),
            ("Google Jules / Gemini", "Async cloud VM, plan before edit, GitHub issue integration, PR delivery, scheduled and failure-repair tasks."),
        ]),
        ("PROTOCOLS + FRAMEWORKS", [
            ("MCP", "Standard tool/context boundary; keep servers allowlisted, credentials scoped, and external content untrusted."),
            ("A2A", "Agent-to-agent discovery and task handoff; use authenticated agent cards and explicit task ownership."),
            ("SDK choices", "OpenAI Agents SDK, Google ADK, LangGraph, CrewAI, AutoGen/AG2, PydanticAI, OpenHands/SWE-agent."),
            ("Repository contracts", "AGENTS.md, CLAUDE.md, WORKFLOW.md, Kiro steering, CODEOWNERS, required checks, protected branches."),
        ]),
    ]
    for x, (head, rows) in zip(cols, blocks):
        box(c, x, y0 - 310, 320, 310, PAPER2, RULE, 0.8)
        eyebrow(c, x + 18, y0 - 26, head)
        rule(c, x + 18, y0 - 40, x + 302, INK, 0.8)
        yy = y0 - 64
        for label, body in rows:
            t(c, x + 18, yy, label, "UI-Bold", 9, INK)
            yy = para(c, x + 18, yy - 15, body, 284, "UI", 8, 10.5, MUTED, 4) - 17
            rule(c, x + 18, yy + 8, x + 302, RULE2, 0.5)
    box(c, M, 94, W - 2*M, 126, INK, INK, 1)
    t(c, M + 24, 194, "METHODOLOGIES OBSERVED", "Mono", 8.5, ACCENT)
    methods = [
        ("AI-DLC", "adaptive phases, approval checkpoints, audit trail"),
        ("Harness engineering", "repo as system of record, executable invariants, agent-first work"),
        ("Spec-driven", "PRD/spec -> tasks -> implementation -> verification"),
        ("TDD + repair loops", "tests first, hooks/CI feedback, bounded autonomous repair"),
        ("Evidence-bound Company OS", "state machine, risk tiers, receipts, learning controls"),
    ]
    mw = (W - 2*M - 48) / 5
    for i, (label, body) in enumerate(methods):
        x = M + 24 + i * mw
        t(c, x, 164, label, "UI-Bold", 9.5, PAPER)
        para(c, x, 146, body, mw - 22, "UI", 8, 10.5, MUTED2, 4)
    t(c, M, 68, "PRIMARY SOURCES", "Mono", 8, MUTED)
    para(c, M + 112, 68, "openai.com/index/harness-engineering  |  openai.com/index/open-source-codex-orchestration-symphony  |  docs.github.com/copilot/agents  |  docs.gitlab.com/user/duo_agent_platform  |  aws.amazon.com/blogs/devops/open-sourcing-adaptive-workflows-for-ai-driven-development-life-cycle-ai-dlc  |  claude.com/blog/how-anthropic-teams-use-claude-code  |  blog.google/innovation-and-ai/models-and-research/google-labs/jules  |  modelcontextprotocol.io  |  a2a-protocol.org", W - 2*M - 112, "Mono", 6.6, 8.2, MUTED, 3)
    footer(c, 9)
    c.showPage()


def add_sources(c):
    # This is intentionally a compact source note on the final page's metadata area.
    pass


def build():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    c = canvas.Canvas(OUT, pagesize=(W,H), pageCompression=1)
    c.setTitle("DevX Labs - AI-native SDLC CTO Operating Model")
    c.setAuthor("DevX Labs")
    page_cover(c)
    page_thesis(c)
    page_graph(c)
    page_contracts(c)
    page_topology(c)
    page_github(c)
    page_standards(c)
    page_rules(c)
    page_research(c)
    c.save()
    print(OUT)


if __name__ == "__main__":
    build()
