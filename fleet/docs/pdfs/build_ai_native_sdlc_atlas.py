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
M = 42

INK = HexColor("#0A0A0A")
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
pdfmetrics.registerFont(TTFont("Mono", os.path.join(FONT_ROOT, "GeistMono-Regular.ttf")))
pdfmetrics.registerFont(TTFont("Mono-Bold", os.path.join(FONT_ROOT, "GeistMono-Bold.ttf")))

U = {
    "CloudEvents": "https://github.com/cloudevents/spec",
    "GitHub webhooks": "https://docs.github.com/en/webhooks",
    "GitLab webhooks": "https://docs.gitlab.com/user/project/integrations/webhooks/",
    "Linear webhooks": "https://linear.app/developers/webhooks",
    "Jira webhooks": "https://developer.atlassian.com/cloud/jira/platform/webhooks/",
    "Azure hooks": "https://learn.microsoft.com/en-us/azure/devops/service-hooks/overview?view=azure-devops",
    "Bitbucket hooks": "https://developer.atlassian.com/cloud/bitbucket/rest/api-group-webhooks/",
    "ServiceNow API": "https://developer.servicenow.com/dev.do#!/reference/api/latest/rest/c_TableAPI",
    "OpenTelemetry": "https://opentelemetry.io/docs/collector/",
    "CloudWatch": "https://docs.aws.amazon.com/AmazonCloudWatch/latest/monitoring/WhatIsCloudWatch.html",
    "Sentry": "https://docs.sentry.io/",
    "Datadog": "https://docs.datadoghq.com/",
    "Debezium": "https://debezium.io/documentation/reference/stable/architecture.html",
    "Debezium outbox": "https://debezium.io/documentation/reference/stable/transformations/outbox-event-router.html",
    "OpenLineage": "https://openlineage.io/docs/",
    "Sourcegraph auto-index": "https://sourcegraph.com/docs/code-navigation/auto-indexing",
    "Sourcegraph executors": "https://sourcegraph.com/docs/admin/executors",
    "SCIP upload": "https://sourcegraph.com/docs/cli/references/code-intel/upload",
    "SCIP lifecycle": "https://sourcegraph.com/docs/code-navigation/explanations/uploads",
    "SCIP schema": "https://raw.githubusercontent.com/scip-code/scip/main/scip.proto",
    "Postgres RLS": "https://www.postgresql.org/docs/current/ddl-rowsecurity.html",
    "Postgres ranges": "https://www.postgresql.org/docs/current/rangetypes.html",
    "Postgres SKIP LOCKED": "https://www.postgresql.org/docs/current/sql-select.html",
    "Kafka": "https://kafka.apache.org/documentation/",
    "Schema Registry": "https://docs.confluent.io/platform/current/schema-registry/develop/api.html",
    "Temporal": "https://docs.temporal.io/workflows",
    "Temporal activities": "https://docs.temporal.io/activities",
    "S3 Object Lock": "https://docs.aws.amazon.com/AmazonS3/latest/userguide/object-lock.html",
    "Neo4j constraints": "https://neo4j.com/docs/cypher-manual/current/constraints/",
    "Neo4j import": "https://neo4j.com/docs/operations-manual/current/import/",
    "pgvector": "https://github.com/pgvector/pgvector",
    "Postgres FTS": "https://www.postgresql.org/docs/current/textsearch.html",
    "SQLite": "https://www.sqlite.org/docs.html",
    "tree-sitter": "https://tree-sitter.github.io/tree-sitter/",
    "Voyage embeddings": "https://docs.voyageai.com/docs/embeddings",
    "Voyage pricing": "https://docs.voyageai.com/docs/pricing",
    "MCP": "https://modelcontextprotocol.io/specification/2025-06-18/basic/index",
    "MCP auth": "https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization",
    "OIDC": "https://openid.net/specs/openid-connect-core-1_0-18.html",
    "DPoP": "https://www.rfc-editor.org/rfc/rfc9449/",
    "SPIFFE": "https://spiffe.io/docs/latest/spiffe-about/overview/",
    "Secrets Store CSI": "https://secrets-store-csi-driver.sigs.k8s.io/",
    "AWS Secrets": "https://docs.aws.amazon.com/secretsmanager/latest/userguide/intro.html",
    "AWS Secrets price": "https://aws.amazon.com/secrets-manager/pricing/",
    "AWS KMS": "https://docs.aws.amazon.com/kms/latest/developerguide/overview.html",
    "AWS KMS price": "https://aws.amazon.com/kms/pricing/",
    "Presidio": "https://microsoft.github.io/presidio/",
    "Gitleaks": "https://github.com/gitleaks/gitleaks",
    "CycloneDX": "https://cyclonedx.org/specification/overview/",
    "SLSA": "https://slsa.dev/spec/v1.2/",
    "Cosign": "https://docs.sigstore.dev/cosign/signing/overview/",
    "Kubernetes Jobs": "https://kubernetes.io/docs/concepts/workloads/controllers/job/",
    "Kubernetes NetworkPolicy": "https://kubernetes.io/docs/concepts/services-networking/network-policies/",
    "Git worktree": "https://git-scm.com/docs/git-worktree",
    "GitHub PR API": "https://docs.github.com/en/rest/pulls",
    "GitHub rulesets": "https://docs.github.com/en/rest/repos/rules",
    "GitHub merge queue": "https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue",
    "Pact": "https://docs.pact.io/pact_broker/can_i_deploy",
    "SonarQube": "https://docs.sonarsource.com/sonarqube-server/latest/analyzing-source-code/ci-integration/overview/",
    "Snyk": "https://docs.snyk.io/developer-tools/snyk-cli/commands",
    "Trivy": "https://trivy.dev/docs/latest/guide/target/repository/",
    "ZAP": "https://www.zaproxy.org/docs/automate/automation-framework/",
    "Playwright": "https://playwright.dev/docs/trace-viewer",
    "Lighthouse CI": "https://github.com/GoogleChrome/lighthouse-ci",
    "k6": "https://grafana.com/docs/k6/latest/using-k6/thresholds/",
    "GitHub attestations": "https://docs.github.com/en/actions/concepts/security/artifact-attestations",
    "CodeQL": "https://docs.github.com/en/code-security/code-scanning/introduction-to-code-scanning/about-code-scanning-with-codeql",
    "OPA": "https://www.openpolicyagent.org/docs/rest-api",
    "Argo CD": "https://argo-cd.readthedocs.io/en/stable/user-guide/sync-waves/",
    "Argo Rollouts": "https://argoproj.github.io/argo-rollouts/features/analysis/",
    "SRE burn rates": "https://sre.google/workbook/alerting-on-slos/",
    "Sourcegraph price": "https://sourcegraph.com/pricing",
    "Temporal price": "https://temporal.io/get-cloud/aws-marketplace",
    "Neo4j price": "https://neo4j.com/pricing/",
    "GitHub price": "https://github.com/pricing",
    "GitHub calculator": "https://github.com/pricing/calculator",
    "Sonar price": "https://www.sonarsource.com/plans-and-pricing/",
    "Snyk price": "https://snyk.io/plans/",
    "PactFlow price": "https://pactflow.io/pricing/",
    "Grafana price": "https://grafana.com/pricing/",
    "Terraform price": "https://developer.hashicorp.com/terraform/cloud-docs/overview/estimate-hcp-terraform-cost",
    "OpenAI price": "https://openai.com/business/pricing/",
    "Anthropic price": "https://platform.claude.com/docs/en/about-claude/pricing",
    "Codex docs": "https://developers.openai.com/codex/",
    "Claude Code": "https://docs.anthropic.com/en/docs/claude-code/overview",
    "MSK price": "https://aws.amazon.com/msk/pricing/",
    "Confluent price": "https://www.confluent.io/pricing/",
    "S3 price": "https://aws.amazon.com/s3/pricing/",
}


def text(c, x, y, value, font="Display", size=8, color=INK, align="left"):
    c.setFillColor(color); c.setFont(font, size)
    if align == "right": c.drawRightString(x, y, value)
    elif align == "center": c.drawCentredString(x, y, value)
    else: c.drawString(x, y, value)


def wrapped(value, font, size, width):
    result = []
    for raw in str(value).split("\n"):
        if not raw: result.append(""); continue
        cur = ""
        for word in raw.split():
            test = word if not cur else cur + " " + word
            if stringWidth(test, font, size) <= width: cur = test
            else:
                if cur: result.append(cur)
                cur = word
        if cur: result.append(cur)
    return result


def para(c, x, y, value, width, font="Display", size=6.4, leading=7.8, color=MUTED, max_lines=None):
    lines = wrapped(value, font, size, width)
    if max_lines: lines = lines[:max_lines]
    for i, line_value in enumerate(lines): text(c, x, y - i * leading, line_value, font, size, color)
    return y - len(lines) * leading


def rect(c, x, y, w, h, fill=PAPER, stroke=RULE, sw=.7):
    c.setFillColor(fill); c.setStrokeColor(stroke); c.setLineWidth(sw); c.rect(x, y, w, h, fill=1, stroke=1)


def line(c, x1, y1, x2, y2, color=RULE, sw=.7, dash=None):
    c.setStrokeColor(color); c.setLineWidth(sw); c.setDash(dash or []); c.line(x1, y1, x2, y2); c.setDash([])


def arrow(c, points, color=BLUE, sw=1.1, dash=None):
    c.setStrokeColor(color); c.setFillColor(color); c.setLineWidth(sw); c.setDash(dash or [])
    p = c.beginPath(); p.moveTo(*points[0])
    for pt in points[1:]: p.lineTo(*pt)
    c.drawPath(p, fill=0, stroke=1); c.setDash([])
    x1, y1 = points[-2]; x2, y2 = points[-1]; a = math.atan2(y2-y1, x2-x1); h=5
    p = c.beginPath(); p.moveTo(x2,y2); p.lineTo(x2-h*math.cos(a-.5),y2-h*math.sin(a-.5)); p.lineTo(x2-h*math.cos(a+.5),y2-h*math.sin(a+.5)); p.close(); c.drawPath(p,fill=1,stroke=0)


def badge(c, x, y, label, color=BLUE, fill=BLUE2, url=None):
    w = stringWidth(label, "Mono-Bold", 5.2) + 12
    rect(c, x, y, w, 14, fill, color, .5); text(c, x+6, y+4, label, "Mono-Bold", 5.2, color)
    if url: c.linkURL(url, (x,y,x+w,y+14), relative=0)
    return w


def card(c, x, y, w, h, code, title, body, status="PROPOSED", doc=None, price=None, target=None):
    palette = {"DOC":(BLUE,BLUE2), "LIVE":(GREEN,GREEN2), "PARTIAL":(RED,RED2), "BLOCK":(RED,RED2), "DATA":(MUTED2,PAPER2), "PROPOSED":(BLUE,PAPER)}
    color, fill = palette.get(status, (BLUE,PAPER))
    rect(c,x,y,w,h,fill,color if status != "PROPOSED" else RULE,.75); c.setFillColor(color); c.rect(x,y+h-24,4,24,fill=1,stroke=0)
    text(c,x+10,y+h-16,code,"Mono-Bold",5.5,color); text(c,x+w-9,y+h-16,status,"Mono-Bold",5.0,color,"right")
    text(c,x+10,y+h-34,title.upper(),"Display-Bold",7.5,INK)
    para(c,x+10,y+h-47,body,w-20,"Display",6.0,7.2,MUTED,max(2,int((h-64)/7.2)))
    bx=x+9
    if doc: bx += badge(c,bx,y+7,"DOC",BLUE,BLUE2,doc)+4
    if price: badge(c,bx,y+7,"PRICE",BLUE,BLUE2,price)
    if target: c.linkAbsolute("detail",target,(x,y,x+w,y+h))


def page_header(c, no, code, title, subtitle, bookmark):
    c.bookmarkPage(bookmark); c.addOutlineEntry(title,bookmark,0,False)
    c.setFillColor(PAPER); c.rect(0,0,W,H,fill=1,stroke=0)
    text(c,M,H-29,"DEVX LABS / CTO IMPLEMENTATION ATLAS","Mono-Bold",7,BLUE)
    text(c,W-M,H-29,f"{code} / {no:02d} / 25 AUG 2026 / A1","Mono",6.8,MUTED,"right")
    line(c,M,H-41,W-M,H-41,INK,.9)
    text(c,M,H-76,title,"Display-Bold",23,INK); text(c,M,H-99,subtitle,"Editorial",11,BLUE)
    lx=M
    for color,label in [(BLUE,"CONTROL / PROPOSED"),(GREEN,"EVIDENCE / LIVE"),(RED,"BLOCK / HUMAN"),(MUTED2,"DATA / REPLAY")]:
        c.setFillColor(color); c.rect(lx,H-125,8,8,fill=1,stroke=0); text(c,lx+13,H-123,label,"Mono-Bold",5.4,MUTED); lx += 132
    text(c,W-M,H-123,"DOC = OFFICIAL SOURCE  /  PROPOSED = TARGET  /  LIVE = CHECKOUT EVIDENCE","Mono-Bold",5.4,MUTED,"right")


def footer(c, no, note="DOC/PRICE badges and linked table cells open official sources."):
    line(c,M,28,W-M,28,RULE,.6); text(c,M,15,note,"Mono",5.6,MUTED); text(c,W-M,15,f"DEVX CONFIDENTIAL / PAGE {no:02d}","Mono-Bold",5.6,INK,"right"); c.showPage()


def flow(c, y, items, height=132, gap=8):
    n=len(items); bw=(W-2*M-(n-1)*gap)/n
    for i,item in enumerate(items):
        x=M+i*(bw+gap); card(c,x,y,bw,height,*item)
        if i<n-1: arrow(c,[(x+bw,y+height/2),(x+bw+gap,y+height/2)])


def table(c, x, top, widths, headers, rows, row_h=27, header_h=24, font_size=5.8, links=None):
    total=sum(widths); rect(c,x,top-header_h,total,header_h,INK,INK,.6); xx=x
    for i,h in enumerate(headers): text(c,xx+7,top-16,h.upper(),"Mono-Bold",5.3,PAPER); xx+=widths[i]
    y=top-header_h
    for ri,row in enumerate(rows):
        rect(c,x,y-row_h,total,row_h,PAPER if ri%2==0 else PAPER2,RULE,.4); xx=x
        for ci,val in enumerate(row):
            para(c,xx+7,y-10,val,widths[ci]-14,"Mono-Bold" if ci==0 else "Display",font_size,font_size+1.3,INK if ci==0 else MUTED,2)
            if links and (ri,ci) in links: c.linkURL(links[(ri,ci)],(xx,y-row_h,xx+widths[ci],y),relative=0)
            xx+=widths[ci]
        y-=row_h
    return y


def section(c, y, label, note=""):
    text(c,M,y,label,"Mono-Bold",6.5,INK); text(c,W-M,y,note,"Mono",5.7,MUTED,"right"); line(c,M,y-10,W-M,y-10,INK,.7)


def master(c):
    page_header(c,1,"LLD-00","One autonomous SDLC. One evidence-bound state machine.","Signal → knowledge → plan → Fleet execution → independent proof → release → runtime learning.","master")
    lanes=[
        ("1 / SIGNAL + AUTHORITY","connector gateway / policy","ingestion",[
            ("A1","External ticket","Jira / Linear / GitHub / GitLab / ADO / ServiceNow","DOC",U["Jira webhooks"],None),
            ("A2","Autonomous finding","SLO burn / CVE / drift / cost / flaky / dependency / support","PROPOSED",None,None),
            ("A3","Connector gateway","verify → raw S3 → inbox dedupe → CloudEvent → reconcile","PROPOSED",U["CloudEvents"],None),
            ("A4","Policy-ready?","evidence + freshness + confidence + risk + reversibility","BLOCK",U["OPA"],None),
            ("A5","ChangeIntent v1","approved proposal; source IDs + immutable evidence hash","PROPOSED",None,None),
        ]),
        ("2 / KNOWLEDGE PLANE","authority + projections","authority",[
            ("B1","SCIP path","Sourcegraph scheduler/executor/index/upload; separate vendor path","DOC",U["Sourcegraph auto-index"],None),
            ("B2","Postgres authority","bitemporal rows, cursor, ACL, idempotency, approval","PROPOSED",U["Postgres RLS"],None),
            ("B3","Graph + vector","Neo4j versioned graph; pgvector/FTS ACL-safe retrieval","PROPOSED",U["pgvector"],None),
            ("B4","ContextPack","pinned SHAs + snapshot + citations + policy + expiry + signature","PROPOSED",U["MCP auth"],None),
        ]),
        ("3 / CHANGE ORCHESTRATION","Temporal parent / Fleet child","planning",[
            ("C1","Clarify","agent asks on ticket; timeout → refuse or explicit assumption","PROPOSED",None,None),
            ("C2","Dependency DAG","code/API/event/DB/IaC/runtime edges; SCC + waves","PROPOSED",None,None),
            ("C3","Reviewer first","distinct identity owns acceptance oracle before code","PROPOSED",None,None),
            ("C4","Temporal saga","one child/repo/base SHA/write-set; no global transaction","PROPOSED",U["Temporal"],None),
            ("C5","Fleet pod","local enforcement kernel inside isolated child boundary","PARTIAL",U["Kubernetes Jobs"],None),
        ]),
        ("4 / PROOF + RELEASE","CI / external authority","verification",[
            ("D1","Implement","small todo → worktree → code/tests → typed fd3 submission","PARTIAL",U["Git worktree"],None),
            ("D2","Independent proof","fresh reviewer + manual/API/UI blast-radius tests","PROPOSED",None,None),
            ("D3","Deterministic gates","contracts / quality / security / load / SBOM / provenance","DOC",U["SonarQube"],None),
            ("D4","PR + merge queue","checks bind SHA + evidence; agent cannot self-approve","BLOCK",U["GitHub merge queue"],None),
            ("D5","GitOps canary","staging → 1/5/25/50% → SLO / business / security","DOC",U["Argo Rollouts"],None),
        ]),
        ("5 / OPERATE + LEARN","runtime evidence loop","operations",[
            ("E1","Observe","OTel/CloudWatch RED+USE; SLO burn + telemetry heartbeat","DOC",U["OpenTelemetry"],None),
            ("E2","Repair","alert → finding → ContextPack → Fleet → PR → canary","PROPOSED",None,None),
            ("E3","Production authority","auto reversible R0/R1; human for irreversible R2/R3","BLOCK",U["OPA"],None),
            ("E4","Company learning","evidence → rule/eval/runbook/graph PR → independent proof","PROPOSED",None,None),
        ]),
    ]
    lane_h=118; gap=12; y=H-168-lane_h
    for li,(label,owner,target,items) in enumerate(lanes):
        rect(c,M,y,W-2*M,lane_h,PAPER if li%2==0 else PAPER2,RULE,.6); rect(c,M,y+lane_h-28,245,28,PAPER3,RULE,.5)
        text(c,M+9,y+lane_h-18,label,"Mono-Bold",5.8,INK); text(c,M+235,y+lane_h-18,owner,"Mono",5.1,MUTED,"right")
        inner=M+258; iw=W-M-inner; bw=(iw-(len(items)-1)*8)/len(items)
        for i,it in enumerate(items):
            x=inner+i*(bw+8); card(c,x,y+10,bw,lane_h-20,*it,target=target)
            if i<len(items)-1: arrow(c,[(x+bw,y+lane_h/2-4),(x+bw+8,y+lane_h/2-4)])
        y-=lane_h+gap
    section(c,704,"CROSS-CUTTING ZERO-TRUST CONTROL PLANE","enforced at ingress, retrieval, execution, evidence and promotion")
    controls=[
        ("Z1","Identity","OIDC workload identity → audience/purpose lease → DPoP proof","BLOCK",U["DPoP"],None),
        ("Z2","Authorization","tenant + class + purpose + ACL before candidate retrieval","BLOCK",U["Postgres RLS"],None),
        ("Z3","Secrets","SPIFFE identity → CSI/tmpfs injection → rotation/revocation","BLOCK",U["SPIFFE"],U["AWS Secrets price"]),
        ("Z4","Sandbox","one repo/pod; deny egress; no host path/DB/merge credential","BLOCK",U["Kubernetes NetworkPolicy"],None),
        ("Z5","Supply chain","pin → scan → SBOM → SLSA provenance → Cosign admission","BLOCK",U["SLSA"],None),
        ("Z6","Authority","content-bound approval; human/break-glass for irreversible action","BLOCK",U["OPA"],None),
        ("Z7","State machine","versioned command → legal transition → receipt → reconciliation","BLOCK",U["Temporal"],None),
    ]; flow(c,520,controls,146,9)
    arrow(c,[(W-M-12,y+lane_h+35),(W-M-12,468),(M+12,468),(M+12,H-168-lane_h+35)],MUTED2,.9,[3,3])
    text(c,M+26,476,"RUNTIME / POSTMORTEM / CUSTOMER FEEDBACK RE-ENTERS AS A NEW VERSIONED SIGNAL","Mono-Bold",5.5,MUTED)
    footer(c,1,"Click lifecycle cards to open their implementation block.")


def ingestion(c):
    page_header(c,2,"LLD-01","Signals, ingestion and autonomous discovery","Webhooks reduce latency; cursor polls and snapshots establish completeness; agents only propose.","ingestion")
    section(c,H-151,"A / SOURCE-NEUTRAL CONNECTOR CONTRACT","webhook + cursor + snapshot + command/writeback")
    sources=[
        ("S1","Work systems","Jira / Linear / GitHub / GitLab / ADO / ServiceNow","DOC",U["Jira webhooks"],None),
        ("S2","Code + supply","Git hosts / registries / SBOM / vulnerability / Sourcegraph","DOC",U["GitHub webhooks"],None),
        ("S3","Runtime + cloud","OTel / Sentry / Datadog / AWS / Azure / GCP / K8s","DOC",U["OpenTelemetry"],None),
        ("S4","Data + enterprise","Debezium / OpenLineage / CMDB / identity / support / FinOps","DOC",U["Debezium"],None),
    ]; flow(c,H-285,sources,118,12)
    section(c,H-319,"B / HOT PATH","ACK only after durable raw acceptance; enrichment stays asynchronous")
    hot=[
        ("01","Receive","bounded POST body; connector/tenant route","PROPOSED",None,None),
        ("02","Authenticate","HMAC/OIDC/mTLS; timestamp + replay window","PROPOSED",None,None),
        ("03","Raw durable","Kafka raw log + S3 version/hash; no parse trust","PROPOSED",U["Kafka"],U["S3 price"]),
        ("04","Normalize","CloudEvents 1.0 + source version + raw URI","PROPOSED",U["CloudEvents"],None),
        ("05","Dedupe/order","unique delivery key; entity revision; stale provenance-only","PROPOSED",U["Postgres RLS"],None),
        ("06","Project/outbox","Postgres current head + outbox in one transaction","PROPOSED",U["Debezium outbox"],None),
        ("07","Reconcile","cursor page then full snapshot; persist cursor after page commit","PROPOSED",None,None),
        ("08","Coverage","checked,total,missing,stale,unsupported; checked=0 fails","BLOCK",None,None),
    ]; flow(c,H-492,hot,146,7)
    section(c,H-528,"C / AUTONOMOUS FINDING-TO-CHANGE LOOP","read-only specialists behind deterministic state transitions")
    agents=[
        ("D1","Signal detector","SLO/CVE/drift/flaky/cost/stale dependency","PROPOSED",None,None),
        ("D2","Correlator","trace/deploy/service/fingerprint/time window","PROPOSED",None,None),
        ("D3","Impact mapper","SCIP + contracts + graph + runtime topology","PROPOSED",None,None),
        ("D4","Duplicate resolver","tickets/incidents/PRs/proposal fingerprints","PROPOSED",None,None),
        ("D5","Issue compiler","claim + evidence + owner + confidence + risk","PROPOSED",None,None),
        ("D6","Policy gate","ALLOW outbox write / DENY receipt / REVIEW irreversible","BLOCK",U["OPA"],None),
    ]; flow(c,H-690,agents,136,9)
    rows=[
        ("DUPLICATE","provider retry","unique inbox key; 2xx/no second effect"),("OUT OF ORDER","parallel delivery","source version compare; reconcile head"),
        ("MISSING DELETE","no delete event","two complete scans; permission loss != delete"),("RATE LIMIT","429/quota","Retry-After + tenant token bucket; cursor retained"),
        ("SCHEMA DRIFT","payload changed","raw retained; quarantine; adapter version"),("WEBHOOK GAP","outage/expired hook","cursor + snapshot + coverage denominator"),
    ]; table(c,M,H-723,[145,190,W-2*M-335],["Failure","Cause","Recovery / proof"],rows,25,23,5.8)
    footer(c,2)


def sourcegraph(c):
    page_header(c,3,"LLD-02","Sourcegraph SCIP: exact provenance and corrected arrows","SCIP does not go directly to Temporal or Kafka. Documented product behavior and company controls stay separate.","sourcegraph")
    section(c,H-151,"A / SOURCEGRAPH PRODUCT PATH","official docs; storage backend and deployment topology remain configuration-specific")
    vendor=[
        ("S1","Trigger","auto-index policy OR customer CI on exact commit","DOC",U["Sourcegraph auto-index"],None),
        ("S2","Schedule/queue","auto-indexing schedules exact repo@commit index job","DOC",U["Sourcegraph auto-index"],None),
        ("S3","Executor","managed/self-hosted execution per documented executor policy","DOC",U["Sourcegraph executors"],None),
        ("S4","Indexer","language indexer emits Index{metadata,documents,symbols}","DOC",U["SCIP schema"],None),
        ("S5","Upload","src code-intel upload -repo -commit -root -indexer -file","DOC",U["SCIP upload"],None),
        ("S6","Process","UPLOADING → QUEUED → PROCESSING → COMPLETED/ERRORED","DOC",U["SCIP lifecycle"],None),
        ("S7","Store","upload retained/processed by Sourcegraph; backend is deployment-specific","DOC",U["SCIP lifecycle"],U["Sourcegraph price"]),
        ("S8","Commit graph","async association; stale state visible until updated","DOC",U["SCIP lifecycle"],None),
    ]; flow(c,H-300,vendor,126,7)
    section(c,H-336,"B / COMPANY CONTROL PATH AROUND SOURCEGRAPH","our architecture; not vendor internals")
    custom=[
        ("C1","Push event","git event retained + canonical inbox","PROPOSED",U["GitHub webhooks"],None),
        ("C2","Coverage workflow","Temporal ensures expected repo/commit/root/indexer terminal state","PROPOSED",U["Temporal"],U["Temporal price"]),
        ("C3","Vendor API","request/observe job and poll status; no raw SCIP in workflow","PROPOSED",U["SCIP lifecycle"],None),
        ("C4","Artifact mirror","optional canonical raw SCIP in company S3 with digest","PROPOSED",U["S3 Object Lock"],U["S3 price"]),
        ("C5","Status event","metadata only → outbox/Kafka: key,state,hash,freshness","PROPOSED",U["Kafka"],None),
        ("C6","Projectors","parse artifact → code graph + chunks + coverage record","PROPOSED",U["SCIP schema"],None),
    ]; flow(c,H-505,custom,142,9)
    section(c,H-542,"C / IDENTITY, RECONCILIATION AND FAILURE","version-sensitive cadence; measure commit-to-queryable index")
    left=(W-2*M-16)*.49
    card(c,M,H-720,left,154,"KEY","External idempotency","sha256(tenant | Sourcegraph instance | repo canonical ID | commit SHA | root | indexer | indexer version | config SHA | artifact SHA). Same key + different hash = reproducibility mismatch. CI and auto-index must not overlap the same repo/root/language authority.","PROPOSED",U["SCIP upload"],None)
    rows=[
        ("INDEX FAIL","build/indexer error","retain prior valid; freshness DEGRADED; bounded retry"),("PROCESS ERROR","malformed upload","quarantine digest; re-index exact SHA"),
        ("STALE GRAPH","async association","strict-freshness request refuses/pends"),("UNSUPPORTED","no precise indexer","AST fallback tagged lower confidence"),
        ("ARTIFACT EXPIRED","retention too short","re-run exact index; size from measured p99"),("EXECUTOR RISK","untrusted build hooks","MicroVM/ephemeral + deny egress + destroy"),
    ]; table(c,M+left+16,H-566,[130,175,W-M-(M+left+16)-305],["Failure","Cause","Recovery"],rows,26,23,5.7)
    rect(c,M,48,W-2*M,43,RED2,RED,.8); para(c,M+12,74,"DO NOT DRAW 'SCIP → TEMPORAL' OR 'SCIP → KAFKA' AS SOURCEGRAPH INTERNALS. Temporal coordinates expected coverage; Kafka carries normalized status events; Sourcegraph retains its own queue, workers and storage.",W-2*M-24,"Mono-Bold",6.0,7.2,RED,2)
    footer(c,3)


def authority(c):
    page_header(c,4,"LLD-03","Canonical authority: Postgres, Kafka, Temporal and S3","Each system owns one concern. Only the PostgreSQL command transaction authorizes lifecycle transitions.","authority")
    section(c,H-151,"A / AUTHORITY MAP","no global database, global order or end-to-end exactly-once claim")
    auth=[
        ("A1","Service DB","domain source of truth; local mutation + outbox","PROPOSED",U["Debezium outbox"],None),
        ("A2","Postgres","command authority; bitemporal state, ACL, idempotency, approval","PROPOSED",U["Postgres RLS"],None),
        ("A3","Kafka/MSK","replicated transport + partition replay; not truth","DOC",U["Kafka"],U["MSK price"]),
        ("A4","Temporal","workflow state/timers/retries/saga; not audit ledger","DOC",U["Temporal"],U["Temporal price"]),
        ("A5","S3 Object Lock","immutable raw/evidence/snapshots/manifests","DOC",U["S3 Object Lock"],U["S3 price"]),
    ]; flow(c,H-285,auth,116,12)
    section(c,H-321,"B / EXACT COMMAND-TO-EVIDENCE FLOW","only step 02 is cross-row atomic")
    steps=[
        ("01","Command","tenant + actor + command_id + causation + expected version","PROPOSED",None,None),
        ("02","PG commit","domain/current state + command receipt + outbox","PROPOSED",U["Postgres ranges"],None),
        ("03","Debezium CDC","WAL slot; event ID; aggregate ID becomes partition key","DOC",U["Debezium outbox"],None),
        ("04","Schema gate","FULL/BACKWARD transitive; incompatible publish blocked","DOC",U["Schema Registry"],None),
        ("05","Consumer","inbox event + projection transaction; offset uncommitted","PROPOSED",U["Kafka"],None),
        ("06","Workflow","stable workflow ID; Activities own every I/O/side effect","DOC",U["Temporal activities"],None),
        ("07","Evidence","unique S3 key + hash + checksum + version + retention","DOC",U["S3 Object Lock"],None),
        ("08","Checkpoint","commit next Kafka offset only after required durability","PROPOSED",None,None),
    ]; flow(c,H-493,steps,145,7)
    section(c,H-530,"C / DATA AND FAILURE CONTRACTS","watermarks are source LSN or topic/partition/offset, never wall-clock order")
    col=(W-2*M-2*12)/3
    card(c,M,H-706,col,152,"PG","Bitemporal + tenant","entity_version(tenant, entity, revision, event_id, valid_time, system_time, payload, source_lsn, schema_id). FORCE RLS; app role cannot own tables/BYPASSRLS. Corrections append and close system time; no silent historical rewrite.","PROPOSED",U["Postgres ranges"],None)
    card(c,M+col+12,H-706,col,152,"KAFKA","Partition + replay","topic canonical.<domain>.<aggregate>.v1; key tenant:aggregate_type:aggregate_id; RF3/minISR2/acks all. Replay uses a new consumer group. Async regional replication has duplicate/loss window.","PROPOSED",U["Kafka"],U["Confluent price"])
    card(c,M+2*(col+12),H-706,col,152,"TEMPORAL","Workflow + activity","Workflow code deterministic. I/O in idempotent Activities with retry/timeout/heartbeat. Continue-As-New for history. Kafka transports volume; Temporal coordinates long-running transitions.","DOC",U["Temporal activities"],U["Temporal price"])
    rows=[
        ("CDC STALL","slot/WAL lag","outbox durable; protect disk; recover connector"),("DUPLICATE","retry after side effect","event/business idempotency; never exactly-once claim"),
        ("S3 FAIL","KMS/network","do not commit offset; verify hash before retry"),("REGION LOSS","async lag","fence old writer; promote epoch; reconcile RPO"),
        ("SPLIT BRAIN","old writer reachable","remain unavailable until IAM/network fenced"),
    ]; table(c,M,H-731,[135,180,W-2*M-315],["Failure","Cause","Recovery"],rows,24,22,5.7)
    footer(c,4)


def graph(c):
    page_header(c,5,"LLD-04","Neo4j projection: identities, ordering, conflicts and rebuild","Proposed company projector: Postgres resolves truth; Neo4j is treated as an idempotent disposable serving projection.","graph")
    section(c,H-151,"A / ATOMIC PROJECTOR","never hold a PostgreSQL transaction while calling Neo4j")
    steps=[
        ("01","Claim","outbox READY/expired; SKIP LOCKED; ≤500; 60s lease","PROPOSED",U["Postgres SKIP LOCKED"],None),
        ("02","Validate","schema, tenant, class, hash, version, checked/total","PROPOSED",None,None),
        ("03","Partition","source|tenant|aggregate; preserve revision order","PROPOSED",None,None),
        ("04","Stable UID","UUIDv5 tenant + source:type:key; never Neo4j internal ID","PROPOSED",U["Neo4j constraints"],None),
        ("05","Apply","parameterized Cypher + constraints; no agent Cypher","PROPOSED",U["Neo4j constraints"],U["Neo4j price"]),
        ("06","Resolve","higher apply / same-hash no-op / same-version conflict","PROPOSED",None,None),
        ("07","Commit","Neo4j commit → capture bookmark → PG APPLIED receipt","PROPOSED",None,None),
        ("08","Watermark","advance contiguous offset only when every prior classified","BLOCK",None,None),
    ]; flow(c,H-301,steps,132,7)
    section(c,H-337,"B / SERVING GRAPH AND QUERY BUDGET","evidence-backed typed edges; runtime details expire/roll up")
    col=(W-2*M-16)*.62
    graph_text="Nodes: Team, Repo, Service, Symbol, API, Event, Schema, Dataset, Table, Resource, Deployment, SLO, Incident, Policy.\n\nEdges: OWNS, DEFINES, IMPORTS, CALLS, EXPOSES, CALLS_API, PRODUCES, CONSUMES, READS, WRITES, DEPLOYS_TO, OBSERVED_CALL, VIOLATED.\n\nEvery node/edge: tenant, stable UID, source, source version/event, payload hash, valid/system interval, confidence basis-points, tombstone. Static CALLS_API and runtime OBSERVED_CALL stay separate."
    card(c,M,H-560,col,196,"MODEL","Company relationship graph",graph_text,"PROPOSED",U["Neo4j constraints"],U["Neo4j price"])
    card(c,M+col+16,H-560,W-M-(M+col+16),196,"QUERY","Bounded impact traversal","Indexed anchor required. Service/API/event/data/infra: default 3, hard 5 hops. Code: default 2, hard 3. Async analysis: hard 8. Enforce iterative BFS, 100 frontier/hop, 500 nodes, 200 paths, 2 MiB response, 2s timeout, allowlisted relationships, tenant on every hop. LIMIT after variable expansion is not a safety guard.","BLOCK",U["Neo4j constraints"],None)
    section(c,H-596,"C / REBUILD AND FAILURE","company decision: canonical outbox/snapshot, not Neo4j CDC, is the rebuild source")
    rebuild=[
        ("R1","New DB","constraints/indexes + schema version","PROPOSED",None,None),
        ("R2","Snapshot","authoritative entities/edges + outbox barrier","PROPOSED",None,None),
        ("R3","Bulk import","neo4j-admin import; manifests + counts","DOC",U["Neo4j import"],None),
        ("R4","Replay","all events after boundary; ordered by logical edge","PROPOSED",None,None),
        ("R5","Validate","counts/orphans/tenants/intervals/tombstones/hashes","BLOCK",None,None),
        ("R6","Switch alias","only after online indexes + query parity","PROPOSED",None,None),
        ("R7","Rollback","old graph read-only for 72h","PROPOSED",None,None),
    ]; flow(c,H-741,rebuild,122,9)
    rows=[
        ("TIMEOUT","locks/expansion","bounded retry; halve batch; no PG ACK"),("MISSING ENDPOINT","edge before node","retry then unresolved endpoint; never guess"),
        ("TENANT VIOLATION","bad producer","fail closed + security event"),("REBUILD MISMATCH","lost event/projector","do not switch; diff + earlier replay"),
    ]; table(c,M,H-767,[145,190,W-2*M-335],["Failure","Cause","Recovery"],rows,24,22,5.7)
    footer(c,5)


def retrieval(c):
    page_header(c,6,"LLD-05","Chunk, vector, graph and signed ContextPack","Worked code example: structural atoms, ACL-before-candidate retrieval, hybrid rank, graph expansion and proof.","retrieval")
    section(c,H-151,"A / INGESTION-TO-CONTEXTPACK","1024 dimensions default; 512 only after a protected eval win")
    steps=[
        ("01","Pin","repo@SHA / doc ETag / contract digest / incident revision","PROPOSED",None,None),
        ("02","Parse","SCIP definitions/occurrences + AST exact ranges","DOC",U["SCIP schema"],None),
        ("03","Chunk","exported/hot symbol 350–700 tokens; hard 1,200","PROPOSED",None,None),
        ("04","Secure","classification, ACL, secret/PII/injection; quarantine","BLOCK",None,None),
        ("05","Identity","logical UUIDv5 + immutable chunk version/content hash","PROPOSED",None,None),
        ("06","Embed/index","voyage-code-4/voyage-4 @1024 + PG FTS + HNSW","PROPOSED",U["Voyage embeddings"],U["Voyage pricing"]),
        ("07","Retrieve","authorize → exact anchors + FTS100 + vector100","BLOCK",U["Postgres RLS"],None),
        ("08","Fuse/expand","RRF k=60 → typed graph expansion → rerank top40","PROPOSED",U["pgvector"],None),
        ("09","Select","MMR + topology coverage within ≤24k tokens","PROPOSED",None,None),
        ("10","Sign","ContextPack SHAs/snapshots/citations/coverage/expiry","PROPOSED",U["MCP auth"],None),
    ]; flow(c,H-298,steps,129,6)
    section(c,H-334,"B / WORKED EXAMPLE","fleet::graph::check_freshness from graph.rs")
    col=(W-2*M-2*12)/3
    card(c,M,H-558,col,198,"CHUNK","Canonical code atom","logical_chunk_id: UUIDv5(...)\nchunk_version_id: sha256(...)\nrepo: fleet-rs @ commit SHA\npath: keel/fleet/src/graph.rs\nlines: 1128–1171\nsymbol: fleet::graph::check_freshness\nsignature + docs + complete function\ntoken_count: 314\nacl_policy_id / class / trust\ncontent_hash / parser + version\nedges: impact_command CALLS; CALLS tree_digest","PROPOSED",None,None)
    card(c,M+col+12,H-558,col,198,"VECTOR","Embedding + indexes","model: voyage-code-4\nrevision: pinned\ndimensions: 1024\nmetric: cosine\nvector: [-0.012, 0.002, …]\n\nHeap: vector(1024). HNSW expression index may use halfvec(1024); rerank against full vector. PG FTS GIN weights exact symbol/path/signature/body. ACL relation is materialized before lexical, ANN, graph, cache or reranker receives candidates.","PROPOSED",U["pgvector"],U["Voyage pricing"])
    card(c,M+2*(col+12),H-558,col,198,"PACK","Signed result manifest","pack_id / change_set / child\nactor + authorization digest\nrepo_sha_map + graph/knowledge snapshot\nprojector watermarks\nlexical_k=100 / vector_k=100\nrrf_k=60 / rerank_k=40\ntoken_budget=24000\ncoverage{checked,total,unknown,stale}\nitems[]{source URI, range, hash, ranks, graph paths}\ntruncation/frontier digest\nissued/expires + manifest digest + Ed25519 signature","PROPOSED",U["MCP auth"],None)
    section(c,H-594,"C / ACL, FRESHNESS AND EVALUATION","client-side post-filtering is never authorization")
    left=(W-2*M-16)*.55
    card(c,M,H-750,left,132,"ACL","Authorize before candidate generation","Validate OIDC subject, tenant, purpose, clearance, groups. Materialize authorized source relation. Fine ACL sets use exact vector search; broad same-visibility partitions may use HNSW iterative scans. Every graph hop rechecks tenant/ACL. Empty allowlist, stale permission, dimension mismatch, missing citation or checked=0 refuses.","BLOCK",U["Postgres RLS"],None)
    rows=[
        ("Recall@100",">=0.95","internal 500 code + 500 company queries"),("nDCG@10",">=0.75","no source class regression >0.03"),
        ("Exact MRR@10",">=0.85","symbol/API/incident targets"),("Topology coverage",">=0.90","definition/contract/caller/test/owner/incident"),
        ("ACL leakage","0 / 10k pairs","tenant, revoked, poisoned, deleted"),("ContextPack p95","<2s cached","<8s cold; publish checked/total"),
    ]; table(c,M+left+16,H-618,[145,90,W-M-(M+left+16)-235],["Metric","Gate","Proof"],rows,22,21,5.6)
    footer(c,6)


def planning(c):
    page_header(c,7,"LLD-06","Clarification, multi-repo impact and release DAG","Thirty repos and 100+ services require SCCs, compatibility waves and compensation—not a distributed transaction.","planning")
    section(c,H-151,"A / PLAN BEFORE CODE","reviewer and acceptance oracle are assigned before implementation")
    steps=[
        ("P1","Consume","ticket/ChangeIntent exact version + evidence hash","PROPOSED",None,None),
        ("P2","Clarify","missing AC/NFR/owner/data/rollback → ticket questions","PROPOSED",None,None),
        ("P3","Context","signed per-child pack: sources, graph, contracts, incidents","PROPOSED",U["MCP auth"],None),
        ("P4","Impact graph","code/API/event/DB/IaC/runtime edges + provenance","PROPOSED",None,None),
        ("P5","SCC collapse","cycles become one coupled release unit; topological DAG","PROPOSED",None,None),
        ("P6","Compatibility","version/adapter or expand-migrate-contract","PROPOSED",U["Pact"],U["PactFlow price"]),
        ("P7","Reviewer first","distinct identity/model writes oracle + abuse/manual plan","PROPOSED",None,None),
        ("P8","Atomize","smallest todo with repo/SHA/write-set/deps/tests/rollback","PROPOSED",None,None),
        ("P9","Dispatch","Temporal parent starts dependency-ready Fleet children","PROPOSED",U["Temporal"],None),
    ]; flow(c,H-301,steps,132,7)
    section(c,H-337,"B / WORKED MULTI-REPO WAVE","timeout field added to POST /charge")
    rows=[
        ("W0","contracts","OpenAPI/event/Pact","add optional field; compatibility gate","none"),
        ("W1","ledger-migrations","database","expand nullable column; lock proof","W0"),
        ("W1","generated-sdk","client","regenerate compatible client","W0"),
        ("W2","ledger-service","provider","read old/new; dual write; provider verify","W0,W1"),
        ("W2","payments-api","consumer","new timeout fallback; client/unit/load tests","W0,W1"),
        ("W3","environment","GitOps","feature flag + staged canary","all W2"),
        ("W4","cleanup","all repos","contract old field only after adoption proof","outcome window"),
    ]; table(c,M,H-363,[75,220,155,470,W-2*M-920],["Wave","Repo/unit","Surface","Action + proof","Depends"],rows,31,25,5.9)
    section(c,H-630,"C / CHILD CONTRACT AND FAILURE","overlapping write-sets serialize; every changed base SHA invalidates context/approval")
    col=(W-2*M-16)*.47
    card(c,M,H-805,col,150,"TODO","Atomic child specification","todo_id / parent / child / attempt; repo + immutable base SHA; branch; read/write sets; predecessor IDs; ContextPack digest + lease jti; acceptance predicates + exact test commands; risk + budget + max attempts; rollback/forward-fix; reviewer/oracle digest; expected evidence count. Parent success requires checked == total == required children.","PROPOSED",None,None)
    failures=[
        ("MISSING CONTEXT","no owner/contract/class","clarify then refuse or explicit bounded assumption"),("GRAPH CYCLE","mutual dependency","collapse SCC; one coupled wave"),
        ("BREAKING API","consumer incompatible","version/adapter; Pact can-i-deploy"),("DESTRUCTIVE DB","drop/lock risk","expand-migrate-contract; human boundary"),
        ("PARTIAL MERGE","child fails","pause saga; preserve success; compensate/forward fix"),
    ]; table(c,M+col+16,H-655,[140,175,W-M-(M+col+16)-315],["Failure","Cause","Recovery"],failures,25,23,5.7)
    footer(c,7)


def fleet(c):
    page_header(c,8,"LLD-07","Fleet inside the enterprise SDLC","Fleet is the per-repo enforcement kernel. Remote context, Kubernetes isolation and multi-repo fan-in are target extensions.","fleet")
    section(c,H-151,"A / ONE CHILD EXECUTION","separate company Context MCP and local Fleet stdio MCP trust planes")
    steps=[
        ("F1","Envelope","tenant,parent,repo SHA,write-set,risk,idempotency","PROPOSED",None,None),
        ("F2","Context lease","OIDC → audience-bound 10–15m OAuth token; no refresh","PROPOSED",U["MCP auth"],None),
        ("F3","Child Job","one repo; .worktrees/<child>; pinned image; deny egress","PROPOSED",U["Kubernetes Jobs"],None),
        ("F4","Fleet gate","clean tree + accepted SOW + scoped role/model","LIVE",None,None),
        ("F5","Worker","Codex/Claude adapter; stdin closed; no ledger/state/socket env","PARTIAL",U["OpenAI price"],None),
        ("F6","fd3 submit","typed note/done/refuse; child cannot stamp authority facts","LIVE",None,None),
        ("F7","Freeze","diff + BLAKE3 artifact; empty diff invariant refusal","LIVE",None,None),
        ("F8","Verify","distinct identity; blind suite + adequacy + blast radius + rollback","PARTIAL",None,None),
        ("F9","Receipt","local stamps live; enterprise evidence schema not yet enforced","PARTIAL",None,None),
        ("F10","External PR","controller opens PR/checks; host owns merge authority","PROPOSED",U["GitHub PR API"],U["GitHub price"]),
    ]; flow(c,H-300,steps,132,6)
    section(c,H-336,"B / CONTRACTS AND ISOLATION","agents never receive database, graph, vector, ledger or merge credentials")
    col=(W-2*M-2*12)/3
    card(c,M,H-558,col,196,"PACK","TicketEnvelope + ContextPack","Ticket version, actor, purpose, risk, repos/base SHAs, dependency edges, integer token/cost/time budgets, accepted SOW and idempotency key. Per-child signed ContextPack pins graph/knowledge snapshots, source digests, policy, repo SHA map, coverage and expiry. Retrieval is remote read-only MCP; local file tools remain stdio.","PROPOSED",U["MCP"],U["OpenAI price"])
    card(c,M+col+12,H-558,col,196,"POD","One Job / one repo / one attempt","restartPolicy Never; backoffLimit 0; automount service token false; non-root; read-only root; seccomp RuntimeDefault; drop ALL; resource/PID/deadline limits; size-limited emptyDir; default-deny network. Trusted sidecar brokers lease and uploads evidence. No host path, Docker socket or shared writable Git store.","PROPOSED",U["Kubernetes Jobs"],None)
    card(c,M+2*(col+12),H-558,col,196,"RECEIPT","Worker vs parent authority","LIVE LOCAL: fd3-only {schema_version, kind, body}; no ledger/state/socket path; parent stamps seq/hash/time/actor/model/exit/artifact; refusal receipt precedes exit; immutable acceptance tests. Typed exits: 0 ok, 3 environment, 6 invariant, 7 refusal, 8 mismatch. TARGET SCHEMA: parent/child IDs, SHAs, ContextPack/lease hashes, checked/total and cost source; not yet contract-enforced.","PARTIAL",None,None)
    section(c,H-594,"C / HONEST STATUS","current checkout evidence is not a production-cloud proof")
    rows=[
        ("stdio MCP + scoped files","LIVE","remote HTTP Context MCP + OAuth","PROPOSED"),("local worktree + fd3","LIVE","ephemeral Kubernetes child","PROPOSED"),
        ("Codex/Claude subprocess","PARTIAL","real-adapter corpus + deterministic config","UNPROVEN"),("SQLite tree-sitter graph","LIVE","cross-repo graph/vector/runtime context","PROPOSED"),
        ("hash-linked local receipts","LIVE","signed WORM checkpoints + HA","PROPOSED"),("swarm lifecycle","PARTIAL","30-repo parent/fan-in + PR authority","PROPOSED"),
    ]; table(c,M,H-620,[260,95,W-2*M-450,95],["Current Fleet","Status","Enterprise extension","Status"],rows,27,24,5.9)
    rect(c,M,48,W-2*M,43,RED2,RED,.8); para(c,M+12,74,"PROOF BOUNDARY: current targeted bash tests/acceptance/p0.sh = 34 passed / 0 failed on 25 Aug 2026. This is not the full verify.sh exit. P1–P4, remote MCP, real model-adapter corpus and distributed operation remain unearned. Fleet stays local/partial where evidence stops.",W-2*M-24,"Mono-Bold",6.0,7.2,RED,2)
    footer(c,8)


def verification(c):
    page_header(c,9,"LLD-08","Independent review, CI gates, merge and release","Model prose is not evidence. Each gate binds inputs, version, invocation, exit code, denominator and immutable output.","verification")
    section(c,H-151,"A / VERIFICATION MATRIX","unknown critical gate or checked=0 blocks")
    gates=[
        ("V1","Build/test","native compile + unit/integration + coverage","PROPOSED",None,None),
        ("V2","Contracts","OpenAPI/AsyncAPI/Protobuf + Pact can-i-deploy","DOC",U["Pact"],U["PactFlow price"]),
        ("V3","Quality","Sonar quality gate + lint/complexity/duplication","DOC",U["SonarQube"],U["Sonar price"]),
        ("V4","Security","CodeQL/Snyk/Trivy/secrets/IaC/container/SARIF","DOC",U["Snyk"],U["Snyk price"]),
        ("V5","Dynamic","ZAP auth DAST + Playwright journeys + Lighthouse","DOC",U["ZAP"],None),
        ("V6","Capacity","k6 baseline/stress/soak + p95/p99/saturation/cost","DOC",U["k6"],U["Grafana price"]),
        ("V7","Supply chain","SBOM + signed provenance + attestation verify","DOC",U["GitHub attestations"],None),
        ("V8","Policy","OPA input hash + bundle revision + decision log","DOC",U["OPA"],None),
    ]; flow(c,H-301,gates,132,8)
    section(c,H-337,"B / PR-TO-PRODUCTION","all approvals expire on SHA/evidence/policy/topology change")
    rel=[
        ("R1","Draft PR","SHA, plan, ChangeIntent, evidence, rollback","PROPOSED",U["GitHub PR API"],None),
        ("R2","Fresh review","impact, manual/API/UI, adversarial diff","PROPOSED",None,None),
        ("R3","Ruleset","checks, CODEOWNERS, signatures, no force push","DOC",U["GitHub rulesets"],U["GitHub price"]),
        ("R4","Merge queue","pull_request + merge_group on latest target","DOC",U["GitHub merge queue"],None),
        ("R5","GitOps","environment repo; CI lacks broad production access","PROPOSED",None,None),
        ("R6","Staging","Argo PreSync migration → workloads → PostSync tests","DOC",U["Argo CD"],None),
        ("R7","Canary","1/5/25/50%; SLO/business/security analysis","DOC",U["Argo Rollouts"],None),
        ("R8","Authority","auto reversible; human/break-glass irreversible","BLOCK",U["OPA"],None),
        ("R9","Close","outcome window + evidence hash + learning proposal","PROPOSED",None,None),
    ]; flow(c,H-505,rel,142,7)
    section(c,H-542,"C / NORMALIZED GATE + REPAIR LOOP","failed gate starts a new scoped child from the exact receipt")
    col=(W-2*M-16)*.53
    card(c,M,H-739,col,171,"EVIDENCE","GateResult v1","evidence_id / change_id / repo / commit SHA / artifact digest; gate{name,result,policy revision}; tool{name,version,invocation}; input digest / immutable output URI / actor / attempt / timestamps / exit_code / checked / total / approval / expiry. Results: passed, failed, blocked, unknown, waived, expired. Waiver binds exact SHA/gate/owner/reason/expiry.","PROPOSED",None,None)
    card(c,M+col+16,H-739,W-M-(M+col+16),171,"REPAIR","Independent correction","Failure publishes machine-readable evidence. Parent creates a new repair child with failing evidence, same base/head SHAs, bounded write-set and new attempt. Rerun failed + impacted gates. Stop on max attempts, unknown critical result, attestation mismatch, policy denial or ambiguous blast radius. No silent waiver, reviewer identity swap or implementor self-certification.","BLOCK",None,None)
    footer(c,9)


def operations(c):
    page_header(c,10,"LLD-09","SLO, capacity, incident repair, DR and company learning","Runtime hooks create findings; the same SDLC loop repairs production through protected PRs and canaries.","operations")
    section(c,H-151,"A / RUNTIME-TO-REPAIR LOOP","one SLO authority; telemetry loss pauses promotion")
    steps=[
        ("O1","Instrument","OTel SDK → node agent → gateway x2+","DOC",U["OpenTelemetry"],None),
        ("O2","RED + USE","rates/errors/duration + utilization/saturation/errors","PROPOSED",None,None),
        ("O3","SLO burn","bad/total; multi-window; no-data inconclusive/fail","DOC",U["SRE burn rates"],None),
        ("O4","Correlate","trace + deploy + SHA + image + config + owner + graph","PROPOSED",None,None),
        ("O5","Incident","dedupe + page/ticket + read-only diagnosis","PROPOSED",None,None),
        ("O6","Repair","ChangeIntent → ContextPack → Fleet → PR → canary","PROPOSED",None,None),
        ("O7","Outcome","SLO recovery + regression + capacity + cost","PROPOSED",None,None),
        ("O8","Learn","rule/eval/runbook/graph PR + independent test","PROPOSED",None,None),
    ]; flow(c,H-301,steps,132,8)
    section(c,H-337,"B / SLO, CAPACITY AND AUTONOMY","scale from measured bottleneck; never autoscale past authority or cost reservations")
    col=(W-2*M-2*12)/3
    card(c,M,H-558,col,196,"SLO","Exact error-budget path","Example checkout success >=99.9% / rolling 30d. Error budget 0.1% of eligible requests. Page on paired long/short windows, e.g. 14.4x over 1h and fast confirmation; slower burn opens ticket. Event binds SLI query, checked/total, budget consumed, service/env, trace/deploy/commit/image/config and allowed actions. Zero denominator is invalid, not green.","PROPOSED",U["SRE burn rates"],None)
    card(c,M+col+12,H-558,col,196,"CAPACITY","Admission + bottleneck proof","Publish steady/burst/recovery envelope for events/s, workflow Actions/s, graph/vector writes/s, retrieval QPS, CI fanout and evidence GB/day. Measure p50/p95/p99, saturation, queue depth, retries, storage growth and unit cost. Atomically reserve WIP + provider quota + integer max cost before dispatch. desired = min(machine demand, admitted quota, WIP, provider quota, budget).","BLOCK",U["k6"],U["Grafana price"])
    card(c,M+2*(col+12),H-558,col,196,"AUTONOMY","Risk and reversibility","R0 observe/propose autonomous. R1 bounded reversible scale/restart/rollback autonomous under signed policy and rollback proof. R2 production deploy/data migration/IAM expansion uses external protected authority; human where required. R3 destructive data/region failover/legal exception requires named human + break-glass TTL. Agent cannot approve itself.","BLOCK",U["OPA"],None)
    section(c,H-594,"C / DISASTER RECOVERY","recovery is a tested workflow with checked/total, not a backup-success badge")
    rows=[
        ("POSTGRES","PITR + cross-region replica","new DB; known LSN/time; validate CDC slot continuity"),("KAFKA","RF3/AZ + async replication","fence old region; RPO=lag; dedupe/reconcile"),
        ("TEMPORAL","multi-region/custom or active-passive","region token fences Activity side effects"),("NEO4J","snapshot + outbox replay","new DB; counts/hashes/query parity; alias switch"),
        ("VECTOR","rebuild from chunks/model revision","ACL/recall/stale-vector eval before cutover"),("S3","Object Lock + cross-account CRR","restore/checksum/manifest-chain drill"),
        ("FLEET","durable parent + fresh child","receipt reconciliation; never reuse dirty workspace"),
    ]; table(c,M,H-620,[145,285,W-2*M-430],["Plane","Recovery mechanism","Acceptance proof"],rows,27,24,5.8)
    rect(c,M,48,W-2*M,43,GREEN2,GREEN,.8); para(c,M+12,74,"COMPANY LEARNING IS NOT CHAT MEMORY: immutable incident/change evidence → verified lesson candidate → owner + scope + expiry → policy/eval/runbook/graph PR → independent test → versioned publication → retrieval only after ACL and freshness checks.",W-2*M-24,"Mono-Bold",6.0,7.2,GREEN,2)
    footer(c,10)


def security(c):
    page_header(c,11,"LLD-10","Security and trust boundaries across every agent hop","Identity is workload-bound; context is pre-authorized; tools are deterministic; irreversible authority stays external.","security")
    section(c,H-151,"A / IDENTITY, LEASE AND TOOL-CALL PATH","never pass a reusable bearer token into an agent process")
    identity=[
        ("I1","Workload identity","Kubernetes SA projected token or SPIFFE SVID","DOC",U["SPIFFE"],None),
        ("I2","Token exchange","OIDC subject + tenant + purpose + repo/write-set","DOC",U["OIDC"],None),
        ("I3","Policy decision","OPA binds risk, tool, resource, digest and max budget","BLOCK",U["OPA"],None),
        ("I4","Short lease","audience-bound 5–15m token; no refresh in child","BLOCK",U["MCP auth"],None),
        ("I5","Sender proof","DPoP jkt/nonce binds token to broker-held key","DOC",U["DPoP"],None),
        ("I6","Tool gateway","schema allowlist + resource matcher + rate/cost limit","PROPOSED",U["MCP"],None),
        ("I7","Receipt","jti + request/input/output/policy hashes + checked/total","PROPOSED",None,None),
        ("I8","Revoke/fence","deny new lease; kill Job; invalidate cache and approval","BLOCK",U["OPA"],None),
    ]; flow(c,H-301,identity,132,7)
    section(c,H-337,"B / CONTEXT, SECRET AND TENANT CONFINEMENT","untrusted text can inform a plan; it cannot directly select or invoke a tool")
    col=(W-2*M-2*12)/3
    card(c,M,H-560,col,198,"CONTEXT","Classify before retrieval","Source ACL snapshot → tenant/class/purpose filter → secret + PII detection → prompt-injection label → quarantine/redaction → lexical/vector candidates → per-hop graph ACL → bounded ContextPack. Cache key includes authz digest, source snapshot and policy revision. Empty allowlist or stale permission refuses.","BLOCK",U["Presidio"],None)
    card(c,M+col+12,H-560,col,198,"SECRETS","Broker, inject, destroy","Secrets Manager/KMS owns encrypted value and rotation. Workload identity fetches only named secret version through trusted sidecar/CSI into memory or size-limited tmpfs; never ticket, prompt, env, image, Git, log or evidence. Egress allowlist and service-side IAM enforce use; revoke lease and destroy pod after attempt.","BLOCK",U["Secrets Store CSI"],U["AWS Secrets price"])
    card(c,M+2*(col+12),H-560,col,198,"TENANCY","Isolation at every index","Tenant key is mandatory in canonical row/RLS, Kafka key/topic policy, S3 prefix+KMS context, graph node/edge+hop, vector partition/filter, cache key, ContextPack and evidence. Cross-tenant joins require a separately approved service identity and produce an immutable access receipt; agents never receive unrestricted SQL/Cypher.","BLOCK",U["Postgres RLS"],U["AWS KMS price"])
    section(c,H-596,"C / SOFTWARE-SUPPLY-CHAIN AND INCIDENT RESPONSE","a model-generated diff enters the same or stricter controls as an untrusted contribution")
    supply=[
        ("S1","Pin","commit SHA + lockfiles + action/image digests","BLOCK",U["GitHub attestations"],None),
        ("S2","Scan","Gitleaks + CodeQL/Snyk + Trivy + IaC policy","DOC",U["Gitleaks"],U["Snyk price"]),
        ("S3","Inventory","CycloneDX/SPDX SBOM + dependency/license policy","DOC",U["CycloneDX"],None),
        ("S4","Build","isolated hermetic runner; least privilege; deny default","PROPOSED",U["Kubernetes NetworkPolicy"],None),
        ("S5","Attest","SLSA provenance binds builder, materials and outputs","DOC",U["SLSA"],None),
        ("S6","Sign","keyless Cosign/Sigstore; transparency receipt","DOC",U["Cosign"],None),
        ("S7","Admit","verify signature/provenance/SBOM/policy before deploy","BLOCK",U["Cosign"],None),
        ("S8","Respond","freeze lease/merge/deploy; rotate; preserve WORM evidence","BLOCK",U["S3 Object Lock"],None),
    ]; flow(c,H-742,supply,124,7)
    section(c,900,"D / FAIL-CLOSED ENFORCEMENT CONTRACTS","negative tests are release gates, not documentation")
    controls=[
        ("TOOL REQUEST","canonical hash of typed tool+args+tenant+purpose+repo SHA+write-set+budget","final gateway re-authorizes exact hash; no dynamic production tool registration"),
        ("TENANT SOURCE","derive only from authenticated identity and server-side connector mapping","reject payload/header override; negative-test replay, cache, restore and error paths"),
        ("DPOP REPLAY","validate iss/sub/aud/azp/scope plus htu/htm/iat/jti/nonce and key thumbprint","non-exportable broker key + single-use replay cache + immediate jti fence"),
        ("APPROVAL TOCTOU","bind action args, env, SHAs, pack/model/image/policy digests, cost and expiry","authority atomically rechecks monotonic epoch; cancel queue/retry/failover on revoke"),
        ("SIDECAR/EGRESS","deny hostNetwork/PID/IPC, privileged, hostPath, metadata, raw DNS/IPv6 bypass","authenticated egress proxy + fixed SNI/destination + bounded upload schema"),
        ("SECRET USE","one version + workload + tool + purpose + attempt; arbitrary name denied","prefer broker-mediated operation; redact logs/artifacts; destroy memory/tmpfs and revoke"),
        ("AGENT SUPPLY","SBOM/provenance includes adapters, MCP, skills, prompts/policy and model revision","exact digest admission; independent signer; no agent-held signing key"),
        ("RETENTION/DELETE","per tenant/class/legal hold across Kafka/Temporal/S3/graph/vector/cache/provider","encrypted WORM payload + crypto erasure; tombstone propagation receipt checked=total"),
        ("INSIDER/BREAK-GLASS","named incident + dual human MFA + fixed action hash/scope/TTL","session record + notify + kill switch + independent post-review; never agent-triggered"),
    ]; table(c,M,880,[185,725,W-2*M-910],["Boundary","Required binding","Fail-closed proof"],controls,32,24,5.7)
    rect(c,M,48,W-2*M,43,RED2,RED,.8); para(c,M+12,74,"FORBIDDEN: prompt-selected raw shell/SQL/Cypher, bearer passthrough, child refresh token, secret in model context, ACL-after-retrieval, mutable image/action, self-approval, unlogged override, or destructive autonomous action. Break-glass is named, MFA-protected, time-boxed, scope-bound and post-reviewed.",W-2*M-24,"Mono-Bold",6.0,7.2,RED,2)
    footer(c,11)


def control_plane(c):
    page_header(c,12,"LLD-11","Executable lifecycle state machine and canonical contracts","Cards become implementation only when each transition has one command, one guard, one authority and one receipt.","control")
    section(c,H-151,"A / LEGAL TRANSITIONS","command transaction is authoritative; Temporal coordinates; agents only submit proposals")
    rows=[
        ("SIGNAL_ACCEPTED → REFINED","SignalNormalized / connector","raw URI+hash; source revision; inbox idempotency; tenant from identity","retry adapter; quarantine schema drift"),
        ("REFINED → CONTEXT_READY","ContextPackIssued / context gateway","ACL+freshness; graph/vector watermarks; checked=total; signed expiry","refresh or refuse; never stale fallback"),
        ("REFINED → NEEDS_CLARIFICATION","ClarificationRequested / planner","missing AC/NFR/owner/data/rollback recorded as typed questions","ticket comment; timer; no worker dispatch"),
        ("NEEDS_CLARIFICATION → REFINED","AnswerRecorded / connector","expected ticket revision; answer author; assumption policy decision","conflict creates new signal version"),
        ("CONTEXT_READY → PLANNED","PlanCompiled / planner","pinned graph snapshot; unknown edges; SCCs; waves; child read/write sets","unknown critical edge blocks"),
        ("PLANNED → ORACLE_LOCKED","ReviewOracleStored / reviewer authority","distinct identity; acceptance/manual/abuse plan digest; no implementation diff","expiry on plan/context/policy change"),
        ("ORACLE_LOCKED → IMPLEMENTING","ChildDispatched / Temporal parent","base SHA+pack+oracle+lease current; WIP/cost reserved; repo lock","idempotent child ID; bounded retry"),
        ("IMPLEMENTING → VERIFYING","SubmissionFrozen / Fleet parent","fd3 typed done; nonempty diff; artifact+receipt hash; exact exit taxonomy","refusal/failure receipt before terminal"),
        ("VERIFYING → PR_READY","GateSetPassed / proof controller","independent review; required evidence checked=total; no unknown critical gate","new repair child; max attempts"),
        ("PR_READY → MERGE_QUEUED","MergeRequested / protected Git authority","head SHA/evidence/oracle/policy/topology unchanged; approvals current","rebase invalidates and returns VERIFYING"),
        ("MERGE_QUEUED → STAGING","MergedAndPromoted / GitOps authority","merge-group SHA; provenance/SBOM/signature; environment commit","queue failure returns PR_READY"),
        ("STAGING → CANARY → RELEASED","PromotionStep / release authority","migration proof; 1/5/25/50%; SLO+business+security windows","auto rollback only if signed reversible action"),
        ("RELEASED → OBSERVING → CLOSED","OutcomeAccepted / outcome controller","outcome window; SLO/capacity/cost; learning proposal; evidence hash","regression opens versioned repair signal"),
        ("ANY → BLOCKED","PolicyDenied|Ambiguous|BudgetExceeded / controller","reason code + missing evidence + checked/total + owner + expiry","no silent success; explicit resume command"),
        ("SIDE_EFFECTED → COMPENSATING","SagaRepairRequested / protected authority","completed effects classified; compensation/forward-fix owner and proof","fence epoch; never infer rollback from timeout"),
    ]; y=table(c,M,H-151,[330,300,800,W-2*M-1430],["Transition","Command / authority","Atomic guard + evidence","Timeout / recovery"],rows,31,24,5.45)
    section(c,y-18,"B / VERSIONED CORE CONTRACTS","canonical JSON, sorted keys, integer budgets, hashes over bytes; schema registry blocks incompatibility")
    col=(W-2*M-2*12)/3
    card(c,M,y-220,col,176,"COMMAND","CommandEnvelope v1","command_id / tenant / change_id / expected_state_version / type / actor identity / purpose / risk / aggregate ID / exact resource+args hash / idempotency key / causation+correlation IDs / policy+approval digest / max integer cost+time / issued+expires / signature. PostgreSQL compares expected version and appends state+outbox+receipt atomically.","BLOCK",U["Postgres ranges"],None)
    card(c,M+col+12,y-220,col,176,"SIGNAL","Signal + Finding + ChangeIntent v1","Signal: source instance/object/revision, raw URI/hash, occurred/received, delivery key, connector/schema version, delete/permission state. Finding: detector/version, subject, evidence refs, reproduction, confidence basis-points, dedupe fingerprint, checked/total, TTL. ChangeIntent: accepted facts/assumptions, AC/NFR, repos, risks, allowed actions and policy receipt.","BLOCK",U["CloudEvents"],None)
    card(c,M+2*(col+12),y-220,col,176,"WRITEBACK","ProviderCommand + TransitionReceipt v1","Writeback: provider, object, expected revision, operation{comment,status,label,link}, idempotency key, retry budget; conflict re-ingests latest version. Receipt: before/after state versions, command/event IDs, actor authority, input/output/evidence hashes, policy+approval digests, side-effect key, exit code, checked/total, attempt, epoch and expiry.","BLOCK",U["Jira webhooks"],None)
    section(c,y-270,"C / MULTI-REPO SAGA AND RECONCILIATION","no distributed commit; every external side effect has an idempotency key and epoch fence")
    saga=[
        ("R1","Create parent","workflow_id=tenant:change:vN; persist repo DAG/SCC/waves and child IDs","PROPOSED",U["Temporal"],None),
        ("R2","Dispatch ready","CAS child PENDING→RUNNING; reserve repo/write-set/WIP/cost","BLOCK",U["Temporal activities"],None),
        ("R3","Record effect","PR/check/merge/deploy ID + request hash + provider revision + receipt","BLOCK",None,None),
        ("R4","Partial failure","stop successors; classify untouched/reversible/irreversible/unknown","BLOCK",None,None),
        ("R5","Repair choice","compensate or forward-fix under protected authority; never model-only","BLOCK",U["OPA"],None),
        ("R6","Reconcile","PG state ↔ Temporal history ↔ Git/GitOps state ↔ S3 evidence manifests","PROPOSED",U["Temporal"],None),
        ("R7","Close","all required children/effects checked=total; outcome/rollback proof retained","BLOCK",U["S3 Object Lock"],None),
    ]; flow(c,y-448,saga,142,8)
    rect(c,M,48,W-2*M,43,RED2,RED,.8); para(c,M+12,74,"INVALIDATION: ticket revision, base/head SHA, graph or ACL watermark, ContextPack digest/expiry, oracle, policy bundle, model/image/tool digest, approval epoch or topology change invalidates every downstream lease and approval. Reconciliation classifies UNKNOWN as blocked, never as success.",W-2*M-24,"Mono-Bold",6.0,7.2,RED,2)
    footer(c,12)


def pricing(c):
    page_header(c,13,"LLD-12","Tools, official links and public pricing snapshot","Checked 25 Aug 2026. Public list prices are not enterprise TCO; custom and region-sensitive items stay explicit.","pricing")
    rows=[
        ("Sourcegraph Enterprise","platform/contract","Starts $16K","2026-08-25","public starting price; term/support custom",U["Sourcegraph price"]),
        ("Temporal Cloud","Actions + storage","$50/1M Actions + $100/mo AWS path","2026-08-25","AWS path separate; storage/multi-region confirm",U["Temporal price"]),
        ("Neo4j Aura BC","GB memory/month","$146/GB/mo; 2GB minimum","2026-08-25","VDC/CMEK/private endpoints custom",U["Neo4j price"]),
        ("Postgres + pgvector","license","$0 software license","2026-08-25","HA compute/storage/ops extra",U["pgvector"]),
        ("MSK","broker/GB/network","region and tier usage based","2026-08-25","calculator + support/egress",U["MSK price"]),
        ("Confluent Cloud","eCKU/GB/network","region and tier usage based","2026-08-25","calculator + contract",U["Confluent price"]),
        ("S3 Object Lock","GB-month/requests/CRR","region/class usage based","2026-08-25","retention/KMS/egress sized",U["S3 price"]),
        ("GitHub Enterprise","seat/month","$21/user/mo first 12 months","2026-08-25","EMU/residency/support/actions",U["GitHub price"]),
        ("GitHub security","active committer/month","$30 Code; $19 Secret Protection","2026-08-25","90-day unique committer",U["GitHub calculator"]),
        ("SonarQube Cloud Team","private LOC/month","Starts $34/mo up to 100K LOC","2026-08-25","Enterprise/Server/DC custom",U["Sonar price"]),
        ("Snyk Team / Ignite","contributing developer","$25/mo; $1,260/year","2026-08-25","Enterprise/Broker custom",U["Snyk price"]),
        ("PactFlow Team","integration plan/month","$127 monthly; $115.42 annual","2026-08-25","Enterprise/on-prem custom",U["PactFlow price"]),
        ("Grafana Cloud k6 Pro","VUH + platform","$0.15/VUH + $19/mo","2026-08-25","enterprise/BYOC custom",U["Grafana price"]),
        ("HCP Terraform Essentials","resource-hour","verify current public rate","2026-08-25","linked source lacks stable quoted rate",U["Terraform price"]),
        ("Voyage embeddings","1M tokens after free tier","code-4 $0.12; voyage-4 $0.06","2026-08-25","first 200M/account free; revisions vary",U["Voyage pricing"]),
        ("ChatGPT Business","user/month","$20 annual; $25 monthly","2026-08-25","2-seat minimum; Codex access included",U["OpenAI price"]),
        ("Codex/API usage","model tokens/credits","usage based; model-specific","2026-08-25","workspace credits/API billing separate",U["OpenAI price"]),
        ("Anthropic API","1M input/output tokens","Opus5 $5/$25; Sonnet5 $2/$10; Haiku4.5 $1/$5","2026-08-25","cache/region/agent modifiers",U["Anthropic price"]),
        ("AWS Secrets Manager","secret-month + API calls","$0.40/secret-mo + $0.05/10K calls","2026-08-25","rotation/replication/region usage",U["AWS Secrets price"]),
        ("AWS KMS","key-month + requests","$1/customer key-mo; requests usage-based","2026-08-25","rotation/region/HSM/sign usage",U["AWS KMS price"]),
        ("Core OSS control stack","license","$0 OSS license","2026-08-25","compute/storage/support/operations extra",U["OPA"]),
    ]
    links={(i,0):r[5] for i,r in enumerate(rows)}; shown=[r[:5] for r in rows]
    y=table(c,M,H-151,[230,210,325,110,W-2*M-875],["Tool","Pricing unit","Public list USD","As of","Boundary"],shown,28,25,5.6,links)
    section(c,y-18,"B / COST MODEL AND BUY/BUILD","do not publish a fake single TCO")
    col=(W-2*M-16)/2
    card(c,M,y-202,col,158,"FORMULA","Monthly cost inputs","Seats + Sourcegraph contract + workflow Actions/storage + event compute/storage/network + Postgres + graph GB + object storage/requests/replication + CI runners + security committers/LOC + test VUH + embedding/rerank/model tokens/tools + support + 24x7 operations. Size from engineers, LOC, events/day, nodes/edges, chunks, QPS, agent runs/tokens, CI fanout, evidence GB/day, retention, regions and RPO/RTO.","BLOCK",None,None)
    card(c,M+col+16,y-202,col,158,"BOUNDARY","Recommended greenfield choice","BUY managed data/workflow/code/security/identity/KMS/Kubernetes primitives. BUILD connector SDK, canonical contracts, bitemporal projection, graph projector, Context Gateway/Pack, dependency planner, Fleet cloud parent/child, evidence ledger, policy state machine and eval/learning pipeline. DO NOT BUILD broker, graph DB, CI engine, analyzer, IdP, signer, scheduler or global transaction coordinator.","PROPOSED",None,None)
    footer(c,13,"Tool cells link official pricing. Rates exclude discounts, support, tax and regional uplift; see the individual source catalog.")


def catalog(c):
    page_header(c,14,"LLD-13","Individual official-source catalog","Every named implementation product has its own clickable primary documentation or specification link.","catalog")
    entries=[
        ("Jira webhooks","work intake",U["Jira webhooks"]),("Linear webhooks","work intake",U["Linear webhooks"]),("GitHub webhooks","code/work intake",U["GitHub webhooks"]),
        ("GitLab webhooks","code/work intake",U["GitLab webhooks"]),("Azure DevOps service hooks","code/work intake",U["Azure hooks"]),("Bitbucket webhooks","code/work intake",U["Bitbucket hooks"]),
        ("ServiceNow Table API","enterprise work intake",U["ServiceNow API"]),("CloudEvents","canonical event envelope",U["CloudEvents"]),("Debezium","CDC",U["Debezium"]),
        ("Debezium outbox","transactional outbox",U["Debezium outbox"]),("OpenLineage","data lineage",U["OpenLineage"]),("Sourcegraph auto-index","precise code index",U["Sourcegraph auto-index"]),
        ("Sourcegraph executors","index execution",U["Sourcegraph executors"]),("SCIP schema","code index artifact",U["SCIP schema"]),("PostgreSQL RLS","canonical authorization",U["Postgres RLS"]),
        ("PostgreSQL FTS","lexical retrieval",U["Postgres FTS"]),("pgvector","vector retrieval",U["pgvector"]),("Kafka","ordered partition transport",U["Kafka"]),
        ("Confluent Schema Registry","schema compatibility",U["Schema Registry"]),("Temporal","durable orchestration",U["Temporal"]),("S3 Object Lock","immutable evidence",U["S3 Object Lock"]),
        ("Neo4j","serving graph",U["Neo4j constraints"]),("Voyage AI","code/prose embeddings",U["Voyage embeddings"]),("MCP specification","agent context/tools",U["MCP"]),
        ("MCP authorization","resource-server auth",U["MCP auth"]),("OpenID Connect","workload federation",U["OIDC"]),("DPoP RFC 9449","sender-constrained token",U["DPoP"]),
        ("SPIFFE","workload identity",U["SPIFFE"]),("Secrets Store CSI","secret delivery",U["Secrets Store CSI"]),("AWS Secrets Manager","secret authority",U["AWS Secrets"]),
        ("AWS KMS","key authority",U["AWS KMS"]),("Kubernetes Jobs","ephemeral child",U["Kubernetes Jobs"]),("Kubernetes NetworkPolicy","deny-default egress",U["Kubernetes NetworkPolicy"]),
        ("Git worktree","isolated checkout",U["Git worktree"]),("OpenAI Codex","implementation adapter",U["Codex docs"]),("Claude Code","implementation adapter",U["Claude Code"]),
        ("GitHub pull requests","PR authority",U["GitHub PR API"]),("GitHub rulesets","protected policy",U["GitHub rulesets"]),("GitHub merge queue","latest-target validation",U["GitHub merge queue"]),
        ("Pact","consumer contract proof",U["Pact"]),("SonarQube","quality gate",U["SonarQube"]),("CodeQL","code scanning",U["CodeQL"]),
        ("Snyk","dependency/code security",U["Snyk"]),("Trivy","repo/IaC/container scan",U["Trivy"]),("Gitleaks","secret scan",U["Gitleaks"]),
        ("OWASP ZAP","authenticated DAST",U["ZAP"]),("Playwright","journey/manual proof",U["Playwright"]),("Lighthouse CI","web performance",U["Lighthouse CI"]),
        ("Grafana k6","capacity tests",U["k6"]),("CycloneDX","SBOM",U["CycloneDX"]),("SLSA 1.2","provenance",U["SLSA"]),
        ("Cosign/Sigstore","artifact signing",U["Cosign"]),("GitHub attestations","build attestation",U["GitHub attestations"]),("OPA","deterministic policy",U["OPA"]),
        ("OpenTelemetry","telemetry",U["OpenTelemetry"]),("AWS CloudWatch","runtime monitoring",U["CloudWatch"]),("Sentry","error monitoring",U["Sentry"]),
        ("Datadog","observability",U["Datadog"]),("Argo CD","GitOps sync",U["Argo CD"]),("Argo Rollouts","canary analysis",U["Argo Rollouts"]),
        ("SQLite","local Fleet graph/state",U["SQLite"]),("tree-sitter","local parsing",U["tree-sitter"]),("HCP Terraform","IaC collaboration",U["Terraform price"]),
        ("Presidio","PII detection",U["Presidio"]),("Google SRE burn rates","SLO alert method",U["SRE burn rates"]),
    ]
    half=(len(entries)+1)//2; cols=[entries[:half],entries[half:]]; gap=20; tw=(W-2*M-gap)/2
    for ci,rows in enumerate(cols):
        x=M+ci*(tw+gap); shown=[(name,role,"OFFICIAL") for name,role,_ in rows]; links={(i,0):url for i,(_,_,url) in enumerate(rows)}
        table(c,x,H-151,[250,tw-340,90],["Tool / spec","Role in architecture","Source"],shown,24,24,5.4,links)
    rect(c,M,96,W-2*M,54,BLUE2,BLUE,.8); para(c,M+12,129,"SOURCE RULE: product behavior uses vendor docs/specifications; company-owned topology, SLOs, limits and orchestration are labelled PROPOSED; list price is never presented as enterprise TCO. Broken or redirected links fail the publication gate.",W-2*M-24,"Mono-Bold",6.0,7.2,BLUE,2)
    footer(c,14,"Click each tool name for its individual official source. Pricing remains on page 13 with an effective date and commercial boundary.")


def acceptance(c):
    page_header(c,15,"LLD-14","CTO acceptance: when this architecture becomes real","The proposal is rejected until every requirement has hash-pinned evidence and a published denominator.","acceptance")
    rows=[
        ("V1-01","Source completeness","manifest vs API inventory + replay + snapshot","any unexplained omission/permission gap"),
        ("V1-02","Autonomous discovery","seed code/runtime/security defects across languages","precision/recall absent or non-reproducible"),
        ("V1-03","SCIP correctness","pinned upload/query hashes + freshness state","unsupported vendor-internal arrow/claim"),
        ("V1-04","Temporal","kill worker; duplicate start; retry; signal; replay","repeated side effect/unbounded retry/history"),
        ("V1-05","Multi-repo","shared-contract change across all consumers","partial merge without saga/compensation"),
        ("V1-06","Graph","rebuild raw; no orphan; source SHA/range per edge","mixed revisions/irreproducible projection"),
        ("V1-07","Vector","dimension/metric, ACL, delete, exact recall","cross-tenant/stale/uncited result"),
        ("V1-08","Security","policy bypass, secret exfiltration, OIDC broadening","privilege expansion/untraceable decision"),
        ("V1-09","SLO/capacity","peak+burst ingress/workflow/graph/vector/CI","no p50/p95/p99/saturation/cost/backpressure"),
        ("V1-10","DR","region loss, restore, replay, partial merge","RPO/RTO prose-only or duplicate action"),
        ("V1-11","Authority","self-approval/direct push/stale approval reuse","forbidden transition succeeds"),
        ("V1-12","Fleet","real commit/adapters/corpus/oracle/exit codes","local/placeholder marketed as cloud proof"),
    ]
    y=table(c,M,H-151,[85,210,475,W-2*M-770],["ID","Requirement","Falsifiable test","Reject when"],rows,35,26,6.0)
    section(c,y-18,"B / INDEPENDENT VERIFIERS","run after implementation and before autonomy expansion")
    col=(W-2*M-2*12)/3
    card(c,M,y-199,col,157,"ARCH","Architecture verifier","Every arrow names control/data/evidence path, API/schema, owner, delivery semantics, retry, idempotency and recovery. Every projection names authority and rebuild source. Every multi-repo wave exposes partial success and compensation. Any ambiguous arrow rejects.","BLOCK",None,None)
    card(c,M+col+12,y-199,col,157,"SEC","Security/autonomy verifier","Reject ACL-after-retrieval, bearer passthrough, child refresh tokens, mutable images, self-approval, unrestricted SQL/Cypher/shell/network, missing tenant/class, stale approval, missing break-glass TTL or destructive autonomous action.","BLOCK",None,None)
    card(c,M+2*(col+12),y-199,col,157,"COMM","Evidence/pricing verifier","Reject secondary-source implementation claims, stale/unitless prices, list price represented as TCO, unlinked tools, vendor SLA shown as estate SLO, unsupported Fleet capability, or any green result without raw artifact + checked/total.","BLOCK",None,None)
    rect(c,M,48,W-2*M,43,BLUE2,BLUE,.8); para(c,M+12,74,"FIRST 90 DAYS: connector SDK + evidence contracts → Postgres/S3 authority → Sourcegraph + graph/vector projectors → Context Gateway/leases → Fleet Kubernetes child → GitHub PR/CI/GitOps → autonomous finding loop → capacity/DR → risk-tiered autonomy expansion.",W-2*M-24,"Mono-Bold",6.0,7.2,BLUE,2)
    footer(c,15)


def build():
    os.makedirs(os.path.dirname(OUT),exist_ok=True)
    c=canvas.Canvas(OUT,pagesize=(W,H),pageCompression=1)
    c.setTitle("DevX Labs — AI-Native SDLC Implementation Atlas"); c.setAuthor("DevX Labs / CTO Office")
    c.setSubject("Evidence-bound autonomous SDLC for 30 repositories and 100+ interdependent services")
    for fn in (master,ingestion,sourcegraph,authority,graph,retrieval,planning,fleet,verification,operations,security,control_plane,pricing,catalog,acceptance): fn(c)
    c.save(); print(OUT)


if __name__ == "__main__": build()
