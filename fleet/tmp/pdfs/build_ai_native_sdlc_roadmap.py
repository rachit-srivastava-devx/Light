from reportlab.pdfgen import canvas
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfbase.pdfmetrics import stringWidth
from reportlab.lib.colors import HexColor
import math
import os


ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
OUT = os.path.join(ROOT, "output/pdf/devx-ai-native-sdlc-delivery-roadmap.pdf")
W, H = 1280, 760
M = 48

INK = HexColor("#0A0A0A")
INK2 = HexColor("#1A1A1A")
MUTED = HexColor("#5C6066")
MUTED2 = HexColor("#8A8F96")
RULE = HexColor("#E5E5E5")
PAPER = HexColor("#FFFFFF")
PAPER2 = HexColor("#FAFAF8")
PAPER3 = HexColor("#F4F4F1")
BLUE = HexColor("#1E6FFF")
BLUE2 = HexColor("#E8F0FF")
RED = HexColor("#C0392B")
RED2 = HexColor("#FBECE9")
GREEN = HexColor("#0A7C53")
GREEN2 = HexColor("#E8F4EF")

FONT_ROOT = "/Users/rachitsrivastava/.agents/skills/canvas-design/canvas-fonts"
pdfmetrics.registerFont(TTFont("Display", os.path.join(FONT_ROOT, "InstrumentSans-Regular.ttf")))
pdfmetrics.registerFont(TTFont("Display-Bold", os.path.join(FONT_ROOT, "InstrumentSans-Bold.ttf")))
pdfmetrics.registerFont(TTFont("Editorial", os.path.join(FONT_ROOT, "IBMPlexSerif-Italic.ttf")))
pdfmetrics.registerFont(TTFont("Mono", os.path.join(FONT_ROOT, "JetBrainsMono-Regular.ttf")))
pdfmetrics.registerFont(TTFont("Mono-Bold", os.path.join(FONT_ROOT, "JetBrainsMono-Bold.ttf")))


PHASES = [
    {
        "id": "P0", "name": "Mobilize and baseline", "start": 0, "end": 1,
        "outcome": "One agreed estate, risk model and pilot boundary.",
        "build": [
            ("Estate manifest", "30 repos, 100+ services, owners, languages, contracts, data classes, environments, pipelines and SLOs."),
            ("Baseline", "DORA, change failure, escaped defects, MTTR, CI p95, cloud cost, security backlog and toil."),
            ("Governance", "State model, R0-R3 authority, threat model, data retention, pilot cohort and architecture decisions."),
        ],
        "prove": [
            ("Coverage", "checked == total for repos, services and production owners; unknowns have named closure dates."),
            ("Pilot fitness", "2-3 services include one API, one data change and one real dependency boundary."),
            ("Decision", "CTO, CISO and platform owners sign scope, no-go rules, staffing and evidence contract."),
        ],
        "unlock": [
            ("Control-plane build", "Connector and evidence work can start against a stable inventory."),
            ("Procurement", "Sourcegraph, workflow, graph, security and cloud evaluations use measured sizing inputs."),
            ("No autonomy", "Agents remain research-only; no repository or production writes."),
        ],
        "gate": "G0 - Estate accepted: 100% pilot ownership and source coverage, baseline published, risk authority signed."
    },
    {
        "id": "P1", "name": "Canonical control and evidence plane", "start": 1, "end": 3,
        "outcome": "Every signal and transition has one authority, one schema and one durable receipt.",
        "build": [
            ("Connector SDK", "Webhook, cursor poll, full snapshot, reconcile and provider writeback for GitHub plus the selected work system."),
            ("Authority stack", "Postgres command state and outbox, Kafka transport, Temporal workflows, S3 Object Lock evidence."),
            ("Trust plane", "OIDC workload identity, OPA transition policy, short leases, tenant binding, audit and break-glass design."),
        ],
        "prove": [
            ("Event torture", "Duplicate, out-of-order, missing delete, schema drift, replay and connector outage produce no silent loss."),
            ("State safety", "Illegal transition, stale expected version, expired approval and zero denominator all fail closed."),
            ("Recovery", "Kill workers and replay events; each side effect occurs once logically and every refusal has a receipt."),
        ],
        "unlock": [
            ("Canonical ChangeIntent", "Tickets and autonomous findings can enter one versioned lifecycle."),
            ("Evidence ledger", "Downstream systems can bind decisions to immutable inputs and outputs."),
            ("Still no code writes", "The platform may create proposals and ticket comments only."),
        ],
        "gate": "G1 - Command path accepted: replayable ingestion, legal transitions, WORM evidence and identity isolation pass adversarial tests."
    },
    {
        "id": "P2", "name": "Knowledge and signed context", "start": 2, "end": 6,
        "outcome": "Agents receive complete, ACL-safe, reproducible context for an exact change.",
        "build": [
            ("Code intelligence", "Sourcegraph SCIP coverage by repo, commit, root and indexer; AST fallback tagged lower confidence."),
            ("Knowledge projections", "Neo4j service/code/data/runtime graph; pgvector plus Postgres FTS; tombstones and rebuild pipeline."),
            ("Context Gateway", "MCP read-only retrieval, ACL before candidates, hybrid rank, citations, watermarks, expiry and signature."),
        ],
        "prove": [
            ("Retrieval", "Recall@100 >= 0.95, nDCG@10 >= 0.75, MRR@10 >= 0.85 on protected internal evals."),
            ("Isolation", "0 ACL leaks across 10,000 tenant, revoked, poisoned and deleted pairs."),
            ("Freshness", "Commit-to-queryable and permission revocation SLOs publish checked/total; stale context refuses."),
        ],
        "unlock": [
            ("Impact planning", "Planner can trace code, API, event, database, infrastructure and runtime dependencies."),
            ("Signed ContextPack", "Each child receives pinned repo SHAs, graph snapshot, citations, policy and budget."),
            ("Read-only agent pilot", "Agents can draft plans and clarifications using production-shaped context."),
        ],
        "gate": "G2 - Context accepted: measured recall, zero ACL leakage, deterministic rebuild and signed freshness proof."
    },
    {
        "id": "P3", "name": "Controlled single-repo delivery", "start": 4, "end": 7,
        "outcome": "A bounded agent change reaches a protected PR with independent evidence.",
        "build": [
            ("Fleet child", "One repo, one worktree, one attempt, pinned image, deny egress, fd3-only worker submission and typed exits."),
            ("Plan before code", "Clarification state, reviewer-first oracle, smallest todos, write-set, exact tests, rollback and budgets."),
            ("Proof controller", "Build, unit/integration, Sonar, CodeQL/Snyk/Trivy, secrets, SBOM, provenance and independent review."),
        ],
        "prove": [
            ("Real corpus", "20 representative pilot tasks across bug, feature, dependency and failure paths; no stub-only evidence."),
            ("Independence", "Implementor cannot edit acceptance tests, self-approve, stamp authority fields or bypass required checks."),
            ("Repair", "Seed failing gates and bad patches; bounded repair improves evidence or terminates with a typed refusal."),
        ],
        "unlock": [
            ("Draft PR autonomy", "R0/R1 agent may open a draft PR; protected systems still own review and merge."),
            ("Pilot repository", "One low-risk repository can use the full ticket-to-PR path."),
            ("Measured scorecard", "Acceptance, repair, escaped defects, review effort, latency and cost are visible."),
        ],
        "gate": "G3 - Single-repo pilot accepted: 20 real tasks, no bypass, independent oracle, bounded repair and exact receipts."
    },
    {
        "id": "P4", "name": "Multi-repo release and production gates", "start": 6, "end": 10,
        "outcome": "Interdependent changes move as compatibility waves, not a distributed transaction.",
        "build": [
            ("Dependency planner", "Cross-repo DAG, SCC collapse, read/write conflict serialization, contract versioning and release waves."),
            ("Protected release", "Pact can-i-deploy, merge queue, GitOps environment repo, staging, migration gates and canary analysis."),
            ("Enterprise security", "DPoP tool leases, sidecar egress proxy, tenant fences, agent-stack SBOM/SLSA/Cosign and approval epochs."),
        ],
        "prove": [
            ("Saga drills", "10 multi-repo rehearsals include partial merge, rebase, controller loss, migration lock and compensation."),
            ("Release safety", "Latest-target checks, signature/provenance validation and rollback canary execute from raw artifacts."),
            ("Authority", "No direct push, stale approval reuse, cross-tenant retrieval or agent-triggered break-glass succeeds."),
        ],
        "unlock": [
            ("Cohort 1", "8-10 repositories onboarded with protected R1 change classes."),
            ("Production pilot", "Low-risk compatible changes may canary under deterministic policy."),
            ("Irreversible boundary", "Migrations, IAM expansion and destructive actions remain external human authority."),
        ],
        "gate": "G4 - Multi-repo production accepted: 10 saga drills, zero authority bypass, tested compensation and canary rollback."
    },
    {
        "id": "P5", "name": "Runtime repair, capacity and learning", "start": 9, "end": 13,
        "outcome": "Production evidence re-enters the same governed lifecycle and improves the system.",
        "build": [
            ("Runtime loop", "OTel/CloudWatch RED and USE, SLO burn, deploy correlation, autonomous finding and protected repair PR."),
            ("Capacity and DR", "Admission control, WIP/cost quotas, backpressure, regional fences, restore/replay and duplicate-action checks."),
            ("Company learning", "Incident evidence becomes a policy, eval, runbook or graph PR with owner, scope, expiry and rollback."),
        ],
        "prove": [
            ("Game days", "10 incident drills cover telemetry loss, noisy alerts, repeated repair, regional loss and stale topology."),
            ("SLO proof", "Multi-window burn alerts publish numerator/denominator; no-data cannot promote or silently recover."),
            ("Learning quality", "Seed harmful and stale lessons; poisoning, conflict, expiry and rollback controls reject them."),
        ],
        "unlock": [
            ("Autonomous R1 repair", "Bounded restart, scale, rollback or code-repair PR may run under signed policy."),
            ("Cohort 2", "15-20 repositories and their production telemetry join the lifecycle."),
            ("Operational ownership", "Platform SLOs, pager rotation, runbooks and cost envelopes become mandatory."),
        ],
        "gate": "G5 - Operate loop accepted: game days, SLO denominators, DR reconciliation and verified learning promotion pass."
    },
    {
        "id": "P6", "name": "Estate rollout and earned autonomy", "start": 12, "end": 18,
        "outcome": "The platform is operated as company infrastructure; autonomy is earned per change class.",
        "build": [
            ("Cohort rollout", "Onboarding factory for remaining repositories: contracts, owners, SLOs, policies, evals and golden pipelines."),
            ("Control operations", "24x7 platform SLO, quota and cost control, model/provider failover, schema evolution and evidence retention."),
            ("Autonomy registry", "Per-class scorecard for acceptance, repair, escaped defects, rollback, security, follow-on change and cost."),
        ],
        "prove": [
            ("Outcome windows", "At least 60 closed production outcome windows with UNKNOWN and rollback rates explicitly published."),
            ("Estate coverage", "30/30 repositories mapped; all autonomous classes have owners, policies, evals and kill switches."),
            ("Sustained controls", "No cross-tenant leak, unreceipted refusal, direct merge, stale approval or unknown critical gate."),
        ],
        "unlock": [
            ("R1 at scale", "Reversible low-risk classes may run ticket-to-canary autonomously within quotas."),
            ("R2/R3 remain protected", "Production migration, IAM, destructive data, region failover and legal exceptions stay human-bound."),
            ("Quarterly re-earn", "Autonomy is reduced automatically when scorecards regress; calendar completion grants nothing."),
        ],
        "gate": "G6 - Scale accepted: 30 repos, 60 outcome windows, 24x7 ownership and class-level autonomy scorecards remain green."
    },
]


def txt(c, x, y, value, font="Display", size=10, color=INK, align="left"):
    c.setFillColor(color); c.setFont(font, size)
    if align == "right": c.drawRightString(x, y, value)
    elif align == "center": c.drawCentredString(x, y, value)
    else: c.drawString(x, y, value)


def wrap(value, font, size, width):
    lines=[]
    for raw in str(value).split("\n"):
        if not raw: lines.append(""); continue
        cur=""
        for word in raw.split():
            test=word if not cur else cur+" "+word
            if stringWidth(test,font,size)<=width: cur=test
            else:
                if cur: lines.append(cur)
                cur=word
        if cur: lines.append(cur)
    return lines


def para(c, x, y, value, width, font="Display", size=8.5, leading=11, color=MUTED, max_lines=None):
    lines=wrap(value,font,size,width)
    if max_lines: lines=lines[:max_lines]
    for i,line in enumerate(lines): txt(c,x,y-i*leading,line,font,size,color)
    return y-len(lines)*leading


def rect(c,x,y,w,h,fill=PAPER,stroke=RULE,sw=.7):
    c.setFillColor(fill); c.setStrokeColor(stroke); c.setLineWidth(sw); c.rect(x,y,w,h,fill=1,stroke=1)


def line(c,x1,y1,x2,y2,color=RULE,sw=.7,dash=None):
    c.setStrokeColor(color); c.setLineWidth(sw); c.setDash(dash or []); c.line(x1,y1,x2,y2); c.setDash([])


def header(c,page,code,title,subtitle):
    c.setFillColor(PAPER); c.rect(0,0,W,H,fill=1,stroke=0)
    txt(c,M,H-27,"DEVX LABS / CTO DELIVERY ROADMAP","Mono-Bold",7,BLUE)
    txt(c,W-M,H-27,f"{code} / {page:02d} / 25 AUG 2026", "Mono",7,MUTED,"right")
    line(c,M,H-39,W-M,H-39,INK,.8)
    txt(c,M,H-75,title,"Display-Bold",28,INK)
    txt(c,M,H-98,subtitle,"Editorial",12,BLUE)


def footer(c,page,note="Autonomy is earned by evidence, never by calendar completion."):
    line(c,M,28,W-M,28,RULE,.6)
    txt(c,M,14,note,"Mono",5.8,MUTED)
    txt(c,W-M,14,f"DEVX CONFIDENTIAL / PAGE {page:02d}","Mono-Bold",5.8,INK,"right")
    c.showPage()


def section(c,y,label,note=""):
    txt(c,M,y,label,"Mono-Bold",6.5,INK)
    if note: txt(c,W-M,y,note,"Mono",6,MUTED,"right")
    line(c,M,y-9,W-M,y-9,INK,.75)


def month_x(month,left,right):
    return left+(right-left)*(month/18)


def draw_axis(c,y,left,right,label=True):
    line(c,left,y,right,y,INK,.8)
    for m in range(19):
        x=month_x(m,left,right)
        line(c,x,y-5,x,y+5,INK if m%3==0 else RULE,.7)
        if label and m%3==0: txt(c,x,y+10,f"M{m}","Mono-Bold",6,INK,"center")


def phase_bar(c,y,p,left,right,h=30,critical=False):
    x1=month_x(p["start"],left,right); x2=month_x(p["end"],left,right)
    fill=BLUE if critical else BLUE2; stroke=BLUE
    rect(c,x1,y,x2-x1,h,fill,stroke,.8)
    short={"P0":"Mobilize","P1":"Control + evidence","P2":"Knowledge + context","P3":"Single-repo delivery","P4":"Multi-repo release","P5":"Runtime + learning","P6":"Estate autonomy"}[p["id"]]
    txt(c,x1+8,y+10,f'{p["id"]}  {short}',"Mono-Bold",6.3,PAPER if critical else INK)


def mini_timeline(c,current,y=626):
    left=250; right=W-M
    draw_axis(c,y,left,right,True)
    for p in PHASES:
        x1=month_x(p["start"],left,right); x2=month_x(p["end"],left,right)
        color=BLUE if p["id"]==current else RULE
        c.setFillColor(color); c.rect(x1,y-24,max(2,x2-x1),8,fill=1,stroke=0)


def milestone(c,x,y,month,title,body):
    c.setFillColor(BLUE); c.saveState(); c.translate(x,y); c.rotate(45); c.rect(-5,-5,10,10,fill=1,stroke=0); c.restoreState()
    txt(c,x,y-22,f"M{month} / {title}","Mono-Bold",6.4,INK,"center")
    para(c,x-70,y-36,body,140,"Display",6.5,8,MUTED,3)


def executive_page(c):
    header(c,1,"ROADMAP-00","18 months to governed autonomy","Build the control plane first. Earn autonomy by change class. Keep irreversible authority external.")
    stat_y=610; labels=[("18","MONTHS"),("7","PHASES"),("6","WORKSTREAMS"),("30","REPOSITORIES")]
    cell=(W-2*M)/4
    line(c,M,stat_y+55,W-M,stat_y+55,INK,.8)
    for i,(n,lbl) in enumerate(labels):
        x=M+i*cell
        if i: line(c,x,stat_y-2,x,stat_y+55,RULE,.8)
        txt(c,x+10,stat_y+10,n,"Display",38,INK)
        txt(c,x+10,stat_y-1,lbl,"Mono-Bold",6.2,MUTED)
    section(c,548,"A / MASTER TIMELINE","overlap is intentional; gates remain sequential")
    left=252; right=W-M; draw_axis(c,515,left,right,True)
    y=468
    for i,p in enumerate(PHASES):
        txt(c,M,y+10,p["id"],"Mono-Bold",7,BLUE)
        txt(c,M+35,y+10,p["name"].upper(),"Display-Bold",8,INK)
        phase_bar(c,y,p,left,right,30,critical=i in (0,1,3,4,6))
        y-=42
    section(c,172,"B / EXECUTIVE MILESTONES","each diamond is a go/no-go decision")
    milestones=[
        (1,"BASELINE","estate and risk accepted"),(3,"CONTROL","canonical command path"),(6,"CONTEXT","signed context pilot"),
        (7,"PR PILOT","20 real single-repo tasks"),(10,"PRODUCTION","multi-repo canary"),(13,"OPERATE","repair and DR loop"),(18,"SCALE","30 repos; R1 earned"),
    ]
    for m,t,b in milestones: milestone(c,month_x(m,M+12,W-M-12),132,m,t,b)
    footer(c,1,"Target plan for a 30-repo, 100+ service production estate; reforecast at every evidence gate.")


def workstreams_page(c):
    header(c,2,"ROADMAP-01","Critical path and parallel workstreams","The control/evidence plane gates every other lane; knowledge, runtime and release engineering overlap behind it.")
    section(c,630,"A / INTEGRATED DELIVERY PLAN","blue bars are critical-path work; gray bars are supporting or cohort work")
    left=270; right=W-M; draw_axis(c,596,left,right,True)
    lanes=[
        ("CONTROL + EVIDENCE",[(0,1,"estate/baseline",False),(1,3,"command state + evidence",True),(3,6,"schema hardening",True),(6,18,"operate/evolve",False)]),
        ("KNOWLEDGE + CONTEXT",[(1,2,"inventory connectors",False),(2,6,"SCIP/graph/vector/MCP",True),(6,12,"coverage + evals",True),(12,18,"estate freshness",False)]),
        ("FLEET + AGENT RUNTIME",[(2,4,"adapter contracts",False),(4,7,"single-repo child",True),(6,10,"multi-repo parent",True),(10,18,"scale/failover",False)]),
        ("PROOF + RELEASE",[(2,4,"gate contracts",False),(4,7,"PR evidence",True),(6,10,"merge/GitOps/canary",True),(10,18,"golden pipelines",False)]),
        ("SRE + LEARNING",[(0,3,"baseline/SLO model",False),(6,9,"telemetry joins",False),(9,13,"repair/DR/learning",True),(13,18,"24x7 operations",True)]),
        ("SECURITY + COMPLIANCE",[(0,2,"threat/data model",True),(1,6,"identity/ACL/secrets",True),(6,12,"supply chain/TOCTOU",True),(12,18,"assurance/audit",False)]),
        ("COHORT ONBOARDING",[(5,7,"pilot: 2-3 services",False),(7,10,"cohort 1: 8-10 repos",False),(10,13,"cohort 2: 15-20",False),(13,18,"cohort 3: 30",False)]),
    ]
    y=546
    for name,bars in lanes:
        txt(c,M,y+11,name,"Mono-Bold",6.4,INK)
        line(c,left,y+15,right,y+15,RULE,.5)
        for s,e,label,critical in bars:
            x1=month_x(s,left,right); x2=month_x(e,left,right)
            rect(c,x1,y,x2-x1,30,BLUE if critical else PAPER3,BLUE if critical else RULE,.6)
            txt(c,x1+6,y+10,label,"Mono-Bold",5.5,PAPER if critical else MUTED)
        y-=55
    section(c,135,"B / DEPENDENCY RULES","calendar overlap never bypasses an upstream gate")
    rules=[
        ("01","No ContextPack","until G1 command/evidence authority is accepted."),
        ("02","No code-writing child","until G2 ACL, freshness and retrieval gates pass."),
        ("03","No production canary","until G3 real-task evidence and independent review pass."),
        ("04","No repair autonomy","until G4 multi-repo compensation and release authority pass."),
    ]
    cw=(W-2*M-3*12)/4
    for i,(n,t,b) in enumerate(rules):
        x=M+i*(cw+12); rect(c,x,58,cw,58,PAPER,RULE,.7); txt(c,x+10,98,n,"Mono-Bold",5.8,BLUE); txt(c,x+42,98,t.upper(),"Display-Bold",7.5,INK); para(c,x+10,81,b,cw-20,"Display",6.4,8,MUTED,2)
    footer(c,2)


def phase_page(c,page,p):
    header(c,page,f'ROADMAP-{page-1:02d}',f'{p["id"]} / {p["name"]}',f'Month {p["start"]} to month {p["end"]}. {p["outcome"]}')
    txt(c,M,600,p["id"],"Display-Bold",86,BLUE2)
    txt(c,M+5,583,f'M{p["start"]} - M{p["end"]}',"Mono-Bold",8,BLUE)
    mini_timeline(c,p["id"],625)
    section(c,552,"A / PHASE CONTRACT","build, falsify, then unlock")
    columns=[("BUILD",p["build"],BLUE,PAPER),("PROVE",p["prove"],RED,RED2),("UNLOCK",p["unlock"],GREEN,GREEN2)]
    gap=14; cw=(W-2*M-2*gap)/3
    for ci,(label,items,color,fill) in enumerate(columns):
        x=M+ci*(cw+gap); rect(c,x,225,cw,300,fill,RULE,.7); c.setFillColor(color); c.rect(x,501,4,24,fill=1,stroke=0)
        txt(c,x+12,507,label,"Mono-Bold",7,color)
        yy=468
        for idx,(title,body) in enumerate(items,1):
            txt(c,x+12,yy,f"0{idx}","Mono-Bold",5.8,color)
            txt(c,x+46,yy,title.upper(),"Display-Bold",8,INK)
            yy=para(c,x+46,yy-15,body,cw-62,"Display",7.1,9.2,MUTED,5)-24
            if idx<3: line(c,x+12,yy+12,x+cw-12,yy+12,RULE,.5)
    section(c,188,"B / EXIT GATE","phase completion is a measured decision, not a status report")
    rect(c,M,70,W-2*M,92,INK,INK,.8)
    txt(c,M+16,135,p["gate"].split(" - ")[0],"Mono-Bold",7,BLUE)
    para(c,M+16,113,p["gate"].split(" - ",1)[1],W-2*M-32,"Display-Bold",12,15,PAPER,3)
    txt(c,W-M-16,83,"GO / HOLD / REDESIGN", "Mono-Bold",6.2,MUTED2,"right")
    footer(c,page)


def operating_page(c):
    header(c,10,"ROADMAP-09","Team, decisions and operating constraints","A platform program, not a tooling rollout. Staff the critical path and force a decision at each gate.")
    section(c,630,"A / CORE TEAM RAMP","loaded people cost is the dominant controllable investment")
    ramp=[("M0-M3","8-10 FTE","architecture, control/data, security, platform/SRE, TPM"),("M3-M9","12-16 FTE","add knowledge/retrieval, Fleet runtime, CI/release and QA/evals"),("M9-M18","16-22 FTE","add onboarding, 24x7 reliability, enablement and domain integration")]
    cw=(W-2*M-2*14)/3
    for i,(period,fte,body) in enumerate(ramp):
        x=M+i*(cw+14); rect(c,x,488,cw,116,PAPER if i<2 else BLUE2,RULE,.7)
        txt(c,x+12,574,period,"Mono-Bold",6.4,BLUE); txt(c,x+12,532,fte,"Display-Bold",30,INK); para(c,x+12,510,body,cw-24,"Display",7,9,MUTED,3)
    section(c,452,"B / TEAM TOPOLOGY","one accountable platform owner; federated service owners supply domain truth")
    teams=[
        ("CONTROL PLANE","Postgres/Kafka/Temporal/S3, connectors, schemas, evidence"),("KNOWLEDGE","SCIP, graph, retrieval, Context Gateway, evals"),
        ("AGENT RUNTIME","Fleet, adapters, isolation, budgets, orchestration"),("PROOF + RELEASE","CI gates, PR authority, GitOps, canary, supply chain"),
        ("SRE + SECURITY","SLO/capacity/DR, identity, secrets, assurance, incident learning"),("DOMAIN COHORTS","0.1-0.2 FTE per participating service team during onboarding"),
    ]
    gap=12; bw=(W-2*M-2*gap)/3; bh=72
    for i,(title,body) in enumerate(teams):
        row=i//3; col=i%3; x=M+col*(bw+gap); y=344-row*(bh+12)
        rect(c,x,y,bw,bh,PAPER2,RULE,.6); txt(c,x+10,y+48,title,"Mono-Bold",6.2,INK); para(c,x+10,y+31,body,bw-20,"Display",6.6,8.3,MUTED,3)
    section(c,246,"C / EXECUTIVE DECISION CALENDAR","a hold is cheaper than scaling an unproven control")
    decisions=[("M1","G0","scope + baseline"),("M3","G1","control authority"),("M6","G2","context integrity"),("M7","G3","single-repo PR"),("M10","G4","multi-repo production"),("M13","G5","operate/repair"),("M18","G6","estate autonomy")]
    dw=(W-2*M)/len(decisions)
    line(c,M,194,W-M,194,INK,.8)
    for i,(m,g,d) in enumerate(decisions):
        x=M+i*dw
        if i: line(c,x,142,x,214,RULE,.7)
        txt(c,x+8,202,m,"Mono-Bold",6.2,BLUE); txt(c,x+8,174,g,"Display-Bold",18,INK); txt(c,x+8,151,d,"Mono",5.7,MUTED)
    rect(c,M,62,W-2*M,56,RED2,RED,.8)
    txt(c,M+12,96,"NO-GO", "Mono-Bold",6.5,RED)
    para(c,M+78,99,"Any cross-tenant leak, direct merge, unreceipted refusal, unknown critical gate, stale approval reuse, destructive agent action or unverifiable denominator stops autonomy expansion and returns the affected class to proposal-only.",W-2*M-94,"Display-Bold",8,10,RED,3)
    footer(c,10,"Timeline assumes committed staffing, executive authority and service-team participation; gates may extend the calendar.")


def build():
    os.makedirs(os.path.dirname(OUT),exist_ok=True)
    c=canvas.Canvas(OUT,pagesize=(W,H),pageCompression=1)
    c.setTitle("DevX Labs - AI-Native SDLC Delivery Roadmap")
    c.setAuthor("DevX Labs / CTO Office")
    c.setSubject("18-month phased implementation roadmap for governed AI-native software delivery")
    executive_page(c)
    workstreams_page(c)
    for idx,p in enumerate(PHASES,start=3): phase_page(c,idx,p)
    operating_page(c)
    c.save()
    print(OUT)


if __name__ == "__main__":
    build()
