from reportlab.pdfgen import canvas
from reportlab.lib.pagesizes import A1, landscape
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfbase.pdfmetrics import stringWidth
from reportlab.lib.colors import HexColor
import math, os

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
OUT = os.path.join(ROOT, "output/pdf/devx-google-ai-native-sdlc-lld.pdf")
W, H = landscape(A1); M = 42

INK=HexColor("#0A0A0A"); MUTED=HexColor("#5D626A"); MUTED2=HexColor("#8B919A")
RULE=HexColor("#E3E5E8"); PAPER=HexColor("#FFFFFF"); PAPER2=HexColor("#FAFAF8")
BLUE=HexColor("#1769FF"); BLUE2=HexColor("#EAF1FF")
GREEN=HexColor("#087A55"); GREEN2=HexColor("#E9F5F0")
AMBER=HexColor("#A96500"); AMBER2=HexColor("#FFF3DD")
RED=HexColor("#C43D32"); RED2=HexColor("#FCECEA")

FONT_ROOT="/Users/rachitsrivastava/.agents/skills/canvas-design/canvas-fonts"
pdfmetrics.registerFont(TTFont("Display",os.path.join(FONT_ROOT,"InstrumentSans-Regular.ttf")))
pdfmetrics.registerFont(TTFont("Display-Bold",os.path.join(FONT_ROOT,"InstrumentSans-Bold.ttf")))
pdfmetrics.registerFont(TTFont("Editorial",os.path.join(FONT_ROOT,"IBMPlexSerif-Italic.ttf")))
pdfmetrics.registerFont(TTFont("Mono",os.path.join(FONT_ROOT,"GeistMono-Regular.ttf")))
pdfmetrics.registerFont(TTFont("Mono-Bold",os.path.join(FONT_ROOT,"GeistMono-Bold.ttf")))

U={
"editions":"https://docs.cloud.google.com/gemini/enterprise/docs/editions",
"ge":"https://cloud.google.com/gemini-enterprise",
"quotas":"https://docs.cloud.google.com/gemini/enterprise/docs/quotas-and-overages",
"licenses":"https://docs.cloud.google.com/gemini/enterprise/docs/licenses",
"apps":"https://docs.cloud.google.com/gemini/enterprise/docs/apps-data-stores",
"connectors":"https://docs.cloud.google.com/gemini/enterprise/docs/connectors/connect-third-party-data-source",
"github":"https://docs.cloud.google.com/gemini/enterprise/docs/connectors/github",
"jira":"https://docs.cloud.google.com/gemini/enterprise/docs/connectors/jira-cloud",
"custom_connector":"https://docs.cloud.google.com/gemini/enterprise/docs/connectors/custom-connector",
"agents":"https://docs.cloud.google.com/gemini/enterprise/docs/agents-overview",
"designer":"https://docs.cloud.google.com/gemini/enterprise/docs/agent-designer",
"gallery":"https://docs.cloud.google.com/gemini/enterprise/docs/agent-gallery",
"features":"https://docs.cloud.google.com/gemini/enterprise/docs/manage-web-app-features",
"metrics":"https://docs.cloud.google.com/gemini/enterprise/docs/access-metrics",
"model_armor":"https://docs.cloud.google.com/gemini/enterprise/docs/enable-model-armor",
"vpc":"https://docs.cloud.google.com/gemini/enterprise/docs/use-vpc-service-controls",
"code":"https://docs.cloud.google.com/gemini/docs/codeassist/overview",
"code_price":"https://cloud.google.com/products/gemini/pricing",
"code_admin":"https://docs.cloud.google.com/gemini/docs/admin",
"code_agent":"https://docs.cloud.google.com/gemini/docs/codeassist/use-agentic-chat-pair-programmer",
"agent_platform":"https://docs.cloud.google.com/gemini-enterprise-agent-platform/agents",
"agent_runtime":"https://docs.cloud.google.com/gemini-enterprise-agent-platform/build/runtime",
"agent_price":"https://cloud.google.com/products/gemini-enterprise-agent-platform/pricing",
"agent_gen_price":"https://cloud.google.com/gemini-enterprise-agent-platform/generative-ai/pricing",
"registry":"https://docs.cloud.google.com/agent-registry/overview",
"gateway":"https://docs.cloud.google.com/gemini-enterprise-agent-platform/govern/gateways/agent-gateway-overview",
"adk":"https://google.github.io/adk-docs/",
"a2a":"https://docs.cloud.google.com/gemini/enterprise/docs/register-and-manage-an-a2a-agent",
"workflows":"https://docs.cloud.google.com/workflows/docs/overview",
"workflows_price":"https://cloud.google.com/workflows/pricing",
"pubsub":"https://docs.cloud.google.com/pubsub/docs/pubsub-basics",
"pubsub_price":"https://cloud.google.com/pubsub/pricing",
"eventarc":"https://docs.cloud.google.com/eventarc/standard/docs/overview",
"eventarc_price":"https://cloud.google.com/eventarc/pricing",
"run":"https://docs.cloud.google.com/run/docs/overview/what-is-cloud-run",
"run_jobs":"https://cloud.google.com/run/docs/create-jobs",
"run_price":"https://cloud.google.com/run/pricing",
"gke":"https://docs.cloud.google.com/kubernetes-engine/docs/concepts/kubernetes-engine-overview",
"gke_price":"https://cloud.google.com/kubernetes-engine/pricing",
"ssm":"https://docs.cloud.google.com/secure-source-manager/docs/overview",
"ssm_branch":"https://docs.cloud.google.com/secure-source-manager/docs/branch-protection-overview",
"build":"https://docs.cloud.google.com/build/docs/automate-builds",
"build_trigger":"https://docs.cloud.google.com/build/docs/automating-builds/create-manage-triggers",
"build_price":"https://cloud.google.com/build/pricing",
"artifact":"https://docs.cloud.google.com/artifact-registry/docs/overview",
"analysis":"https://docs.cloud.google.com/artifact-analysis/docs/metadata-management-overview",
"binary":"https://docs.cloud.google.com/binary-authorization/docs/overview",
"deploy":"https://docs.cloud.google.com/deploy/docs/overview",
"deploy_price":"https://cloud.google.com/deploy/pricing",
"monitor":"https://docs.cloud.google.com/monitoring/docs/monitoring-overview",
"slo":"https://docs.cloud.google.com/stackdriver/docs/solutions/slo-monitoring",
"logging":"https://docs.cloud.google.com/logging/docs/overview",
"trace":"https://docs.cloud.google.com/trace/docs/overview",
"scc":"https://docs.cloud.google.com/security-command-center/docs/concepts-security-command-center-overview",
"iam":"https://docs.cloud.google.com/iam/docs/overview",
"wif":"https://docs.cloud.google.com/iam/docs/workload-identity-federation",
"kms":"https://docs.cloud.google.com/kms/docs",
"secrets":"https://docs.cloud.google.com/secret-manager/docs/overview",
"dlp":"https://docs.cloud.google.com/sensitive-data-protection/docs/sensitive-data-protection-overview",
"audit":"https://docs.cloud.google.com/logging/docs/audit",
"org_policy":"https://docs.cloud.google.com/resource-manager/docs/organization-policy/overview",
"cai":"https://docs.cloud.google.com/asset-inventory/docs/asset-inventory-overview",
"cai_price":"https://cloud.google.com/asset-inventory/pricing",
"bq":"https://docs.cloud.google.com/bigquery/docs/introduction",
"bq_vector":"https://docs.cloud.google.com/bigquery/docs/vector-search-intro",
"spanner":"https://docs.cloud.google.com/spanner/docs/graph/overview",
"spanner_price":"https://cloud.google.com/spanner/pricing",
"vector":"https://docs.cloud.google.com/vertex-ai/docs/vector-search/overview",
"knowledge":"https://docs.cloud.google.com/dataplex/docs/catalog-overview",
"knowledge_price":"https://cloud.google.com/products/knowledge-catalog/pricing",
"storage":"https://docs.cloud.google.com/storage/docs/bucket-lock",
"lighthouse":"https://github.com/GoogleChrome/lighthouse-ci",
"alloydb":"https://docs.cloud.google.com/alloydb/docs/overview",
"alloydb_vector":"https://docs.cloud.google.com/alloydb/docs/ai/vector-search-overview",
"alloydb_hybrid":"https://docs.cloud.google.com/alloydb/docs/ai/run-hybrid-vector-similarity-search",
"alloydb_cdc":"https://docs.cloud.google.com/datastream/docs/configure-alloydb-psql",
"alloydb_dr":"https://docs.cloud.google.com/alloydb/docs/cross-region-replication/about-cross-region-replication",
"tasks":"https://docs.cloud.google.com/tasks/docs/dual-overview",
"scheduler":"https://docs.cloud.google.com/scheduler/docs/overview",
"pubsub_delivery":"https://docs.cloud.google.com/pubsub/docs/subscriber",
"pubsub_exact":"https://docs.cloud.google.com/pubsub/docs/exactly-once-delivery",
"eventarc_retry":"https://docs.cloud.google.com/eventarc/docs/retry-events",
"dataflow":"https://docs.cloud.google.com/dataflow/docs/overview",
"vertex_embed":"https://docs.cloud.google.com/vertex-ai/generative-ai/docs/embeddings/get-text-embeddings",
"spanner_schema":"https://docs.cloud.google.com/spanner/docs/graph/schema-overview",
"spanner_vector":"https://docs.cloud.google.com/spanner/docs/find-approximate-nearest-neighbors",
"cai_feed":"https://docs.cloud.google.com/asset-inventory/docs/monitoring-asset-changes",
"code_customize":"https://docs.cloud.google.com/gemini/docs/codeassist/code-customization-overview",
"ssm_pr":"https://docs.cloud.google.com/secure-source-manager/docs/work-with-issues-pull-requests",
"build_private":"https://docs.cloud.google.com/build/docs/private-pools/private-pools-overview",
"build_provenance":"https://docs.cloud.google.com/build/docs/securing-builds/generate-validate-build-provenance",
"deploy_canary":"https://docs.cloud.google.com/deploy/docs/deployment-strategies/canary",
"deploy_approval":"https://docs.cloud.google.com/deploy/docs/promote-release",
"monitor_pubsub":"https://docs.cloud.google.com/monitoring/support/notification-options#pubsub",
"prometheus":"https://docs.cloud.google.com/stackdriver/docs/managed-prometheus",
"run_scale":"https://docs.cloud.google.com/run/docs/about-instance-autoscaling",
"gke_hpa":"https://docs.cloud.google.com/kubernetes-engine/docs/concepts/horizontalpodautoscaler",
"policy_controller":"https://docs.cloud.google.com/kubernetes-engine/enterprise/policy-controller/docs/overview",
"billing_export":"https://docs.cloud.google.com/billing/docs/how-to/export-data-bigquery",
"budgets":"https://docs.cloud.google.com/billing/docs/how-to/budgets",
"recommender":"https://docs.cloud.google.com/recommender/docs/overview",
"backup_dr":"https://docs.cloud.google.com/backup-disaster-recovery/docs/concepts/backup-dr",
"spanner_backup":"https://docs.cloud.google.com/spanner/docs/backup",
"storage_dr":"https://docs.cloud.google.com/storage/docs/availability-durability",
"evals":"https://docs.cloud.google.com/vertex-ai/generative-ai/docs/models/evaluation-overview",
"model_price":"https://cloud.google.com/vertex-ai/generative-ai/pricing",
"chat":"https://developers.google.com/workspace/chat/quickstart/webhooks",
"alloydb_price":"https://cloud.google.com/alloydb/pricing",
"tasks_price":"https://cloud.google.com/tasks/pricing",
"ssm_price":"https://cloud.google.com/products/secure-source-manager",
"artifact_price":"https://cloud.google.com/artifact-registry/pricing",
"analysis_price":"https://cloud.google.com/artifact-analysis/pricing",
"binary_price":"https://cloud.google.com/binary-authorization/pricing",
"scc_price":"https://cloud.google.com/security-command-center/pricing",
"storage_price":"https://cloud.google.com/storage/pricing",
"kms_price":"https://cloud.google.com/kms/pricing",
"obs_price":"https://cloud.google.com/products/observability/pricing",
}

def txt(c,x,y,s,font="Display",size=8,color=INK,align="left"):
    c.setFillColor(color); c.setFont(font,size)
    {"left":c.drawString,"right":c.drawRightString,"center":c.drawCentredString}[align](x,y,str(s))

def wrap(s,font,size,width):
    out=[]
    for raw in str(s).split("\n"):
        if not raw: out.append(""); continue
        cur=""
        for word in raw.split():
            test=word if not cur else cur+" "+word
            if stringWidth(test,font,size)<=width: cur=test
            else:
                if cur: out.append(cur)
                cur=word
        if cur: out.append(cur)
    return out

def para(c,x,y,s,width,font="Display",size=6.2,leading=7.5,color=MUTED,max_lines=None):
    lines=wrap(s,font,size,width)
    if max_lines: lines=lines[:max_lines]
    for i,v in enumerate(lines): txt(c,x,y-i*leading,v,font,size,color)
    return y-len(lines)*leading

def rect(c,x,y,w,h,fill=PAPER,stroke=RULE,sw=.6):
    c.setFillColor(fill); c.setStrokeColor(stroke); c.setLineWidth(sw); c.rect(x,y,w,h,fill=1,stroke=1)

def line(c,x1,y1,x2,y2,color=RULE,sw=.7,dash=None):
    c.setStrokeColor(color); c.setLineWidth(sw); c.setDash(dash or []); c.line(x1,y1,x2,y2); c.setDash([])

def arrow(c,pts,color=BLUE,sw=1.1,dash=None):
    c.setStrokeColor(color); c.setFillColor(color); c.setLineWidth(sw); c.setDash(dash or [])
    p=c.beginPath(); p.moveTo(*pts[0])
    for pt in pts[1:]: p.lineTo(*pt)
    c.drawPath(p,fill=0,stroke=1); c.setDash([])
    x1,y1=pts[-2]; x2,y2=pts[-1]; a=math.atan2(y2-y1,x2-x1); h=5
    p=c.beginPath(); p.moveTo(x2,y2); p.lineTo(x2-h*math.cos(a-.5),y2-h*math.sin(a-.5)); p.lineTo(x2-h*math.cos(a+.5),y2-h*math.sin(a+.5)); p.close(); c.drawPath(p,fill=1,stroke=0)

PAL={"INCLUDED":(BLUE,BLUE2),"QUOTA":(BLUE,BLUE2),"APP":(GREEN,GREEN2),"PREVIEW":(RED,RED2),"PAYG":(AMBER,AMBER2),"BUILD":(RED,RED2),"HUMAN":(INK,PAPER2),"DOC":(MUTED2,PAPER2)}

def badge(c,x,y,label,url=None,color=BLUE,fill=BLUE2):
    w=stringWidth(label,"Mono-Bold",5.1)+12; rect(c,x,y,w,14,fill,color,.5); txt(c,x+6,y+4,label,"Mono-Bold",5.1,color)
    if url: c.linkURL(url,(x,y,x+w,y+14),relative=0)
    return w

def card(c,x,y,w,h,code,title,body,status="PAYG",url=None,price=None):
    color,fill=PAL[status]; rect(c,x,y,w,h,fill,color,.75); c.setFillColor(color); c.rect(x,y+h-24,4,24,fill=1,stroke=0)
    txt(c,x+10,y+h-16,code,"Mono-Bold",5.4,color); txt(c,x+w-9,y+h-16,status,"Mono-Bold",5.0,color,"right")
    txt(c,x+10,y+h-34,title.upper(),"Display-Bold",7.4,INK)
    para(c,x+10,y+h-47,body,w-20,"Display",5.9,7.1,MUTED,max(2,int((h-64)/7.1)))
    bx=x+9
    if url: bx+=badge(c,bx,y+7,"DOC",url)+4
    if price: badge(c,bx,y+7,"PRICE",price,AMBER,AMBER2)

def header(c,no,code,title,subtitle,bookmark):
    c.bookmarkPage(bookmark); c.addOutlineEntry(title,bookmark,0,False); c.setFillColor(PAPER); c.rect(0,0,W,H,fill=1,stroke=0)
    txt(c,M,H-29,"DEVX LABS / GOOGLE-NATIVE SDLC ATLAS","Mono-Bold",7,BLUE)
    txt(c,W-M,H-29,f"{code} / {no:02d} / 26 AUG 2026 / A1","Mono",6.8,MUTED,"right")
    line(c,M,H-41,W-M,H-41,INK,.9); txt(c,M,H-76,title,"Display-Bold",23,INK); txt(c,M,H-99,subtitle,"Editorial",11,BLUE)
    lx=M
    for color,label in [(BLUE,"STANDARD INCLUDED/QUOTA"),(GREEN,"APP HANDLED"),(RED,"PREVIEW / CUSTOMER BUILD"),(AMBER,"GOOGLE CLOUD PAYG"),(INK,"HUMAN AUTHORITY")]:
        c.setFillColor(color); c.rect(lx,H-125,8,8,fill=1,stroke=0); txt(c,lx+13,H-123,label,"Mono-Bold",5.2,MUTED); lx+=145
    txt(c,W-M,H-123,"EVERY NAMED PRODUCT LINKS TO A GOOGLE SOURCE","Mono-Bold",5.3,MUTED,"right")

def footer(c,no,note="Prices are public USD list prices observed 26 Aug 2026; contract, region and SKU prevail."):
    line(c,M,28,W-M,28,RULE,.6); txt(c,M,15,note,"Mono",5.5,MUTED); txt(c,W-M,15,f"DEVX CONFIDENTIAL / PAGE {no:02d}","Mono-Bold",5.5,INK,"right"); c.showPage()

def section(c,y,label,note=""):
    txt(c,M,y,label,"Mono-Bold",6.4,INK); txt(c,W-M,y,note,"Mono",5.6,MUTED,"right"); line(c,M,y-10,W-M,y-10,INK,.7)

def table(c,x,top,widths,heads,rows,row_h=29,header_h=24,font=5.6,links=None):
    total=sum(widths); rect(c,x,top-header_h,total,header_h,INK,INK,.6); xx=x
    for i,h in enumerate(heads): txt(c,xx+7,top-16,h.upper(),"Mono-Bold",5.1,PAPER); xx+=widths[i]
    y=top-header_h
    for ri,row in enumerate(rows):
        rect(c,x,y-row_h,total,row_h,PAPER if ri%2==0 else PAPER2,RULE,.4); xx=x
        for ci,val in enumerate(row):
            para(c,xx+7,y-10,val,widths[ci]-14,"Mono-Bold" if ci==0 else "Display",font,font+1.3,INK if ci==0 else MUTED,3)
            if links and (ri,ci) in links: c.linkURL(links[(ri,ci)],(xx,y-row_h,xx+widths[ci],y),relative=0)
            xx+=widths[ci]
        y-=row_h
    return y

def row_cards(c,y,items,h=118,gap=8):
    bw=(W-2*M-(len(items)-1)*gap)/len(items)
    for i,it in enumerate(items):
        x=M+i*(bw+gap); card(c,x,y,bw,h,*it)
        if i<len(items)-1: arrow(c,[(x+bw,y+h/2),(x+bw+gap,y+h/2)])

def master(c):
    header(c,1,"G-LLD-00","The complete Google-native SDLC — with the boundary made explicit.","Gemini Enterprise app is the governed engagement surface; durable delivery is a separate evidence-bound system.","master")
    lanes=[
      ("1 / SIGNAL + READY",[("A1","Ticket / finding","Gemini GitHub/Jira connectors expose search/actions; Monitoring/SCC/CAI/build/cost produce autonomous findings.","APP",U["github"],None),("A2","Normalize","Cloud Run verifies source and stores payload; Pub/Sub transports canonical signal at least once.","PAYG",U["pubsub"],U["pubsub_price"]),("A3","Ready policy","Command service checks source revision, owner, scope, risk, context freshness, authority and budget; asks through connector.","BUILD",U["workflows"],U["workflows_price"]),("A4","ChangeIntent","AlloyDB transaction writes intent/state/outbox/refusal-or-accept receipt; Workflows starts only from committed fact.","BUILD",U["alloydb"],None)]),
      ("2 / CONTEXT + IMPACT",[("B1","Enterprise search","Gemini Enterprise app blends ACL-aware connected data and citations within pooled indexing.","APP",U["apps"],None),("B2","Code graph ingest","Cloud Run Jobs parse pinned SHAs; CAI adds deployed-resource facts; BigQuery keeps history.","BUILD",U["run_jobs"],U["run_price"]),("B3","Graph + vector","Spanner Graph exact paths; Vertex AI Vector Search semantic candidates; Knowledge Catalog governance.","PAYG",U["spanner"],U["spanner_price"]),("B4","ContextPack","Pinned repos + impact paths + acceptance + policy + expiry + ACL + citations; signed by Cloud KMS.","BUILD",U["kms"],None)]),
      ("3 / PLAN + EXECUTE",[("C1","Clarify + plan","ADK planner on Agent Runtime; reviewer/oracle assigned before code; plan and assumptions persist as evidence.","PAYG",U["agent_runtime"],U["agent_price"]),("C2","Parent saga","Workflows coordinates bounded retries/callbacks; AlloyDB owns state; Pub/Sub/Tasks fan out repo-wave children.","PAYG",U["workflows"],U["workflows_price"]),("C3","Fleet kernel","Customer Fleet on GKE/Cloud Run Jobs: fenced lease, isolated worktree, no authority path, typed receipts and budgets.","BUILD",U["gke"],U["gke_price"]),("C4","Implementors","Humans get Code Assist Standard; autonomous Gemini CLI/ADK workers use Vertex AI identity/inference and separate PAYG.","PAYG",U["code"],U["model_price"])]),
      ("4 / PROVE + RELEASE",[("D1","PR + branch rule","Secure Source Manager or GA GitHub connector; protected branch retains independent merge authority.","HUMAN",U["ssm_branch"],None),("D2","Cloud Build gates","Compile/test/lint/contract/security/performance/Lighthouse; a zero-input gate fails.","PAYG",U["build"],U["build_price"]),("D3","Supply chain","Artifact Registry + Artifact Analysis + attestations + Binary Authorization admission.","PAYG",U["binary"],None),("D4","Cloud Deploy","Staging → canary → production on GKE/Cloud Run; approval, verification, rollback evidence.","PAYG",U["deploy"],U["deploy_price"])]),
      ("5 / OBSERVE + REPAIR",[("E1","SLO evidence","Cloud Monitoring, Logging, Trace and SCC emit typed violations, not free-form prompts.","PAYG",U["slo"],None),("E2","Repair intake","Eventarc → Pub/Sub → Workflows creates a new ChangeIntent linked to incident and deployed artifact.","PAYG",U["eventarc"],U["eventarc_price"]),("E3","Proposal only","Repair agent may diagnose, patch and prove; protected authority controls rollback/merge/promotion.","BUILD",U["agent_platform"],U["agent_price"]),("E4","Company learning","Accepted evidence updates BigQuery evals, Spanner edges, Knowledge Catalog and agent policy versions.","BUILD",U["knowledge"],U["knowledge_price"])])]
    y=H-340
    for label,items in lanes:
        txt(c,M,y+171,label,"Mono-Bold",6.1,INK); bw=(W-2*M-3*8)/4
        for i,it in enumerate(items):
            x=M+i*(bw+8); card(c,x,y,bw,160,*it)
            if i<3: arrow(c,[(x+bw,y+80),(x+bw+8,y+80)])
        y-=190
    section(c,460,"NON-NEGOTIABLE AUTHORITY PLANE","IAM + WIF / VPC-SC / KMS / Secret Manager / Model Armor / Audit Logs / Organization Policy / protected merge")
    controls=[("Z1","Identity","Workload and workforce identities; one service account per role.","BUILD",U["wif"],None),("Z2","Secrets","Secret Manager references only; rotate and revoke.","PAYG",U["secrets"],None),("Z3","AI safety","Model Armor app screening is included; A2A agents require their own configuration.","APP",U["model_armor"],None),("Z4","Irreversible action","Content-bound human approval for production, secrets, data loss and merge.","HUMAN",U["ssm_branch"],None),("Z5","Evidence","Every transition writes checked/total, hashes, actor, model, cost and result.","BUILD",U["audit"],None)]
    row_cards(c,282,controls,150,7)
    section(c,250,"CONTRACT ON EVERY ARROW","owner + trigger + typed input + idempotency + ordering + retry/timeout + authority + immutable receipt")
    table(c,M,226,[270,390,420,380,380,460],["Owner","Trigger","Input/output schema","Delivery/idempotency","Retry/recovery","Evidence"],[("named service account or human role","source revision, state version or scheduled reconciliation","versioned JSON/protobuf; hashes over canonical bytes","at-least-once safe; expected version/fencing token","bounded backoff; DLQ is not completion; reconcile UNKNOWN","URI+generation+SHA-256; actor; tool/model; checked/total; exit")],70,25,5.7)
    footer(c,1,"Read top-to-bottom. Green is app-handled; blue is Standard-included; amber is separately metered; red is customer-built; black is human authority.")

def entitlement(c):
    header(c,2,"G-LLD-01","Gemini Enterprise Standard: the exact seat entitlement.","Included means quota-backed access in the subscription — not unlimited runtime or a complete SDLC.","entitlement")
    section(c,H-155,"COMMERCIAL CONTRACT","Public page combines Standard / Plus at a starting price; exact Standard quote remains sales-led.")
    cards=[("S1","Starting price","$30 USD / seat / month starting price is published jointly for Standard/Plus; exact Standard quote is sales-led.","INCLUDED",U["ge"],U["ge"]),("S2","Index pool","30 GiB per licensed Standard user, pooled across editions in the same project and location.","QUOTA",U["quotas"],U["quotas"]),("S3","Assistant","160 queries × Standard seats/day, pooled per project/location; reset at midnight Pacific Time.","QUOTA",U["quotas"],None),("S4","AI dev tools","$10 credit × seats/month; pooled rolling seven-day enforcement; invoiced account; unused credit expires.","QUOTA",U["quotas"],U["agent_price"])]
    row_cards(c,H-290,cards,112)
    section(c,H-320,"FEATURE ENTITLEMENT","All entries below are explicitly checked for Standard in Google’s edition table.")
    rows=[
      ("SEARCH","Full connector ecosystem; permission-aware enterprise search; Google + third-party grounding; blended answers","INCLUDED / quota"),
      ("ASSISTANT","Latest Gemini models priority; Google Search + web grounding; text/image/video generation","INCLUDED / quota"),
      ("NOTEBOOK","Use, create and publish Gemini Notebook Enterprise notebooks","INCLUDED / notebook limits"),
      ("CODE","Gemini Code Assist Standard features; bundled licenses must be assigned","INCLUDED / separate license assignment"),
      ("NO-CODE AGENTS","Build/publish and use custom no-code agents; edition table still labels build/publish Preview","INCLUDED / launch-stage caveat"),
      ("GOOGLE AGENTS","Deep Research 3/day; Data Insights; prebuilt permission-aware tasks/actions","INCLUDED / pooled quota"),
      ("CUSTOM AGENTS","Bring full-code agents built outside app; Agent Marketplace; basic governance/admin","ACCESS INCLUDED / runtime separate"),
      ("SECURITY","Enterprise security/compliance and AI developer tools","INCLUDED / configuration required"),
    ]
    y=table(c,M,H-350,[230,1500,570],["Capability","Documented Standard entitlement","Boundary"],rows,36,24,6.1,{(i,1):U["editions"] for i in range(len(rows))})
    section(c,y-18,"DAILY STANDARD QUOTAS","quantity shown is multiplied by purchased seats and pooled; a few users can consume the pool")
    qs=[("AGENT CREATE","1/day"),("DEEP RESEARCH","3/day"),("IMAGE","5/day"),("VIDEO","2/day"),("ASSISTANT","160/day"),("INDEX","30 GiB/seat")]
    bw=(W-2*M-5*10)/6
    for i,(a,b) in enumerate(qs):
        x=M+i*(bw+10); rect(c,x,y-145,bw,92,BLUE2,BLUE,.8); txt(c,x+12,y-79,a,"Mono-Bold",5.8,BLUE); txt(c,x+12,y-117,b,"Display-Bold",20,INK)
    section(c,y-178,"APP SYSTEM LIMITS","technical ceilings are not purchased quota; Plus is required for enhanced/unlimited blended search")
    limits=[
      ("DATA STORES","100/project default; technical maximum 500","50 stores per blended app"),
      ("ENGINES","150/project/location","project and location scoped"),
      ("DOCUMENTS","10 million regional documents/project/location","indexed bytes consume seat pool"),
      ("QUERY RATE","300 complete-query requests/min/project","Standard blended search remains rate-limited"),
      ("REGIONAL SEARCH","300 requests/min/project/location","quota increase/product limits must be validated"),
      ("SYNC","incremental omits identity/deletion changes","full sync required for deletion reconciliation"),
    ]
    y2=table(c,M,y-203,[250,650,1400],["Limit","Documented value","SDLC consequence"],limits,35,24,5.65,{(0,1):U["quotas"],(5,1):U["connectors"]})
    section(c,y2-18,"BUNDLED CODE ASSIST STANDARD","included license still has its own per-user product quotas; private-repo customization remains Enterprise")
    codeq=[
      ("REQUEST RATE","2 requests/second/user","editor/CLI retry and backpressure still required"),
      ("CODE","6,000 generation/completion requests/day/user","not an autonomous-job completion quota"),
      ("CHAT","960 chat/assistant requests/day/user","separate from Gemini Enterprise assistant pool"),
      ("AGENT + CLI","1,500 combined agent-mode/Gemini CLI requests/day/user","long-running agent compute/model usage can bill separately"),
      ("PR REVIEW","at least 100 GitHub PR reviews/day per installed GitHub app","review output is advisory; branch protection remains authority"),
      ("CONTEXT","local-codebase awareness up to one-million-token context","not a signed multi-repo dependency snapshot"),
    ]
    table(c,M,y2-43,[250,650,1400],["Quota","Documented Standard limit","Boundary"],codeq,34,24,5.55,{(0,1):"https://docs.cloud.google.com/gemini/docs/quotas",(5,1):U["code"]})
    rect(c,M,50,W-2*M,43,RED2,RED,.8); txt(c,M+14,77,"CONTRACT CHECK", "Mono-Bold",6,RED); para(c,M+120,78,"No single fixed Standard price is published for assistant, Deep Research, media or agent-action overages; Google points to Agent Platform pricing. Obtain a written SKU/region/allowlist schedule before approval.",W-2*M-140,"Display",6.5,8,MUTED,2)
    footer(c,2)

def app_boundary(c):
    header(c,3,"G-LLD-02","What the Gemini Enterprise app already handles — and where it stops.","The app is a secure front door and governance surface, not the durable delivery control plane.","app")
    section(c,H-155,"HANDLED BY THE APP","Configured and operated through the Gemini Enterprise app / console.")
    handled=[("A1","Employee surface","Chat, answers, citations, search, actions, agents and Agent Gallery.","APP",U["ge"],None),("A2","Connected knowledge","Many-to-many apps/data stores, blended search, sync/federation and document ACLs.","APP",U["apps"],None),("A3","Connector actions","GitHub can branch/push/PR/Actions/merge; Jira can create/update/assign/status/comment and more.","APP",U["github"],None),("A4","Agent catalog","Register deployed ADK/A2A/Dialogflow agents; test, share, suspend and expose to users.","APP",U["agents"],None),("A5","Basic governance","Feature toggles, app IAM, sharing approval, agent identity display and usage analytics.","APP",U["features"],None),("A6","Prompt screening","Model Armor covers Core Assistant, Google agents and Agent Designer at no extra app cost; not custom ADK/A2A/Dialogflow.","APP",U["model_armor"],None)]
    row_cards(c,H-288,handled,112,7)
    section(c,H-320,"NOT HANDLED BY THE APP","These require a separate Google service, custom control, or human authority.")
    gaps=[("G1","Durable SDLC state","No ticket-to-production saga, idempotency ledger, compensation or cross-repo transaction.","BUILD",U["workflows"],None),("G2","Semantic code graph","Search/data stores are not AST, call, build, API, event, DB or deployment dependency graphs.","BUILD",U["spanner"],None),("G3","Agent hosting","ADK/A2A agents are hosted and maintained on Agent Runtime, Cloud Run or GKE; app registration is not compute.","PAYG",U["agent_runtime"],U["agent_price"]),("G4","Deterministic proof","No hermetic tests, coverage/SAST/DAST/load thresholds, signed provenance or zero-input failure.","BUILD",U["build"],U["build_price"]),("G5","Release authority","Connector merge capability is not protected-branch policy, separation of duties or production approval.","HUMAN",U["ssm_branch"],None),("G6","Auto-remediation","Metrics and alerts do not create proven fixes, execute rollback, or close incidents by themselves.","BUILD",U["monitor"],None)]
    row_cards(c,H-453,gaps,112,7)
    section(c,H-490,"APP-MANAGED DATA + AGENT LIFECYCLE","these controls remain inside the employee application boundary")
    approws=[
      ("DATA STORE","many-to-many app/store binding; blended search; maximum 50 stores/app","search projection, not lifecycle authority"),
      ("SYNC","full, incremental, identity and entity sync schedules","incremental sync omits identity/deletion changes; full reconciliation required"),
      ("FEDERATION","query sent to source with user identity; source ACL and third-party terms apply","query can be rewritten from conversation; not a pinned source snapshot"),
      ("ACL","document ACL + external identity mappings constrain app search","custom connectors must provide correct ACLs/mappings"),
      ("CHUNKING","layout parsing; 100-500 token configurable chunks; BYO chunks Preview","not code-symbol-aware multi-repo chunking"),
      ("KNOWLEDGE GRAPH","narrow supported people/Jira/SharePoint/identity entity relationships","not semantic code/API/data/infra/runtime dependency graph"),
      ("AGENTS","register deployed ADK/A2A/Dialogflow; preview/share/approve/suspend/disable/delete","registration does not include Agent Runtime compute or SDLC state"),
      ("OBSERVABILITY","optional OpenTelemetry traces/spans/logs/metrics; prompt logging configurable","Cloud Logging is PAYG and prompt content can contain PII"),
    ]
    y=table(c,M,H-515,[250,1250,800],["Responsibility","App-managed behavior","Boundary"],approws,40,24,5.55,{(0,1):U["apps"],(1,1):U["connectors"],(6,1):U["agents"],(7,1):U["metrics"]})
    section(c,y-18,"SDLC CONNECTOR/ACTION REALITY","stage and scope differ; connector action is user-authorized capability, not autonomous workflow authority")
    conns=[
      ("GITHUB","documented read/write actions","branch; one/multi-file commit; PR/review/issue/project/label actions; trigger/cancel/rerun Actions; merge","broad GitHub App permission - remove merge from agent role"),
      ("JIRA CLOUD","documented federation/ingestion/actions","create/update/assign/status/link/comment/attachment/worklog; webhooks update entities","real-time index update is not agent trigger; identity changes not real-time"),
      ("LINEAR","Public Preview","federated search/actions; save comment, issue or project","preview control plane cannot be production authority"),
      ("GITLAB","Public Preview","federated search through read_api","no documented branch/commit/MR/pipeline mutation actions"),
      ("SOURCEGRAPH","Public Preview","search/import + MCP search/tools","no documented SCIP-to-orchestrator or complete semantic graph contract"),
      ("CUSTOM MCP","documented integration","HTTPS StreamableHTTP, OAuth 2.0/PKCE, up to 100 enabled actions","bypasses Agent Gateway policy; tools default to confirmation unless read-only"),
    ]
    y2=table(c,M,y-43,[230,300,1000,770],["Connector","Stage","Documented app behavior","Production boundary"],conns,41,24,5.4,{(0,2):U["github"],(1,2):U["jira"],(5,2):U["custom_connector"]})
    section(c,y2-18,"EXTERNAL AUTHORITY PATH","Every mutation leaving the app crosses deterministic control")
    stages=[("REQUEST","Gemini app / connector"),("AUTHORIZE","command API + policy + IAM"),("EXECUTE","Agent Runtime / GKE / Cloud Run"),("PROVE","Cloud Build + locked evidence"),("APPROVE","protected merge / Cloud Deploy authority")]
    bw=(W-2*M-4*22)/5
    sy=max(94,y2-128)
    for i,(a,b) in enumerate(stages):
        x=M+i*(bw+22); rect(c,x,sy,bw,82,PAPER2,INK,.8); txt(c,x+12,sy+56,f"{i+1:02d} / {a}","Mono-Bold",6,BLUE); para(c,x+12,sy+34,b,bw-24,"Display-Bold",7.3,9,INK,3)
        if i<4: arrow(c,[(x+bw,sy+41),(x+bw+22,sy+41)],INK,1)
    rect(c,M,49,W-2*M,35,AMBER2,AMBER,.8); txt(c,M+14,61,"BILLING TRUTH: app visibility is not entitlement. Agent execution, model tokens, logging, CI, runtime and data services remain quota-backed or PAYG.","Mono-Bold",6.0,AMBER)
    footer(c,3)

def signals(c):
    header(c,4,"G-LLD-03","Signals, connectors and the precise ready-to-consume transition.","Connector sync serves search; Eventarc/Pub/Sub/Workflows serve durable orchestration. They are different paths.","signals")
    section(c,H-155,"SOURCE PLANE","External systems remain systems of record; Google-managed connectors expose data/actions.")
    src=[("S1","GitHub","GA federation/actions: issues, PRs, code, branch, push, workflow, merge. GitHub App permissions required.","APP",U["github"],None),("S2","Jira","Search/issues/actions; Jira Cloud launch stage must be checked; Data Center federation/actions GA.","APP",U["jira"],None),("S3","Linear / others","Full connector ecosystem access is included, but many connectors/actions are Public Preview.","APP",U["connectors"],None),("S4","Autonomous findings","Cloud Monitoring SLO burn, SCC finding, CAI drift, build regression, dependency or cost signal.","PAYG",U["monitor"],None)]
    row_cards(c,H-286,src,110)
    section(c,H-317,"ATOMIC EVENT PATH","At-least-once transports demand idempotent consumers and an immutable event identity.")
    path=[("P1","Ingress","Cloud Run verifies provider signature / OAuth principal / timestamp / replay window.","PAYG",U["run"],U["run_price"]),("P2","Envelope","Signal.v1: event ID/key, source revision, tenant, subject, payload URI/hash, time and trace.","BUILD",U["eventarc"],None),("P3","Publish","Pub/Sub topic per trust domain; DLQ; ordering only where configured; consumer remains idempotent.","PAYG",U["pubsub"],U["pubsub_price"]),("P4","Canonicalize","Command service re-fetches current source, hashes evidence, commits inbox + ChangeIntent + outbox in AlloyDB.","BUILD",U["alloydb"],None),("P5","Ready gate","Fields + owner + scope + risk + acceptance + data class + budget + freshness + authority.","BUILD",U["org_policy"],None),("P6","Ack / question","Ready -> queue; ambiguous -> connector comment; stale/duplicate -> receipt + stop.","APP",U["github"],None)]
    row_cards(c,H-448,path,110,7)
    section(c,H-480,"WHY SOURCE SYNC DOES NOT GO DIRECTLY TO WORKFLOWS","The app’s index is optimized for retrieval; orchestration requires source version, retry state and acknowledgement.")
    rows=[("CONNECTOR SYNC","source → Gemini data store","search freshness / ACL / blended answers","No durable SDLC command"),("CHANGE EVENT","source hook → Cloud Run → Pub/Sub","ordered-by-design only where explicitly configured; replay + DLQ","Triggers Workflows intake"),("RECONCILER","Cloud Scheduler → Cloud Run Job","repairs missed hooks; compares source cursor/version","Emits same idempotent event"),("APP ACTION","Gemini UI → connector action","end-user authorized mutation","Must still satisfy external policy for code/prod")]
    y=table(c,M,H-505,[260,530,850,660],["Path","Transport","Purpose","Boundary"],rows,38,24,6.0,{(0,0):U["apps"],(1,0):U["pubsub"],(3,0):U["github"]})
    section(c,y-18,"CANONICAL SIGNAL.V1","large source payload is stored in Cloud Storage; envelope carries reference + digest")
    fields=[
      ("IDENTITY","schema_version, event_id, event_key=sha256(source|native_id), tenant, trust_domain"),
      ("SOURCE","provider, object_type, object_id, external_revision, delivery_id, producer_identity"),
      ("SUBJECT","ticket/repo/service/resource IDs, environment, candidate owner and risk hints"),
      ("TIME","occurred_at, received_at, replay_window, source_cursor/epoch, traceparent"),
      ("PAYLOAD","gs:// URI + object generation + SHA-256 + media/schema type + classification"),
      ("DELIVERY","ordering_key if used, attempt, original message ID, causation/correlation IDs"),
    ]
    y2=table(c,M,y-43,[240,2060],["Field group","Required values"],fields,35,24,5.7,{(0,1):U["eventarc"],(4,1):U["storage"]})
    section(c,y2-18,"READY-TO-CONSUME GATE","only the command transaction moves RECEIVED to READY or NEEDS_CLARIFICATION")
    ready=[
      ("SOURCE","signature/principal valid; current provider revision re-fetched; not deleted; ACL resolvable","quarantine invalid/replayed event; duplicate returns prior receipt"),
      ("INTENT","problem, acceptance criteria, owner, priority, due/urgency and explicit assumptions","post typed questions through connector; no worker dispatch"),
      ("SCOPE","candidate repositories/services, environment, data class, migration/contract/security flags","unknown owner or critical service becomes BLOCKED"),
      ("CONTEXT","repo/default-branch SHAs, connector freshness, graph/index watermarks and evidence hash","stale/missing plane launches reconciliation or indexing"),
      ("AUTHORITY","risk tier, allowed actions, human boundaries, budget reservation and policy bundle digest","no policy/budget/approval -> refuse with expiry and owner"),
      ("EMIT","ChangeIntent.v1 + state version + event/outbox + receipt in AlloyDB transaction","Workflows starts only from committed outbox fact"),
    ]
    y3=table(c,M,y2-43,[210,1280,810],["Gate","Pass condition","Failure path"],ready,39,24,5.55,{(5,1):U["alloydb"]})
    section(c,y3-18,"COMPLETENESS + LIMITS","webhooks reduce latency; cursor/snapshot proves coverage")
    comp=[
      ("HOOK","P95 intake latency; signature failures; duplicate rate; DLQ age","never treated as complete inventory"),
      ("CURSOR POLL","Cloud Scheduler every measured interval; source cursor + count + hash","repairs missed hooks; scheduler itself is at least once"),
      ("SNAPSHOT","daily/weekly full listing by source criticality; checked/total and permission-denied set","zero inputs or unexplained omission fails"),
      ("SIZE","Eventarc event limit and Workflows cumulative-data limit require payload indirection","keep orchestration payloads small; fetch signed evidence by URI"),
    ]
    table(c,M,y3-43,[220,1120,960],["Mechanism","Evidence","Rule"],comp,37,24,5.55,{(1,1):U["scheduler"],(3,1):U["eventarc_retry"]})
    footer(c,4)

def authority_plane(c):
    header(c,5,"G-LLD-04","Canonical authority: AlloyDB, Pub/Sub, Workflows and locked Cloud Storage.","Each service owns one concern. Only the AlloyDB command transaction authorizes lifecycle state; agents and Workflows submit proposals.","authority")
    section(c,H-155,"A / AUTHORITY MAP","no global order, distributed transaction or transport exactly-once claim")
    items=[
      ("A1","Command API","Private Cloud Run service authenticates workload identity, validates schema/hash/expiry and calls one SQL transaction.","BUILD",U["run"],U["run_price"]),
      ("A2","AlloyDB authority","PostgreSQL tables own aggregate version, legal state, approvals, leases, budgets, receipts and transactional outbox.","PAYG",U["alloydb"],None),
      ("A3","Pub/Sub transport","Committed facts fan out at least once; ordering key is change_id only where ordered delivery is explicitly enabled.","PAYG",U["pubsub_delivery"],U["pubsub_price"]),
      ("A4","Workflows saga","Coordinates timers, retries, callbacks, fan-out/fan-in and compensation; never writes lifecycle tables directly.","PAYG",U["workflows"],U["workflows_price"]),
      ("A5","Cloud Tasks","Rate-limited callback/writeback queue; task name is the idempotency key; handler still deduplicates side effects.","PAYG",U["tasks"],None),
      ("A6","Locked evidence","Cloud Storage retention/Bucket Lock holds non-PII evidence bytes; AlloyDB stores URI, generation, SHA-256 and retention class.","PAYG",U["storage"],None),
    ]
    row_cards(c,H-288,items,112,7)
    section(c,H-320,"B / ONE ATOMIC COMMAND TRANSACTION","business refusals commit a receipt; transient SQL failures retry the same command_id")
    rows=[
      ("01 AUTHN","Cloud Run verifies IAM principal and perimeter context","principal, tenant and purpose derive from identity - never request body"),
      ("02 IDEMPOTENCY","INSERT command_receipt(command_id, request_sha256)","same ID+hash returns prior result; different hash = IDEMPOTENCY_CONFLICT"),
      ("03 LOCK","SELECT tenant, change FOR UPDATE","verify authority_epoch, expected_state_version, active lease and budget"),
      ("04 GUARD","legal transition + policy + approval + evidence set","approval binds change, head SHA, graph/context/policy/evidence digests and expiry"),
      ("05 MUTATE","update aggregate; append event; close/open temporal assertion","integer version increments once; absence is never coerced to zero"),
      ("06 OUTBOX","insert event_outbox in same transaction","event ID, aggregate version, schema fingerprint, class and payload hash"),
      ("07 RECEIPT","write ACCEPTED or REFUSED receipt before COMMIT","actor, before/after, checked/total, reason, hashes and timestamps"),
      ("08 PUBLISH","CDC or leased outbox relay publishes after commit","consumer inbox deduplicates event ID; contiguous watermark only"),
    ]
    y=table(c,M,H-345,[210,890,1200],["Step","Atomic operation","Invariant / emitted evidence"],rows,42,24,5.9,{(1,1):U["alloydb"],(7,1):U["alloydb_cdc"]})
    section(c,y-20,"C / DELIVERY, WATERMARK AND FAILURE CONTRACT","Pub/Sub exactly-once is regional/pull-only and does not make external side effects exactly once")
    sem=[
      ("AUTHORITY","epoch + aggregate_version + event_seq","unique command_id + request hash","conflict/retry; never infer success"),
      ("OUTBOX/CDC","slot + LSN + txid + event_id","unique event ID + source position","backlog alarm; snapshot + reconcile"),
      ("PUB/SUB","topic + subscription + message ID + attempt","consumer inbox event ID","ack after commit; DLQ is not completion"),
      ("WORKFLOWS","execution + logical step key","command ID from change+step+epoch","idempotent command; expiring callback token"),
      ("CLOUD TASKS","queue + explicit task name","provider command + expected object revision","409/412 re-ingests latest source"),
      ("EVIDENCE","object generation + SHA-256","content digest + immutable manifest","missing generation/hash blocks close"),
      ("RECONCILE","barrier + per-subscription end position","counts + sorted digest + checked/total","zero or mismatch blocks release"),
    ]
    y2=table(c,M,y-45,[220,610,720,750],["System","Watermark","Idempotency boundary","Recovery"],sem,36,24,5.6,{(2,0):U["pubsub_exact"],(3,0):U["workflows"],(5,0):U["storage"]})
    section(c,y2-18,"D / RECONCILIATION + REGIONAL FAILURE","fence writers before failover; no cross-service atomic regional switch")
    recovery=[
      ("R1","Barrier","Pause commands briefly; append barrier; capture authority version and transport positions.","BUILD",U["pubsub"],None),
      ("R2","Compare","Outbox vs inbox vs evidence: count, IDs, payload hashes, gaps and checked/total.","BUILD",U["alloydb"],None),
      ("R3","Failover","Fence old writer; promote cross-region secondary; increment epoch; reject stale leases.","PAYG",U["alloydb_dr"],None),
      ("R4","Resume","Replay committed outbox; restart saga; reconcile before accepting new writes.","BUILD",U["workflows"],None),
    ]
    row_cards(c,max(105,y2-205),recovery,155,9)
    rect(c,M,48,W-2*M,39,RED2,RED,.8); txt(c,M+14,63,"REFUSE: checked=0 / unknown effect / stale epoch / missing receipt / evidence hash mismatch / side effect without expected provider revision.","Mono-Bold",5.9,RED)
    footer(c,5)

def graph_projection(c):
    header(c,7,"G-LLD-06","Spanner Graph projection: exact identities, ordering, conflicts and rebuild.","AlloyDB and BigQuery facts remain canonical. Spanner Graph is an idempotent serving projection that can be discarded and rebuilt.","graph")
    section(c,H-155,"A / CANONICAL FACT MODEL","models summarize evidence; they never invent identity or resolve conflicting facts")
    facts=[
      ("F1","Observation","Source partition/epoch/offset, native ID/version, operation, payload URI/hash, parser and observed/effective time.","BUILD",U["bq"],None),
      ("F2","Entity registry","Stable UUID + canonical URI for repo, symbol, service, API, event, data, infra, deployment and team.","BUILD",U["alloydb"],None),
      ("F3","Assertion","Subject, predicate, object, qualifiers, confidence basis-points, winning evidence and bitemporal intervals.","BUILD",U["alloydb"],None),
      ("F4","Projection outbox","Assertion revision, mutation type, source watermark, graph/projector version and payload hash.","BUILD",U["alloydb_cdc"],None),
      ("F5","Spanner tables","Relational tables hold typed node/edge rows; property graph schema maps labels, keys and endpoints.","PAYG",U["spanner_schema"],U["spanner_price"]),
      ("F6","Projection receipt","APPLIED / STALE / TOMBSTONED / QUARANTINED, commit time, counts and graph snapshot ID.","BUILD",U["spanner"],None),
    ]
    row_cards(c,H-288,facts,112,7)
    section(c,H-320,"B / ATOMIC PROJECTOR","never hold an AlloyDB transaction while calling Spanner")
    rows=[
      ("01 CLAIM","lease rows by tenant + source partition + contiguous offset","expired lease recovers; empty claim is not successful work"),
      ("02 VALIDATE","schema/projector version, payload hash, epoch, nonempty batch, checked=total","unknown schema, mixed tenant or gap quarantines the partition"),
      ("03 IDENTITY","resolve canonical URI; proven rename keeps UUID; ambiguity creates ExternalEndpoint","no fuzzy-name merge; no internal DB row ID as durable identity"),
      ("04 RESOLVE","AlloyDB policy chooses winning/conflicted assertion by source precedence/confidence","Spanner never independently decides truth"),
      ("05 COALESCE","highest revision per logical_edge_id; deterministic endpoint/edge sort","prevents duplicate mutation and write contention"),
      ("06 WRITE","Spanner transaction upserts endpoints, node versions, edge head/version and tombstones","tenant in primary keys; stale revision ignored with receipt"),
      ("07 SNAPSHOT","record graph schema, projector digest, watermark vector and checked/total","reader pins minimum snapshot; stale response is explicit"),
      ("08 ACK","mark outbox disposition; advance contiguous checkpoint after Spanner commit","crash-before-ack replays safely via deterministic IDs"),
      ("09 REBUILD","new schema generation from canonical snapshot; compare counts/digests/queries","switch read alias only after full reconciliation"),
    ]
    y=table(c,M,H-345,[190,900,1210],["Step","Operation","Failure / invariant"],rows,40,24,5.7,{(5,1):U["spanner"],(8,1):U["spanner_schema"]})
    section(c,y-18,"C / NODE + EDGE SCHEMA","runtime observation never overwrites static structure")
    schema=[
      ("NODES","Repository, File, Symbol, Package, Service, ApiOperation, EventChannel, Schema, DataAsset, InfraResource, Deployment, Team","stable entity ID; version = repo SHA / contract digest / asset name / deployment UID"),
      ("STATIC CODE","CONTAINS, DEFINES, IMPORTS, CALLS, DEPENDS_ON, TESTS","compiler/AST first; heuristic parser second; source span + parser digest"),
      ("CONTRACT","EXPOSES_API, CALLS_API, PRODUCES_EVENT, CONSUMES_EVENT, HAS_SCHEMA","OpenAPI/protobuf/AsyncAPI/schema digest + producer/consumer coordinates"),
      ("DATA","READS, WRITES, DERIVES_FROM, MIGRATES","DDL/query/catalog lineage + environment + classification"),
      ("INFRA","DEPLOYS_FROM, RUNS_ON, ROUTES_TO, CONFIGURED_BY","Cloud Asset full name + IaC address + deployed artifact digest"),
      ("RUNTIME","OBSERVED_CALL, OBSERVED_ERROR, SATURATES, VIOLATES_SLO","bounded retention/rollups + time window + telemetry query"),
      ("OWNERSHIP","OWNS, MAINTAINS, APPROVES","directory/catalog evidence + valid/system intervals; missing owner blocks write"),
    ]
    table(c,M,y-43,[180,1120,1000],["Domain","Labels / relationships","Identity and proof"],schema,38,24,5.55,{(0,1):U["spanner_schema"],(4,1):U["cai"]})
    rect(c,M,48,W-2*M,43,AMBER2,AMBER,.8); para(c,M+12,75,"GOOGLE GAP: Gemini Enterprise search and Gemini Code Assist code customization are not a company semantic dependency graph. Parsers, canonical identities, bitemporal resolution, projection/rebuild control and the bounded Graph API are customer-built on Google Cloud.",W-2*M-24,"Mono-Bold",6.0,7.2,AMBER,2)
    footer(c,7)

def retrieval_context(c):
    header(c,8,"G-LLD-07","Chunk, vector, graph and signed ContextPack.","Authorize before retrieval; fuse lexical/vector candidates; expand exact graph paths; issue a hash-pinned, expiring context lease.","retrieval")
    section(c,H-155,"A / INGESTION-TO-CONTEXTPACK","embedding is a search hint; canonical IDs and source bytes remain evidence")
    pipe=[
      ("01","Structural atom","Parser emits symbol, class, config, test, API, schema or IaC resource - never arbitrary fixed-token slices.","BUILD",U["run_jobs"],None),
      ("02","Normalize","Canonical URI, repo SHA, span, language, parser digest, owner, class and ACL principals.","BUILD",U["bq"],None),
      ("03","Embed","Vertex embedding model, dimension and task type recorded; content hash keys the cache.","PAYG",U["vertex_embed"],U["model_price"]),
      ("04","Store","AlloyDB vector for ACL/hybrid search; Vertex Vector Search only when online scale proves necessary.","PAYG",U["alloydb_vector"],None),
      ("05","Graph join","Candidate canonical IDs expand through bounded Spanner Graph predicates and watermark.","PAYG",U["spanner"],U["spanner_price"]),
      ("06","Pack + sign","Gateway returns cited bytes, graph paths, policy, tests, limits and expiry; KMS signs digest.","BUILD",U["kms"],None),
    ]
    row_cards(c,H-288,pipe,112,7)
    section(c,H-320,"B / WORKED RECORDS","illustrative schema; values are versioned, validated and hash-pinned")
    gap=16; cw=(W-2*M-gap)/2; left=M; right=M+cw+gap; top=H-350; boxh=250
    rect(c,left,top-boxh,cw,boxh,PAPER2,INK,.7); txt(c,left+12,top-24,"SYMBOL_CHUNK V1 / ALLOYDB","Mono-Bold",6.2,BLUE)
    chunk="\n".join([
      "chunk_id: sha256(repo_sha|path|symbol|span|parser)",
      "uri: repo://corp/payments@a91f/src/pay.ts#authorize",
      "kind: function  language: typescript  span: 118:1-184:2",
      "source_hash: sha256:...  parser: tree-sitter-ts@digest",
      "summary: generated only after exact symbol extraction",
      "owner: group:payments-eng  data_class: PCI",
      "acl_allow: [group:payments-eng]  acl_deny: [suspended]",
      "embedding_model + dimensions + task_type + vector_hash",
      "graph_entity_id: 6f2...  repo_sha: a91f...",
      "valid_from + system_from + deleted_at",
      "evidence_uri: gs://bucket/object#generation",
    ])
    para(c,left+12,top-44,chunk,cw-24,"Mono",6.2,8.4,INK,20)
    rect(c,right,top-boxh,cw,boxh,PAPER2,INK,.7); txt(c,right+12,top-24,"CONTEXTPACK V1 / RESPONSE","Mono-Bold",6.2,GREEN)
    pack="\n".join([
      "pack_id + schema_version + tenant + purpose",
      "change_id + risk + allowed_actions + max_cost_cents",
      "subject: repo/file/symbol/service IDs",
      "repo_pins: [{repo, base_sha, expected_head_sha}]",
      "graph: {schema, projector, watermark_vector}",
      "retrieval: {query_hash, lexical_k, vector_k, reranker}",
      "chunks: [{id, source_uri, span, source_hash, score}]",
      "paths: [{typed_edges, assertion_ids, confidence_bp}]",
      "contracts/tests/standards/owners + missing_critical[]",
      "acl + policy + tool-catalog decision hashes",
      "checked + total + created_at + expires_at",
      "kms_key_version + signature + reproducibility_manifest",
    ])
    para(c,right+12,top-44,pack,cw-24,"Mono",6.2,8.4,INK,20)
    section(c,top-boxh-24,"C / ACL-FIRST HYBRID QUERY","never retrieve cross-tenant candidates and filter afterward")
    steps=[
      ("1 AUTHZ","Resolve groups, purpose, tenant, repo allowlist and classification ceiling before query."),
      ("2 LEXICAL","Full-text candidates inside ACL and pinned repo/version filters."),
      ("3 VECTOR","AlloyDB ScaNN/HNSW with inline metadata filters; exact KNN for recall evaluation."),
      ("4 FUSE","Reciprocal-rank fusion; dedupe canonical entity/version; reject stale/deleted IDs."),
      ("5 EXPAND","Spanner Graph 1-3 hop allowlisted predicates; cap paths/nodes/time; return explanation edges."),
      ("6 RERANK","Authority, freshness, structural proximity, tests, runtime evidence and relevance."),
      ("7 PROVE","Re-read source bytes by URI+generation; verify hash/span; publish checked/total and gaps."),
      ("8 LEASE","KMS-sign digest; bind actor/task/base SHAs/tools; downstream change invalidates."),
    ]
    y=table(c,M,top-boxh-49,[180,1120,1000],["Step","Atomic behavior","Failure rule"],[(a,b,"fail closed on empty, stale, unauthorized or unverified input") for a,b in steps],35,24,5.5,{(2,1):U["alloydb_vector"],(3,1):U["alloydb_hybrid"],(4,1):U["spanner"]})
    section(c,y-17,"D / EVALUATION + CAPACITY GATES","publish denominator and corpus version by language, repo and risk tier")
    evalrows=[
      ("RECALL","symbol recall@10; affected-service path recall; missed critical edge rate","exact search + seeded dependency fixtures"),
      ("SECURITY","cross-tenant/ACL leakage; deleted-document resurrection; purpose escalation","must equal zero using adversarial principals/tombstones"),
      ("FRESHNESS","hook-to-query lag P50/P95/P99; stale-pack refusal rate","separate parse, embed, graph and serving watermarks"),
      ("SCALE","chunks, dimensions, index build, QPS, P95, graph cap, cost/query","benchmark AlloyDB vs Vector Search before dual-store complexity"),
    ]
    table(c,M,y-42,[220,1050,1030],["Gate","Measure","Pass evidence"],evalrows,38,24,5.7,{(0,1):U["evals"],(3,1):U["alloydb_vector"]})
    footer(c,8)

def knowledge(c):
    header(c,6,"G-LLD-05","Continuous code, contract, data and infrastructure ingestion.","Gemini Enterprise indexes documents; the SDLC still needs a version-pinned structural fact pipeline across every repository and deployed service.","knowledge")
    section(c,H-155,"CONTINUOUS INGESTION","Deterministic parsers create facts; models summarize only after exact identities exist.")
    items=[("K1","Pinned snapshot","Cloud Run Job clones exact SHA; Cloud Storage Bucket Lock retains immutable source manifest.","PAYG",U["storage"],U["run_price"]),("K2","Parse + normalize","Customer parser containers emit symbols/imports/calls/tests/APIs/events/DB/IaC/owners with parser version.","BUILD",U["run_jobs"],None),("K3","Cloud reality","Cloud Asset Inventory export/feed maps projects, GKE, Cloud Run, IAM and resources; relationship access may need SCC.","PAYG",U["cai"],U["cai_price"]),("K4","Historical facts","BigQuery append-only tables hold every repo snapshot, edge, deletion, quality metric and evaluation result.","PAYG",U["bq"],None),("K5","Operational graph","Spanner Graph maps relational tables to typed nodes/edges; Enterprise/Plus Spanner edition required.","PAYG",U["spanner"],U["spanner_price"]),("K6","Semantic retrieval","Vertex AI embeddings + Vector Search 2.0; vectors always resolve to canonical symbol/node IDs.","PAYG",U["vector"],None)]
    row_cards(c,H-288,items,112,7)
    section(c,H-320,"SOURCE-TO-FACT MATRIX","every extractor emits canonical IDs, evidence coordinates, parser/tool digest and checked/total")
    matrix=[
      ("CODE","compiler/Kythe where supported; language server/AST; Tree-sitter fallback","Symbol/File/Package","DEFINES, CALLS, IMPORTS, IMPLEMENTS, TESTS"),
      ("BUILD","lockfiles, manifests, build graph, generated sources","Package/Target/Artifact","DEPENDS_ON, BUILDS_WITH, GENERATES"),
      ("API","API Hub, OpenAPI, protobuf, GraphQL","Api/Version/Operation/Schema","EXPOSES, CALLS_API, IMPLEMENTS_API"),
      ("EVENT","AsyncAPI, protobuf/schema registries, broker config","EventChannel/EventSchema","PRODUCES, CONSUMES, HAS_SCHEMA"),
      ("DATABASE","DDL, migrations, ORM metadata, query/static/runtime evidence","Database/Schema/Table/Column/Migration","READS, WRITES, MIGRATES, USES_COLUMN"),
      ("DATA","Knowledge Catalog entries/aspects and lineage","Dataset/Table/Pipeline","PRODUCES, DERIVES_FROM, CONSUMES"),
      ("INFRA","Terraform plan/state, KRM/Kubernetes, Cloud Asset Inventory feed","Module/Resource/Deployment","PROVISIONS, RUNS_ON, ROUTES_TO, CONFIGURED_BY"),
      ("RUNTIME","Trace/Service Mesh/logs/deploy history","Service/Deployment/Endpoint","OBSERVED_CALL, OBSERVED_ERROR, DEPLOYS_ARTIFACT"),
      ("OWNERSHIP","repo owners, directory groups, service catalog, approver policy","Team/Principal/Standard","OWNS, MAINTAINS, APPROVES"),
    ]
    y=table(c,M,H-345,[170,800,460,870],["Plane","Inputs / extractor","Nodes","Edges"],matrix,41,24,5.45,{(2,1):"https://docs.cloud.google.com/apigee/docs/apihub/api-supply-chain",(5,1):U["knowledge"],(6,1):U["cai_feed"],(7,1):U["trace"]})
    section(c,y-18,"ATOMIC SNAPSHOT + INCREMENTAL LOOP","repository count is not capacity; measure files, symbols, chunks, edges and update rate")
    ingest=[
      ("01 EVENT","receive default-branch/PR/release/delete/config event; dedupe provider delivery ID"),
      ("02 PIN","resolve immutable commit SHA, source ACL snapshot and parent manifest; never index branch name as identity"),
      ("03 SNAPSHOT","private build/job fetches exact source; writes compressed source + manifest to versioned/locked GCS"),
      ("04 DIFF","compare manifests; reparse changed files plus dependency/build/generated closure"),
      ("05 DISPATCH","language/build workers run pinned containers with CPU/memory/time/egress budgets"),
      ("06 FACTBATCH","nodes, edges, chunks, ACL domains, tombstones and evidence; schema/tool/parser versions"),
      ("07 VALIDATE","nonempty, referential integrity, source hash, unresolved threshold, checked=total, tenant isolation"),
      ("08 COMMIT","append canonical facts/outbox in authority transaction; no cross-database transaction"),
      ("09 PROJECT","Spanner Graph, vector/full-text, BigQuery history, Knowledge Catalog and Gemini app summaries"),
      ("10 VERIFY","manifest/file/symbol/edge/chunk/delete counts and digests; publish freshness per plane"),
    ]
    y2=table(c,M,y-43,[190,2110],["Step","Exact behavior"],ingest,34,24,5.55,{(2,1):U["build_private"],(8,1):U["spanner"]})
    section(c,y2-18,"LANGUAGE + CONFIDENCE POLICY","Google has no managed polyglot semantic code-graph product")
    rules=[
      ("SEMANTIC","compiler/typechecker/Kythe-resolved","confidence=1,000,000 ppm; accepted for release impact"),
      ("SYNTACTIC","AST/Tree-sitter/import resolver","typed as syntactic; accepted only by language/risk policy"),
      ("RUNTIME","trace/mesh/query observation","time-windowed evidence; absence never proves no dependency"),
      ("MODEL PROPOSAL","LLM-suggested missing relationship","PROPOSED; not accepted for release until corroborated"),
      ("UNKNOWN","unsupported language/generated/dynamic/reflection edge","blocks critical autonomy or requires owner waiver with expiry"),
    ]
    table(c,M,y2-43,[230,650,1420],["Class","Source","Use"],rules,37,24,5.55)
    rect(c,M,48,W-2*M,40,AMBER2,AMBER,.8); txt(c,M+14,63,"GEMINI APP PROJECTION: publish human-oriented service cards, ADRs, runbooks, API/SLO summaries and approved learnings - never flatten the authoritative dependency graph into documents.","Mono-Bold",5.8,AMBER)
    footer(c,6)

def multirepo(c):
    header(c,9,"G-LLD-08","Clarification, multi-repo impact and release DAG.","For 30 repositories and 100+ services, SCCs, compatibility waves and compensation replace the fiction of one atomic multi-repo change.","multirepo")
    section(c,H-155,"GRAPH PROJECTION","A ticket is projected onto code, contracts, data and runtime — not sent directly to an implementor.")
    nodes=[("I1","Seed entities","ticket terms, named service, files, stack trace, SLO and recent deploy SHA.","BUILD",U["apps"],None),("I2","Resolve identity","map source URI → repo@SHA → symbol → owning service → deployed asset.","BUILD",U["spanner"],None),("I3","Traverse typed edges","CALLS / IMPORTS / EXPOSES / CONSUMES / READS / WRITES / DEPLOYS / OWNS.","PAYG",U["spanner"],U["spanner_price"]),("I4","Find cycles","strongly connected components become atomic change groups; no parallel edits inside a cycle.","BUILD",U["spanner"],None),("I5","Order waves","schema producers → tolerant consumers → traffic migration → cleanup; attach rollback per wave.","BUILD",U["deploy"],None),("I6","Quantify coverage","checked repos/total, symbols parsed, unresolved edges, stale assets and confidence distribution.","BUILD",U["bq"],None)]
    row_cards(c,H-288,nodes,112,7)
    section(c,H-320,"EXAMPLE IMPACT DAG","Every edge is backed by a source span, contract, asset record or runtime trace.")
    y=H-470
    levels=[[("ticket","PAY-418","BUILD")],[("repo","payments-api","BUILD"),("repo","risk-engine","BUILD")],[("contract","auth.v3","HUMAN"),("db","payments.auth","HUMAN"),("event","auth_scored","HUMAN")],[("svc","checkout","PAYG"),("svc","fraud-worker","PAYG"),("dash","payment-slo","PAYG")]]
    xs=[]
    for li,level in enumerate(levels):
        yy=y-li*82; bw=170; gap=28; total=len(level)*bw+(len(level)-1)*gap; start=W/2-total/2; centers=[]
        for j,(kind,name,status) in enumerate(level):
            x=start+j*(bw+gap); color,fill=PAL[status]; rect(c,x,yy,bw,52,fill,color,.8); txt(c,x+10,yy+34,kind.upper(),"Mono-Bold",5.3,color); txt(c,x+10,yy+14,name,"Display-Bold",8,INK); centers.append((x+bw/2,yy+52,x+bw/2,yy))
        xs.append(centers)
    for li in range(len(xs)-1):
        for a in xs[li]:
            for b in xs[li+1]: arrow(c,[(a[2],a[3]),(a[2],a[3]-12),(b[0],b[1]+12),(b[0],b[1])],MUTED2,.6)
    section(c,900,"PLAN BEFORE CODE","reviewer and acceptance oracle are assigned before implementors; clarification changes intent version")
    planrows=[
      ("CLARIFY","missing AC/NFR/owner/data/rollback/security/contract/infra facts become typed questions","post through Jira/GitHub; wait state in AlloyDB; no open workflow for multi-day response"),
      ("IMPACT","pin graph snapshot; enumerate candidate/checked repos; classify exact/observed/proposed/unknown edges","unknown critical dependency blocks; owner may approve an expiry-bound discovery waiver"),
      ("SCC","Tarjan/Kosaraju over write/contract dependency subgraph; each strongly connected component is one change group","members serialized or coordinated; no independent merge inside the cycle"),
      ("WAVES","topologically order expand contract producer, tolerant consumers, traffic/data migration and cleanup","each wave has preconditions, proof, rollback/forward-fix and compatibility window"),
      ("REVIEWER","select independent identity/model/context; store acceptance/manual/abuse/performance/security oracle digest","oracle cannot read implementation diff until review; context/policy change invalidates it"),
      ("CHILDREN","one child per repo/write-set with base SHA, ContextPack, todo DAG, test map, budget, lease and reviewer","overlapping write sets are serialized; child cannot merge or deploy"),
    ]
    yplan=table(c,M,875,[220,1120,960],["Stage","Atomic output","Authority / failure"],planrows,42,24,5.55,{(2,1):U["spanner"],(3,1):U["deploy"]})
    section(c,yplan-18,"MULTI-REPO SAGA","no distributed Git transaction; parent records and reconciles every external effect")
    saga=[
      ("01 CREATE","persist DAG/SCC/waves and deterministic child IDs","parent state/version + plan/oracle/context digests"),
      ("02 DISPATCH","CAS child PENDING->RUNNING; reserve repo/write-set/WIP/cost; issue fencing token","duplicate/expired lease cannot write"),
      ("03 FAN-IN","verify child base/head SHA, receipts, tests, diff and artifact hashes","partial success does not advance parent"),
      ("04 WAVE PR","assemble integration branch/PR for ready SCC or compatibility wave","recompute graph impact and conflicts against latest target"),
      ("05 PARTIAL FAIL","stop successors; classify untouched/reversible/irreversible/unknown effects","unknown becomes BLOCKED; never automatic success"),
      ("06 REPAIR","compensate, revert PR or forward-fix under protected authority","database/schema uses explicit expand-contract plan"),
      ("07 RECONCILE","authority state vs Workflows vs SCM/CI/deploy objects vs evidence manifests","counts/digests/checked-total before close"),
    ]
    ysaga=table(c,M,yplan-43,[180,980,1140],["Step","Operation","Required proof"],saga,37,24,5.45,{(0,1):U["workflows"],(3,1):U["ssm_pr"]})
    section(c,ysaga-18,"AUTONOMY REFUSAL RULES","unknown is not low risk")
    rules=[("R1","checked_repos < total_candidate_repos","refuse plan; run targeted re-index"),("R2","unresolved critical edge > 0","ask owner or create discovery task"),("R3","graph snapshot older than source SHA","refuse execution; reconcile"),("R4","cycle has no joint rollback","require architecture approval"),("R5","data migration lacks expand/contract","require human DBA approval")]
    table(c,M,ysaga-43,[150,700,1450],["Rule","Condition","Action"],rules,30,22,5.6)
    footer(c,9)

def agents(c):
    header(c,10,"G-LLD-09","Agent topology: Gemini app, Agent Platform, ADK and deterministic orchestration.","One controller owns state; specialists are replaceable workers; every tool call crosses identity, policy, budget and evidence controls.","agents")
    section(c,H-155,"THREE DISTINCT PLANES","Do not collapse these into one box called Gemini Enterprise.")
    planes=[("P1","Gemini Enterprise app","Employee UI, search, connected actions, Agent Gallery, sharing and basic governance.","APP",U["agents"],None),("P2","Agent Platform","ADK/Studio build; Agent Runtime; sessions/memory; evaluation; Registry/Gateway; observability.","PAYG",U["agent_platform"],U["agent_price"]),("P3","Delivery control","Workflows + Pub/Sub + BigQuery state + Fleet + Cloud Build + protected SCM authority.","BUILD",U["workflows"],U["workflows_price"])]
    row_cards(c,H-288,planes,112,16)
    section(c,H-320,"SPECIALIST CONTRACTS","One state-machine entry point; specialists never self-approve.")
    specs=[("T0","INTAKE","normalize ticket / alert → ChangeIntent"),("T1","CLARIFIER","questions + explicit assumptions; no code"),("T2","IMPACT","repo/service DAG + graph coverage receipt"),("T3","PLANNER","atomic todos + test oracle + rollback + budget"),("T4","REVIEWER","acceptance oracle before implementation; separate identity"),("T5","IMPLEMENTOR","one repo/worktree/write-set; tests with each todo"),("T6","VERIFIER","fresh context; adversarial impact + manual scenario list"),("T7","RELEASER","artifact promotion proposal; never owns final authority"),("T8","REPAIR","incident evidence → new ChangeIntent; same proof path")]
    bw=(W-2*M-2*10)/3
    for i,(code,name,body) in enumerate(specs):
        col=i%3; row=i//3; x=M+col*(bw+10); y=H-456-row*78; card(c,x,y,bw,66,code,name,body,"PAYG",U["adk"],U["agent_price"])
    section(c,1035,"DETERMINISTIC INVOCATION PATH","Agent Runtime executes reasoning; the controller authorizes every state/tool boundary")
    callrows=[
      ("01 CLAIM","dispatcher CASes stage + attempt + lease/fence in AlloyDB","duplicate/expired worker receives refusal receipt"),
      ("02 IDENTITY","launch dedicated runtime/service account for one role and trust domain","no user bearer token or shared production principal"),
      ("03 CONTEXT","Context Broker verifies signed pack, actor, purpose, TTL, base SHAs and policy digest","stale/unauthorized pack aborts before model call"),
      ("04 PROMPT","ADK instruction + typed input schema + acceptance oracle reference + allowed action catalog","prompt cannot expand IAM/tool scope"),
      ("05 MODEL","configured Gemini model returns structured proposal; model/token/cache identifiers recorded","model prose is untrusted until schema/evidence checks"),
      ("06 TOOL","Agent Gateway/approved MCP validates tool, resource, arguments, purpose, budget and idempotency","custom MCP from app may bypass Gateway and therefore stays out of privileged path"),
      ("07 CHECKPOINT","worker emits intermediate receipt/heartbeat with fence, cost and evidence references","timeout/cancel leaves recoverable state, not assumed failure/success"),
      ("08 COMPLETE","typed StageReceipt: outcome, checked/total, outputs/hashes, unresolved facts and refusal code","controller independently verifies before transition"),
      ("09 EVALUATE","traces and outcomes join BigQuery eval dataset; raw sensitive prompts follow explicit retention","agent version promotion is separate learning pipeline"),
    ]
    ycall=table(c,M,1010,[180,1260,860],["Step","Runtime behavior","Guard"],callrows,38,24,5.5,{(3,1):U["adk"],(5,1):U["gateway"],(8,1):U["evals"]})
    section(c,ycall-18,"RUNTIME ENVELOPE","enforced around every specialist invocation")
    rows=[("IDENTITY","dedicated service account + workload identity; no shared user token","principal + token audience + expiry"),("TOOLS","Gateway / approved MCP / connector allowlist; deny-by-default","tool+resource+args hash and policy verdict"),("CONTEXT","signed ContextPack; pinned graph/source snapshot; TTL; tenant ACL","pack hash + authorization decision"),("BUDGET","integer max tokens, wall time, tool calls, compute and cents","reservation, actuals and overage refusal"),("OUTPUT","typed schema + citations + checked/total + refusal/receipt","invalid/empty schema cannot advance"),("OBSERVABILITY","trace links ticket -> agent -> tool -> commit -> build -> deploy -> SLO","trace is index; evidence store remains proof"),("DATA","Sensitive Data Protection/classification; prompt logging explicit and minimized","no secret/PII in immutable evidence")]
    yenv=table(c,M,ycall-43,[260,1300,740],["Envelope","Required field","Evidence"],rows,35,22,5.55,{(1,1):U["gateway"],(5,1):U["trace"],(6,1):U["dlp"]})
    rect(c,M,48,W-2*M,43,AMBER2,AMBER,.8); txt(c,M+14,64,"BOUNDARY: the Gemini Enterprise app can list and invoke a registered agent; Agent Runtime/model/tools remain PAYG, and the app does not own the SDLC lease, state transition, evidence validation or merge/deploy authority.","Mono-Bold",5.75,AMBER)
    footer(c,10)

def fleet(c):
    header(c,11,"G-LLD-10","Fleet inside Google Cloud: atomic isolated implementation.","Fleet is customer-built; Google supplies execution, identity, source, CI, artifact and release primitives around the per-repo enforcement kernel.","fleet")
    section(c,H-155,"PLACEMENT","Workflows parent → Pub/Sub child command → GKE/Cloud Run Job Fleet pod → protected source branch.")
    items=[("F1","Lease","AlloyDB authority claims repo@baseSHA + write-set + TTL + fencing token; duplicate/stale lease refused.","BUILD",U["alloydb"],None),("F2","Sandbox","GKE pod or Cloud Run Job; pinned image digest; no host path; egress allowlist; ephemeral disk.","PAYG",U["gke"],U["gke_price"]),("F3","Checkout","Secure Source Manager or GitHub credential; exact base SHA; worktree per child lane; no merge scope.","BUILD",U["ssm"],None),("F4","Invoke","Autonomous Gemini CLI/ADK uses Vertex AI workload identity and PAYG inference; Standard Code Assist is for licensed humans.","PAYG",U["agent_runtime"],U["model_price"]),("F5","Verify local","Run changed tests after each todo; reject zero tests; emit receipts through parent-owned typed channel.","BUILD",U["build"],None),("F6","Publish","Commit and push child branch; parent verifies ancestry/diff/receipts and assembles integration wave.","BUILD",U["kms"],None)]
    row_cards(c,H-288,items,112,7)
    section(c,H-320,"ATOMIC TODO LOOP","Model text never advances state; only parsed receipts do.")
    steps=[("01","select next todo"),("02","materialize minimal context"),("03","edit bounded write-set"),("04","run targeted test"),("05","inspect diff + secrets"),("06","emit receipt"),("07","update lease"),("08","repeat or refuse")]
    bw=(W-2*M-7*8)/8
    for i,(n,s) in enumerate(steps):
        x=M+i*(bw+8); rect(c,x,H-442,bw,88,PAPER2,BLUE,.7); txt(c,x+10,H-382,n,"Display-Bold",17,BLUE); para(c,x+10,H-405,s,bw-20,"Display-Bold",7.5,9,INK,3)
        if i<7: arrow(c,[(x+bw,H-398),(x+bw+8,H-398)])
    section(c,H-470,"CHILD EXECUTION MANIFEST","parent issues immutable work; worker receives no authority database, ledger or reusable merge credential")
    manifest=[
      ("IDENTITY","change_id, child_id, attempt_id, repo_id, role, service account, fencing token, expiry"),
      ("SOURCE","provider/repository, base SHA, expected remote head, branch name, allowed path/write set"),
      ("CONTEXT","ContextPack URI/hash/signature, graph snapshot, plan/todo/oracle/policy/tool-catalog digests"),
      ("BUDGET","integer wall seconds, CPU/memory/disk, model tokens, tool calls, network bytes and max cents"),
      ("SANDBOX","pinned image digest, non-root/read-only root, ephemeral volume, seccomp, egress destinations"),
      ("TOOLS","Gemini CLI/ADK adapter, git proxy, test runners, approved MCP endpoints and exact versions"),
      ("OUTPUT","commit SHA, diff hash, test/gate receipts, artifacts, unresolved list, cost and typed terminal outcome"),
      ("FORBIDDEN","authority DB path, state dir, evidence signer, merge/deploy credential, broad shell/network/secret list"),
    ]
    yman=table(c,M,H-495,[240,2060],["Group","Required values"],manifest,37,24,5.55,{(4,1):U["gke"],(5,1):U["code"]})
    section(c,yman-18,"WORKTREE + FAN-IN","Cloud Run tasks do not share disk; coordinated multi-repo work uses one bounded GKE lane or separate branches")
    fan=[
      ("PREPARE","clone exact repo SHA into ephemeral disk; verify object hash; create .worktrees/<child>; share build cache only through controlled volume/cache","dirty or mismatched base -> stale refusal"),
      ("PARALLELIZE","todo DAG lanes may run only when write sets and generated outputs are disjoint; one lease/fence per lane","overlap -> serialize or create SCC child"),
      ("CHECKPOINT","commit/push after each proven todo or bounded batch; upload receipt before worker can exit","ephemeral loss cannot erase accepted evidence"),
      ("FAN-IN","parent fetches child commits; validates ancestry, diff scope, receipt hashes and latest target; merges to integration branch","child success alone never marks parent complete"),
      ("CLEANUP","revoke token, delete pod/disk/worktree, release lease/reservation; retain branch/evidence by policy","cleanup failure is observable but never erases proof"),
    ]
    yfan=table(c,M,yman-43,[230,1570,500],["Stage","Exact operation","Failure"],fan,40,24,5.45,{(0,1):U["run_jobs"],(3,1):U["ssm_pr"]})
    section(c,yfan-18,"FAILURE -> CAUSE -> FIX","Fleet converts surprises into typed outcomes")
    failures=[("ENVIRONMENT","tool/compiler unavailable","environment_fault; do not blame agent"),("AMBIGUOUS","empty/contradictory input","write refusal receipt; question ticket"),("STALE","base SHA/context changed","cancel lease; rebuild ContextPack"),("CONFLICT","overlapping write-set","serialize SCC/wave or escalate"),("VERIFY","test/evidence mismatch","keep branch; no PR-ready transition"),("BUDGET","time/token/compute exceeded","terminate pod; preserve receipt; re-plan"),("SECURITY","scope/egress/secret/policy violation","kill lane; quarantine evidence; page owner")]
    table(c,M,yfan-43,[220,760,1320],["Type","Cause","Deterministic response"],failures,31,22,5.55)
    footer(c,11)

def prove(c):
    header(c,12,"G-LLD-11","Independent review, deterministic CI evidence and protected merge.","The implementing model never certifies its own work; every gate binds exact inputs, tool version, denominator, exit code and immutable evidence.","prove")
    section(c,H-155,"PR PATH","Fresh reviewer context is assembled from the diff and graph — never inherited from the implementor.")
    items=[("V1","Open PR","Fleet publishes branch + plan + ContextPack hash + todo receipts + risk tier.","BUILD",U["ssm"],None),("V2","Recompute impact","Verifier reads diff, resolves changed symbols and traverses reverse graph edges.","BUILD",U["spanner"],None),("V3","Manual scenarios","Agent lists user journeys, operational checks and failure injections; humans execute high-risk cases.","HUMAN",U["code_agent"],None),("V4","Cloud Build","PR trigger uses dedicated service account; approval where required; status posted to branch.","PAYG",U["build_trigger"],U["build_price"]),("V5","Branch protection","Require successful checks, reviewers/code owners, resolved comments, non-stale approval.","HUMAN",U["ssm_branch"],None)]
    row_cards(c,H-288,items,112,8)
    section(c,H-320,"DETERMINISTIC GATE MATRIX","Each gate publishes checked/total, evidence URI, tool version and exit code.")
    rows=[("BUILD","compiler / package / lockfile / reproducibility","Cloud Build private pool when VPC isolation is required","0 inputs = fail"),("TEST","unit + integration + contract + migration + e2e","Cloud Build + ephemeral GKE/Cloud Run dependencies","coverage and selected tests recorded"),("QUALITY","language-native lint/static rules + duplication/complexity policy","customer containers in Cloud Build","Google has no SonarQube-equivalent code-quality gate"),("SECURITY","secret scan + SAST/DAST harness + dependency/container vulnerability","Artifact Analysis + SCC + customer scanners","critical finding blocks"),("SUPPLY CHAIN","SBOM/provenance/attestation + image digest","Artifact Registry + Binary Authorization","unsigned digest denied"),("PERFORMANCE","Lighthouse + benchmark + load/capacity harness","Google Lighthouse in Cloud Build; workload on GKE/Cloud Run","P95/regression threshold blocks"),("POLICY","risk, data class, owners, approvals, rollout and rollback","Organization Policy + custom policy service","agent cannot waive")]
    links={(0,2):U["build"],(3,2):U["analysis"],(4,2):U["binary"],(5,2):U["lighthouse"],(6,2):U["org_policy"]}
    y=table(c,M,H-350,[180,650,900,570],["Gate","Evidence","Google-native execution","Fail rule"],rows,44,24,5.8,links)
    section(c,y-18,"GATE RECEIPT V1","company contract - not a Google-managed schema")
    receipt=[
      ("SUBJECT","change/child/repo, base SHA, head SHA, PR ID, artifact digest, graph/context/plan/oracle/policy hashes"),
      ("EXECUTION","Cloud Build ID, trigger/revision, worker pool, service account, builder image digest, start/end"),
      ("GATE","gate ID/profile, tool/version/config digest, exact invocation, required/selected input manifest"),
      ("RESULT","PASS/FAIL/ERROR/REFUSED/SKIPPED; exit type/code; checked/total; thresholds and observed values"),
      ("EVIDENCE","GCS URI + generation + SHA-256 + media type; JUnit/SARIF/SBOM/provenance/LHCI/load outputs"),
      ("AUTHORITY","actor, approval digest/expiry, branch rule version, attestor, signature, causation/correlation IDs"),
    ]
    yr=table(c,M,y-43,[250,2050],["Group","Required fields"],receipt,36,24,5.55,{(1,1):U["build"],(4,1):U["build_provenance"]})
    section(c,yr-18,"SOURCE -> BUILD -> ARTIFACT -> DEPLOY CHAIN","three GitHub integrations remain separate; Secure Source Manager is the Google-native SCM option")
    chain=[
      ("PR EVENT","SSM pull_request trigger or Cloud Build GitHub App/Developer Connect","exact PR head SHA + trusted/external contributor control","starts PR qualification only"),
      ("AI REVIEW","Gemini Code Assist GitHub Preview or read-only Gemini CLI reviewer for SSM","fresh diff/graph context + typed findings + receipt","advisory; known file/logging limitations"),
      ("HUMAN REVIEW","CODEOWNERS/reviewer group; approval bound to current head/evidence","new commit invalidates approval; comments resolved","agent role excludes approve/merge"),
      ("RELEASE BUILD","protected-main push; private pool; dedicated release SA; requested provenance verification","image listed under top-level images; full evidence stored externally","explicit docker push alone does not create the same provenance record"),
      ("QUALIFY","Artifact Analysis/SCC Artifact Guard + custom broad-language scanners + policy controller","production-qualified attestation after all gates","Artifact Guard is container-focused Preview/SCC paid tier; not general SAST"),
      ("ADMIT","Binary Authorization requires attestation for immutable image digest","coding agent has no signing key; breakglass is human","built-by attestor proves builder, not all quality gates"),
    ]
    yc=table(c,M,yr-43,[210,900,800,390],["Stage","Google execution","Evidence","Boundary"],chain,42,24,5.35,{(0,1):U["build_trigger"],(3,1):U["build_private"],(4,1):U["analysis"],(5,1):U["binary"]})
    rect(c,M,53,W-2*M,48,RED2,RED,.8); txt(c,M+14,82,"MERGE AUTHORITY", "Mono-Bold",6,RED); para(c,M+130,83,"The Gemini Enterprise GitHub connector can technically merge a PR. Production design should remove or policy-gate that permission and retain protected-branch, required-human or independently delegated authority for irreversible changes.",W-2*M-150,"Display",6.5,8,MUTED,2)
    footer(c,12)

def release(c):
    header(c,13,"G-LLD-12","Release, SLO, capacity, incident repair and disaster recovery.","Cloud Deploy changes traffic; Cloud Monitoring proves health; every runtime finding re-enters the same protected lifecycle.","release")
    section(c,H-155,"PROMOTION STATE MACHINE","Artifact identity stays constant from staging through production.")
    items=[("R1","Artifact","Cloud Build writes immutable image/package to Artifact Registry with provenance and SBOM.","PAYG",U["artifact"],None),("R2","Admission","Binary Authorization verifies attestor/policy before workload admission.","PAYG",U["binary"],None),("R3","Staging","Cloud Deploy renders and deploys to GKE/Cloud Run; smoke/contract/data migration verification.","PAYG",U["deploy"],U["deploy_price"]),("R4","Canary","1% → 5% → 25% → 50% → 100%; each hold evaluates SLO and business KPI windows.","PAYG",U["deploy"],None),("R5","Authority","Human or pre-authorized policy approves production wave; higher risk always requires human.","HUMAN",U["ssm_branch"],None),("R6","Rollback","Cloud Deploy rollback or traffic reversal; database rollback follows explicit expand/contract plan.","BUILD",U["deploy"],None)]
    row_cards(c,H-288,items,112,7)
    section(c,H-320,"SLO + CAPACITY LOOP","Alerts are typed evidence with denominator, window and burn rate.")
    rows=[("DEFINE","SLI query, SLO target, window, budget and owner","Cloud Monitoring SLO / custom metrics"),("BASELINE","pre-change P50/P95/P99, throughput, saturation, cost/request","Monitoring + Logging + Trace"),("LOAD","representative traffic model, concurrency, payload mix, warm/cold, failure injection","customer harness on GKE/Cloud Run"),("COMPARE","confidence interval + regression budget + capacity headroom","BigQuery evaluation table"),("CANARY","short/long burn alerts + business KPI + error class","Cloud Monitoring alert policy"),("DECIDE","continue / pause / rollback; decision receipt stored with artifact","Workflows + Cloud Deploy"),("LEARN","incident, trace and fix linked to graph node and eval corpus","BigQuery + Spanner Graph + Knowledge Catalog")]
    links={(0,2):U["slo"],(1,2):U["monitor"],(4,2):U["monitor"],(5,2):U["workflows"],(6,2):U["knowledge"]}
    y=table(c,M,H-350,[180,1200,920],["Step","Exact requirement","Google service"],rows,39,24,6.0,links)
    section(c,y-18,"CLOUD DEPLOY RELEASE CONTRACT","one immutable digest; each phase emits job-run and verification evidence")
    deployrows=[
      ("CREATE RELEASE","source/release build submits Skaffold render + immutable artifact digest + config/policy hashes","Cloud Deploy release ID; render/build logs; provenance"),
      ("STAGING","deploy + predeploy migrations + smoke/contract/synthetic/data verification","rollout/phase/job-run IDs + gate receipts"),
      ("APPROVAL","prod target requireApproval; human group owns clouddeploy.approver; approval binds rollout/evidence","Required/Approved event + actor + timestamp + digest"),
      ("CANARY","configured percentages create phases; verification after each phase; hold window uses Monitoring queries","traffic %, start/end, request denominator, SLO/business/security decision"),
      ("FAIL","verification nonzero pauses/fails phase; repair rule may retry job or create rollback to last success","failure evidence; no empty metric interpreted as healthy"),
      ("ROLLBACK","rollback creates a new rollout; traffic/code returns to known digest; DB uses compatibility plan","new rollout ID + previous/target digests + compensation receipt"),
      ("CLOSE","observe outcome window; link runtime version/trace/SLO to ticket, PR, build, artifact and rollout","OutcomeAccepted receipt + learning proposal"),
    ]
    yd=table(c,M,y-43,[230,1310,760],["Phase","Atomic behavior","Evidence"],deployrows,42,24,5.5,{(0,1):U["deploy"],(2,1):U["deploy_approval"],(3,1):U["deploy_canary"]})
    section(c,yd-18,"DATABASE + CONTRACT MIGRATION","rollback is not always reverse DDL")
    mig=[
      ("EXPAND","add compatible schema/API/event version; deploy tolerant readers/writers","backward/forward compatibility tests and capacity headroom"),
      ("MIGRATE","backfill with checkpoint, idempotency and reconciliation; observe lag/error/saturation","checked/total rows, failed keys, resume cursor and cost"),
      ("SWITCH","move consumers/traffic by graph-ordered wave; pin schema/artifact versions","consumer readiness and rollback window"),
      ("CONTRACT","remove old path only after telemetry proves zero/approved residual use","owner approval + dependency graph snapshot + tombstones"),
      ("FAILURE","choose traffic rollback, forward fix or restore by compatibility manifest","no generic agent-issued DROP/restore"),
    ]
    ym=table(c,M,yd-43,[220,1260,820],["Stage","Method","Gate"],mig,39,24,5.55)
    section(c,ym-18,"END-TO-END TRACEABILITY","do not keep one distributed trace open for days")
    trace=[("ticket/source revision","change/child/attempt IDs","repo base/head + PR","Cloud Build + evidence digest","artifact/provenance/attestation","Cloud Deploy rollout/phase","runtime service.version + SLO outcome")]
    table(c,M,ym-43,[300,300,320,340,360,330,350],["Intent","Control","Source","CI","Artifact","Release","Runtime"],trace,55,24,5.4)
    footer(c,13)

def operations_dr(c):
    header(c,14,"G-LLD-13","Runtime signals, capacity control, autonomous repair and disaster recovery.","The diagnostic model is read-only. A deterministic catalog and separate executor identity own bounded mutation, rollback and restore.","operations")
    section(c,H-155,"A / SIGNAL PRODUCERS","all events normalize to incident.v1 before correlation")
    signals=[
      ("O1","SLO burn","Cloud Monitoring fast/slow burn, threshold, forecast and metric-absence policies.","PAYG",U["slo"],None),
      ("O2","Telemetry","Logging, Trace, Error Reporting and Managed Service for Prometheus.","PAYG",U["prometheus"],None),
      ("O3","Security","SCC continuous Pub/Sub export: ACTIVE, not muted, High/Critical findings.","PAYG",U["scc"],None),
      ("O4","Delivery","Cloud Build/Deploy verification failure, GKE unschedulable, Cloud Run 429 and quota events.","PAYG",U["deploy"],None),
      ("O5","Cost","Billing budget/anomaly Pub/Sub signals are delayed advisory evidence, not an immediate spend gate.","PAYG",U["budgets"],None),
      ("O6","Provider health","Personalized Service Health enrichment prevents mutating during a Google product/region incident.","PAYG","https://docs.cloud.google.com/service-health/docs/overview",None),
    ]
    row_cards(c,H-288,signals,112,7)
    section(c,H-320,"B / INCIDENT STATE MACHINE","Eventarc acknowledges Workflows start, not eventual workflow success; a stranded-execution reconciler is mandatory")
    states=[
      ("RECEIVED","validate schema/source/time; derive incident_key = hash(source,event,resource,condition)","duplicate returns existing incident"),
      ("CORRELATED","join service, release, image, trace, graph, SLO, SCC and provider-health windows","unresolved service/release -> evidence collection only"),
      ("COLLECTING","snapshot exact metric queries, logs, traces, findings, quotas and topology","manifest stores URI/time/hash; zero sources fails"),
      ("DIAGNOSING","ADK/Agent Runtime ranks hypotheses with cited evidence; optional Cloud Assist is read-only","variable/uncited result cannot authorize"),
      ("PLAN_PROPOSED","match hypothesis to versioned repair catalog; no free-form production shell/API","no catalog action -> ticket/change path"),
      ("POLICY_CHECKED","validate environment, blast radius, dependency capacity, freeze, rollback and identity","agent never owns approver/executor role"),
      ("EXECUTING","separate Cloud Run/GKE executor consumes single-use signed action receipt","idempotency key + before state + API response"),
      ("VERIFYING","synthetic, canary-vs-baseline, burn, security and headroom checks","checked=0 or missing telemetry -> rollback/pause"),
      ("STABILIZING","observe service-specific window; compare error budget, saturation and business KPI","new fast burn -> compensate immediately"),
      ("RESOLVED","close only after source condition clears and action/evidence reconcile","source health is never manually falsified"),
      ("LEARNING","persist timeline, root cause, action, override and outcome; create frozen regression case","promotion only through eval/CI pipeline"),
    ]
    y=table(c,M,H-345,[210,1260,830],["State","Atomic work","Failure / authority"],states,36,24,5.55,{(0,1):U["eventarc_retry"],(3,1):U["agent_runtime"],(7,1):U["monitor"]})
    section(c,y-17,"C / CAPACITY ENVELOPE","scale the dependency chain, not just the frontend")
    cap=[
      ("INGRESS","RPS, payload mix, LB/quota, burst and retry amplification","load test normal/spike/soak/dependency slowdown/quota exhaustion"),
      ("CLOUD RUN","concurrency, CPU, cold start, max instances, 429; memory does not drive autoscaling","downstream connection admission; max instances is not an instantaneous hard ceiling"),
      ("GKE","pod requests, HPA metrics, node quota, PDB/affinity, unschedulable pods","HPA chooses largest metric replica count; unavailable metric prevents scale-down"),
      ("DATA","AlloyDB sessions/CPU/storage/replication; Spanner CPU/latency; Pub/Sub oldest age","scale action must pass database and downstream capacity policy"),
      ("AGENTS/CI","queue age, task concurrency, Agent Runtime quota, build-minutes, reviewer WIP","reservation ledger controls dispatch; budget alert is not real-time admission"),
    ]
    y2=table(c,M,y-42,[200,1000,1100],["Layer","Measure","Gate / failure"],cap,40,24,5.55,{(1,1):U["run_scale"],(2,1):U["gke_hpa"],(4,1):U["billing_export"]})
    section(c,y2-17,"D / DR ADAPTERS + PROOF","RPO/RTO are service-owner contracts measured by restore drills")
    dr=[
      ("CONTROL","AlloyDB cross-region replication + backup/PITR; authority epoch fences old writer","restore new endpoint; replay outbox; reconcile before commands"),
      ("GRAPH","Spanner backup + canonical fact replay into new graph generation","count/hash/query suite before read-alias switch"),
      ("EVIDENCE","dual/multi-region Cloud Storage, locked manifests and KMS key availability","sample restore + signature/hash validation"),
      ("RUNTIME","pre-provisioned GKE/Cloud Run target, Backup for GKE/Backup and DR where applicable","management-plane creation is not a low-RTO strategy"),
      ("DELIVERY","Cloud Deploy re-promotes known digest; saga reconciles unknown/partial effects","database compatibility manifest chooses rollback vs forward-fix"),
    ]
    table(c,M,y2-42,[230,1130,940],["Plane","Recovery mechanism","Acceptance proof"],dr,38,24,5.5,{(0,1):U["alloydb_dr"],(1,1):U["spanner_backup"],(2,1):U["storage_dr"],(3,1):U["backup_dr"]})
    rect(c,M,48,W-2*M,40,RED2,RED,.8); txt(c,M+14,63,"NO DIRECT PATH: alert -> LLM -> production.  Required: signal -> evidence -> diagnosis -> catalog -> policy -> signed action -> separate executor -> verifier -> rollback/learning.","Mono-Bold",5.8,RED)
    footer(c,14)

def security(c):
    header(c,15,"G-LLD-14","Security and trust boundaries across every agent hop.","Standard includes enterprise app controls and Model Armor integration; workload security, data controls and irreversible authority remain separately engineered.","security")
    section(c,H-155,"CONTROL STACK","The narrowest credential wins; no agent receives standing production authority.")
    controls=[("C1","Identity","Cloud Identity / WIF for people; Workload Identity Federation and dedicated service accounts for workloads.","PAYG",U["wif"],None),("C2","Authorization","IAM custom roles + conditions + deny policies; connector scopes minimized; production mutation split.","BUILD",U["iam"],None),("C3","Perimeter","VPC Service Controls + Access Context Manager; app actions are blocked by default until allowed.","PAYG",U["vpc"],None),("C4","Secrets + keys","Secret Manager references, CMEK/Cloud KMS signing, rotation and revocation; no prompt-carried secret.","PAYG",U["secrets"],None),("C5","AI safety","Model Armor inspect/block; configure block-all on screening outage for high-risk app paths.","APP",U["model_armor"],None),("C6","Data protection","Sensitive Data Protection discovers/classifies/redacts; Model Armor itself does not remove PII.","PAYG",U["dlp"],None)]
    row_cards(c,H-288,controls,112,7)
    section(c,H-320,"TRUST-ZONE MATRIX","Every boundary has an ingress check, egress rule, log and owner.")
    rows=[("USER → APP","SSO / group / app IAM / device or network access","Gemini Enterprise app","user prompt + connector action audit"),("APP → CONNECTOR","OAuth/GitHub App scopes; document ACL; VPC-SC allowlist caveat","Gemini connector","source action + actor + object version"),("APP → AGENT","Agent identity/SPIFFE when published; Registry/Gateway policy","Agent Registry/Gateway","session + tool calls + policy verdict"),("AGENT → TOOL","service account; gateway allowlist; schema validation; purpose bound","Agent Gateway / MCP","request hash + response hash + cost"),("WORKER → REPO","short-lived credential; repo/branch scope; write-set; no merge credential","GKE/Cloud Run Job","commit + diff + receipt"),("CI → ARTIFACT","dedicated build SA; private pool; provenance; isolated secrets","Cloud Build/Artifact Registry","build + test + attestation"),("DEPLOY → PROD","Binary Authorization; deploy SA; approval; rollout budget","Cloud Deploy/GKE/Cloud Run","artifact digest + target + SLO decision")]
    links={(0,2):U["features"],(1,2):U["github"],(2,2):U["registry"],(3,2):U["gateway"],(4,2):U["gke"],(5,2):U["artifact"],(6,2):U["deploy"]}
    y=table(c,M,H-350,[250,900,550,600],["Boundary","Enforcement","Google surface","Evidence"],rows,42,24,5.8,links)
    section(c,y-18,"PRODUCT-SPECIFIC SECURITY BOUNDARIES","included support still requires configuration and does not propagate automatically")
    product=[
      ("MODEL ARMOR","no additional app cost; inspect/block and fail-closed option","Core Assistant, Google agents, Agent Designer","does not screen custom ADK/A2A/Dialogflow; blocks matching PII but does not de-identify"),
      ("VPC-SC","configure perimeter + access levels + egress/ingress","Gemini Enterprise app/data stores and selected Google services","assistant actions blocked until allowlisted; existing stores can require recreation"),
      ("CMEK","app/data-store/session/end-user data key configuration","Cloud KMS with separated key manager/user","key disable can stop serving; recovery latency means tested runbook required"),
      ("CUSTOM MCP","OAuth2/PKCE + up to documented action limit","Gemini app connector path","calls do not pass Agent Gateway; privileged use stays behind separate gateway/controller"),
      ("GITHUB APP","repository content/PR/issues/actions/project permissions","Gemini Enterprise connector","create separate read/search and bounded-write installations; no agent merge permission"),
      ("ANTIGRAVITY","AI developer credit/IDE tooling","human developer surface","documented compliance exclusions require workload-specific review"),
      ("AUDIT","Admin/System/Policy Denied + explicitly enabled Data Access","central Logging project + locked security evidence","Audit Logs corroborate actions; they do not authorize lifecycle state"),
    ]
    yp=table(c,M,y-43,[220,520,640,920],["Control","Configured capability","Applies to","Critical boundary"],product,42,24,5.35,{(0,1):U["model_armor"],(1,1):U["vpc"],(2,1):U["kms"],(3,1):U["custom_connector"],(6,1):U["audit"]})
    section(c,yp-18,"ABUSE + FAILURE TESTS","each attack produces a deterministic refusal and audit/evidence record")
    threats=[
      ("PROMPT INJECTION","untrusted ticket/code/doc asks for hidden secret/tool escalation","trust labels + Model Armor where applicable + tool policy + schema/resource validation"),
      ("DATA LEAK","cross-tenant chunk, stale ACL, deleted document, prompt/log contains PII","ACL-before-retrieval + SDP + prompt logging off/minimized + deletion reconciliation"),
      ("CREDENTIAL","bearer token in prompt/env/transcript; shared service account","WIF/short-lived scoped token; secret reference; dedicated identity; revoke on exit"),
      ("SUPPLY CHAIN","mutable builder/agent image, unsigned artifact, poisoned dependency","digest pinning + private pool + provenance + scans + Binary Authorization"),
      ("AUTHORITY","agent self-approves/merges/deploys or reuses stale approval","custom IAM role excludes irreversible permission; approval binds exact hashes/expiry"),
      ("NETWORK","worker reaches arbitrary internet/internal admin endpoint","deny-default VPC/firewall/network policy; gateway/egress allowlist; DNS/flow logs"),
      ("BREAKGLASS","emergency principal becomes standing bypass","time-bound human role, reason/ticket, dual notification, post-use review and rotation"),
    ]
    table(c,M,yp-43,[220,900,1180],["Threat","Exploit","Required control"],threats,38,24,5.45,{(0,2):U["model_armor"],(1,2):U["dlp"],(3,2):U["binary"]})
    rect(c,M,49,W-2*M,47,RED2,RED,.8); txt(c,M+14,77,"SECURITY BLOCKERS", "Mono-Bold",6,RED); para(c,M+145,79,"No shared agent identity • no broad GitHub App • no Data Access audit blind spot • no fail-open Model Armor for critical paths • no direct alert-to-production action • no human approval represented by model text.",W-2*M-165,"Display",6.5,8,MUTED,2)
    footer(c,15)

def contracts(c):
    header(c,16,"G-LLD-15","Executable lifecycle state machine and canonical evidence contracts.","Cards become implementation only when each transition has one command, one guard, one authority, one timeout and one receipt.","contracts")
    section(c,H-155,"AUTHORITY + PROJECTION MAP","AlloyDB commands state; GCS preserves evidence; Spanner serves graph; BigQuery analyzes history")
    rows=[("change_intent","source ID/version, accepted facts/assumptions, AC/NFR, risk, repos, allowed actions, state/version","AlloyDB authority"),("workflow_binding","change/stage, execution ID, callback, retries, deadline and last reconciled status","AlloyDB; Workflows reference"),("agent_invocation","attempt, agent/model/tool versions, context hash, scope, budget, input/output hash, cost","AlloyDB receipt + BigQuery"),("evidence_manifest","actor/command, checked/total, exit, objects{URI,generation,hash}, chain hash, signature","Locked Cloud Storage + AlloyDB ref"),("graph_snapshot","repo SHAs, asset time, parser/projector versions, edge counts, unresolved/stale and watermarks","Spanner projection + BigQuery"),("artifact_release","source/artifact digest, SBOM, provenance, attestations, tests, target, rollout, SLO decision","Artifact Registry/Deploy + AlloyDB ref"),("approval","subject hashes, identity, scope, risk, expiry, decision, reason, authority epoch","AlloyDB authority + Audit Logs"),("learning","failure signature, root cause, accepted fix, rule/eval version, owner, effectiveness window","BigQuery/Knowledge Catalog projection")]
    links={(0,2):U["alloydb"],(1,2):U["workflows"],(3,2):U["storage"],(4,2):U["spanner"],(5,2):U["artifact"],(7,2):U["knowledge"]}
    y=table(c,M,H-180,[300,1500,500],["Record","Required fields","Authority"],rows,42,24,5.7,links)
    section(c,y-18,"LEGAL TRANSITIONS","agents submit StageReceipt; command controller checks guard and writes transition/outbox/receipt atomically")
    transitions=[
      ("RECEIVED -> NEEDS_CLARITY","missing typed requirement/owner/scope/risk","ClarificationRequested + ticket writeback receipt"),
      ("RECEIVED -> READY","source current; required fields/policy/budget complete","ChangeIntent hash + source revision + checked/total"),
      ("READY -> IMPACTED","graph/context fresh; candidate repos checked; no critical unknown","graph snapshot + impact paths + coverage receipt"),
      ("IMPACTED -> PLANNED","SCC/waves/todos/tests/rollback compiled","Plan digest + child manifests + assumptions"),
      ("PLANNED -> ORACLE_LOCKED","independent reviewer identity and acceptance oracle stored","oracle digest + expiry; invalidated by plan/context/policy change"),
      ("ORACLE_LOCKED -> EXECUTING","lease/fence, base SHAs, budget and write sets current","ChildDispatched command + reservation"),
      ("EXECUTING -> PR_OPEN","nonempty bounded diff; local checks; all child receipts verified","branch/head SHA + plan/context/todo receipt set"),
      ("PR_OPEN -> VERIFIED","independent review and every required CI gate checked=total","gate-set digest + artifact/provenance refs"),
      ("VERIFIED -> APPROVED","human/policy authority binds current head/artifact/evidence/policy","approval digest + actor + expiry"),
      ("APPROVED -> DEPLOYING","protected merge + immutable release artifact + rollout created","merge/release/rollout IDs"),
      ("DEPLOYING -> OBSERVING","staging/canary/production verification passes","phase receipts + SLO/business/security denominator"),
      ("OBSERVING -> DONE","outcome window accepted and learning proposal stored","OutcomeAccepted + learning record"),
      ("ANY -> BLOCKED","ambiguity, stale input, invariant, budget, authority or unknown side effect","refusal receipt with owner/expiry/resume command"),
    ]
    yt=table(c,M,y-43,[320,1100,880],["Transition","Guard","Evidence"],transitions,34,24,5.35,{(0,2):U["github"],(9,2):U["deploy"]})
    section(c,yt-18,"STATE MACHINE","only deterministic evaluators emit transitions")
    states=[("DISCOVERED","source event stored"),("NEEDS_CLARITY","question posted"),("READY","policy complete"),("IMPACTED","graph coverage accepted"),("PLANNED","reviewer oracle signed"),("EXECUTING","repo leases active"),("PR_OPEN","receipts attached"),("VERIFIED","all gates checked"),("APPROVED","content-bound authority"),("DEPLOYING","artifact promoted"),("OBSERVING","SLO window active"),("DONE","learning promoted")]
    bw=(W-2*M-5*8)/6
    for i,(a,b) in enumerate(states):
        row=i//6; col=i%6; x=M+col*(bw+8); sy=max(118,yt-160)-row*70; rect(c,x,sy,bw,56,PAPER2,BLUE,.7); txt(c,x+9,sy+36,a,"Mono-Bold",5.5,BLUE); para(c,x+9,sy+18,b,bw-18,"Display",5.8,7,MUTED,2)
        if col<5: arrow(c,[(x+bw,sy+28),(x+bw+8,sy+28)])
    footer(c,16)

def learning_eval(c):
    header(c,17,"G-LLD-16","Company-wide learning: failures become governed rules, tests and agent evaluations.","Conversation memory never changes production behavior. Only accepted evidence moves through a versioned standards compiler and promotion pipeline.","learning")
    section(c,H-155,"A / LEARNING LOOP","one failure becomes a reproducible organizational capability")
    stages=[
      ("L1","Capture","Incident/change receipts, diff, logs, traces, graph snapshot, model/tools, cost and human override.","BUILD",U["bq"],None),
      ("L2","Classify","Deterministic signature by failure type, service, dependency, environment and violated invariant.","BUILD",U["spanner"],None),
      ("L3","Curate","Human/independent reviewer accepts root cause, correct fix and negative examples; PII is removed.","HUMAN",U["dlp"],None),
      ("L4","Compile","Generate proposed policy, parser rule, CI fixture, agent eval, runbook or graph resolution change.","BUILD",U["knowledge"],None),
      ("L5","Replay","Frozen BigQuery/GCS corpus replays baseline and candidate with exact tool/model/prompt versions.","PAYG",U["evals"],None),
      ("L6","Promote","Independent gates and owners sign version; staged rollout by repo/risk tier; rollback retained.","HUMAN",U["deploy"],None),
      ("L7","Observe","Measure false positives, escaped defects, repair rate, cost and override after effectiveness window.","BUILD",U["monitor"],None),
    ]
    row_cards(c,H-288,stages,112,7)
    section(c,H-320,"B / STANDARDS COMPILER","source document becomes executable controls; prose alone has no authority")
    rows=[
      ("ENGINEERING STANDARD","Markdown/Docs + owner + semantic version + effective date","lint/compiler config, allowed dependency set, code-review checklist","CI fixture proves compliant/noncompliant examples"),
      ("ARCHITECTURE RULE","ADR + graph predicate + risk class + exception process","impact guard, repo ownership, forbidden edge query","seeded graph cases + expiry-bound waiver"),
      ("SECURITY POLICY","Org policy/IAM/VPC-SC/Model Armor/SDP intent","Organization Policy, Policy Controller, Binary Authorization, custom policy API","negative access, prompt injection, exfiltration and unsigned-image tests"),
      ("RELIABILITY POLICY","SLI/SLO, capacity envelope, rollout and rollback contract","Monitoring alert, Cloud Deploy verify/repair, autoscaling bounds","burn replay, missing telemetry, 429, dependency saturation, rollback drill"),
      ("AGENT CONTRACT","role, context schema, tool allowlist, budget, refusal and receipt schema","ADK agent version + Agent Registry/Gateway + controller validation","response/tool-use/hallucination/safety + deterministic tool-call evals"),
      ("DATA/GOVERNANCE","classification, retention, lineage, ACL and deletion requirements","Knowledge Catalog aspects, SDP templates, storage lifecycle/lock, context authorization","cross-tenant leakage=0; tombstone and erasure denominators"),
    ]
    y=table(c,M,H-345,[300,650,730,620],["Input","Versioned source","Compiled enforcement","Proof"],rows,49,24,5.55,{(2,2):U["policy_controller"],(4,2):U["registry"],(5,2):U["knowledge"]})
    section(c,y-18,"C / EVALUATION MATRIX","compare candidate against pinned production baseline; publish confidence and denominator")
    evalrows=[
      ("INTAKE","ready classification, clarification precision, duplicate/refusal correctness","ticket/alert corpus by source, risk and language"),
      ("IMPACT","repo/service recall, critical-edge miss rate, false blast radius, stale-context refusal","200+ human-labeled multi-repo changes + graph mutations"),
      ("PLAN","acceptance completeness, todo atomicity, rollback/contract/data coverage","independent reviewer rubric + deterministic required-field checks"),
      ("IMPLEMENT","task success, unauthorized writes, test delta, repair attempts, cost/time","isolated replay repositories; no production credential"),
      ("REVIEW","defect/security/performance finding recall and false-positive rate","mutant PRs + known incidents + clean controls"),
      ("OPS","diagnosis rank, evidence citation, catalog selection, unsafe-action proposal rate","frozen incident packages + simulation only"),
      ("SYSTEM","escaped defects, rollback rate, P95 cycle time/cost, ACL leaks, human override","by repo, risk tier, agent/model/policy version"),
    ]
    y2=table(c,M,y-43,[220,1000,1080],["Lane","Metrics","Corpus / denominator"],evalrows,39,24,5.6,{(0,1):U["evals"],(5,1):U["evals"]})
    section(c,y2-18,"D / PROMOTION AUTHORITY","no auto-learning directly from production output")
    promos=[
      ("P1","Proposed","Learning record links source evidence and owner.","BUILD",U["bq"],None),
      ("P2","Validated","Deterministic checks + independent rubric + safety eval pass.","BUILD",U["evals"],None),
      ("P3","Approved","Policy/standard owner signs exact artifact digest.","HUMAN",U["kms"],None),
      ("P4","Canary","Selected repos/risk tier; old version remains available.","PAYG",U["deploy_canary"],None),
      ("P5","Effective","Outcome window proves improvement without safety regression.","BUILD",U["monitor"],None),
    ]
    row_cards(c,max(100,y2-200),promos,150,8)
    footer(c,17)

def pricing(c):
    header(c,18,"G-LLD-17","Commercial boundary: included, overage, PAYG and build cost.","A Standard seat is the employee front door. The autonomous SDLC is a portfolio of separately metered Google services and customer engineering.","pricing")
    section(c,H-155,"PUBLIC USD SNAPSHOT / 26 AUG 2026","Use billing SKUs and negotiated contract for procurement; list prices can change.")
    rows=[
      ("Gemini Enterprise Standard","INCLUDED","Starts $30/seat/month for Standard/Plus; exact Standard quote sales-led","30 GiB index; 160 assistant/day; 3 Deep Research/day; $10 dev credit"),
      ("Gemini Enterprise overage","OVERAGE","$5/GiB-month index; other overages at Agent Platform prices","Invoiced billing + enabled overages required"),
      ("Code Assist Standard","INCLUDED","Bundled Standard license; standalone $0.031232877/h monthly or $0.026027397/h annual","~$22.80 / ~$19 monthly at 730h"),
      ("Code Assist Enterprise","SEPARATE","$0.073972603/h monthly or $0.061643836/h annual","~$54 / ~$45 monthly; private-code customization"),
      ("Agent Runtime compute","PAYG","50 vCPU-h/month free, then $0.085/vCPU-h","Model tokens billed separately"),
      ("Agent Runtime memory","PAYG","100 GiB-h/month free, then $0.009/GiB-h","Runtime/Sandbox allocation"),
      ("Agent storage","PAYG","1 GiB-month free, then $0.30/GiB-month equivalent","Sessions/Memory/Registry schedules vary"),
      ("Workflows","PAYG","5k internal + 2k external steps free; then $0.01/$0.025 per 1k","Retries are additional steps"),
      ("Pub/Sub","PAYG","First 10 GiB/month free; then $40/TiB basic delivery","Network/storage extras apply"),
      ("Eventarc Standard","PAYG","First 50k events/month free; source-dependent event rate","Pub/Sub transport may bill separately"),
      ("Cloud Tasks","PAYG","First 1M operations/month free; then $0.40/M","Attempts charged in 32 KiB chunks; rate limits are architecture inputs"),
      ("Cloud Run Jobs","PAYG","us-central1 example $0.000011244/vCPU-s + $0.000001235/GiB-s","One-minute minimum; network/storage/logging/model tokens extra"),
      ("GKE Autopilot","PAYG","$0.10/cluster-hour + requested resources; eligible monthly cluster credit","General-purpose resource and region prices vary"),
      ("AlloyDB HA","PAYG","Example starts ~$0.06608/vCPU-h + $0.0112/GiB-h + storage","HA primary bills active+standby nodes; backup/network extra"),
      ("Cloud Build","PAYG","2,500 eligible free build-min/month; e2-medium starts $0.003/min","Private pools, logs, storage and artifacts extra"),
      ("Secure Source Manager","PAYG","$1,000/instance/month public product price","Repositories/egress/CI and support boundaries require quote validation"),
      ("Artifact Registry","PAYG","First 0.5 GiB-month free; then ~$0.000136986/GiB-hour","Transfer/requests and retention add cost"),
      ("Artifact Analysis","PAYG","$0.26 automatic/on-demand scan public rate","Repeated automatic scan of same digest documented free; verify SKU/region"),
      ("SCC Artifact Guard","SEPARATE","Requires SCC Premium/Enterprise; PAYG registry scan $0.20; subscription floor applies","Preview/container vulnerability policy - not broad language SAST"),
      ("Binary Authorization","PAYG","Cloud Run no charge; GKE ~$0.01613/cluster-hour with billing credit","Attestor/KMS/Logging/GKE costs separate"),
      ("Cloud Deploy","PAYG","First active multi-target pipeline free; each additional $5/month","Cloud Build, Storage, Logging bill separately"),
      ("Spanner Graph/vector","PAYG","Enterprise/Enterprise Plus compute, storage, backup and network","No single atlas rate; size by processing units, graph rows and QPS"),
      ("BigQuery/Knowledge Catalog","PAYG","storage/compute/scanning/metadata operations by edition/region","History, evals, lineage and catalog are not seat entitlement"),
      ("Cloud Storage + KMS","PAYG","us-central1 storage ~$0.022/GiB-month; KMS software key/version+ops usage","Locked versions, replication, signatures and retention stay billable"),
      ("Cloud Logging/Monitoring","PAYG","first allowances then telemetry volume/retention/metrics pricing","Prompt/tool/audit volume and central retention drive TCO"),
      ("Fleet + parsers + policy","BUILD","Engineering payroll + operation + evaluation","No Google seat entitlement covers this control plane"),
    ]
    link_list=[U["ge"],U["quotas"],U["code_price"],U["code_price"],U["agent_price"],U["agent_price"],U["agent_price"],U["workflows_price"],U["pubsub_price"],U["eventarc_price"],U["tasks_price"],U["run_price"],U["gke_price"],U["alloydb_price"],U["build_price"],U["ssm_price"],U["artifact_price"],U["analysis_price"],U["scc_price"],U["binary_price"],U["deploy_price"],U["spanner_price"],U["knowledge_price"],U["storage_price"],U["obs_price"],U["workflows"]]
    y=table(c,M,H-180,[420,180,900,800],["Product / layer","Class","Public price","Boundary"],rows,35,26,5.55,{(i,0):link_list[i] for i in range(len(rows))})
    section(c,y-18,"CTO COST MODEL","monthly = seats + overage + model tokens + agent runtime + orchestration + CI + compute + data + security + observability + people")
    drivers=[
      ("DEMAND","tasks/day by risk and repo count; autonomous findings/day; peak fan-out; clarification/retry/repair rate"),
      ("AI","input/output/cache/embedding tokens, tool calls, vCPU/GiB-hours, sessions/memory and developer credits"),
      ("DELIVERY","build minutes, private-pool idle/active capacity, artifact/scans, canary/test load and evidence bytes"),
      ("KNOWLEDGE","source snapshots, facts/edges/chunks, embedding dimensions, index QPS, rebuild frequency and retention"),
      ("OPERATIONS","telemetry GiB/cardinality/retention, backups/replication, support, egress and on-call engineering"),
    ]
    table(c,M,y-43,[250,2050],["Driver","Measure during pilot"],drivers,34,24,5.55)
    rect(c,M,48,W-2*M,32,AMBER2,AMBER,.8); txt(c,M+14,60,"NO ALL-IN NUMBER IS HONEST UNTIL PILOT TELEMETRY MEASURES: tasks/day, tokens/task, vCPU-h/task, build-min/task, graph size, vector QPS, retention and failure/retry rate.","Mono-Bold",5.9,AMBER)
    footer(c,18)

def sources(c):
    header(c,19,"G-LLD-18","Official Google source catalog.","Every product behavior and commercial claim in this atlas resolves to a primary Google documentation or pricing page.","sources")
    section(c,H-155,"CLICK A ROW","Launch stage and pricing remain time-sensitive; re-verify before procurement or production rollout.")
    src=[
      ("Gemini Enterprise editions",U["editions"],"Standard feature matrix"),("Gemini Enterprise product/pricing",U["ge"],"$30 starting price"),("Licenses/subscriptions",U["licenses"],"seat assignment/location"),("Quotas and overages",U["quotas"],"pooled quotas + overage"),("Apps and data stores",U["apps"],"search/data-store model"),("Connector catalog",U["connectors"],"source launch stages"),("GitHub connector",U["github"],"read/write actions"),("Jira Cloud connector",U["jira"],"ticket actions/sync"),("Custom connector",U["custom_connector"],"ACL/sync responsibilities"),("Agent overview",U["agents"],"app agent governance"),("Agent Designer",U["designer"],"Preview no-code agents"),("Model Armor in app",U["model_armor"],"included screening boundary"),("App VPC-SC",U["vpc"],"perimeter/action caveat"),("App metrics",U["metrics"],"usage visibility"),
      ("Code Assist overview",U["code"],"Standard vs Enterprise"),("Code Assist customization",U["code_customize"],"private-repo index limits"),("Code Assist pricing",U["code_price"],"license rates"),("Code Assist quotas","https://docs.cloud.google.com/gemini/docs/quotas","per-user limits"),("Agent Platform overview",U["agent_platform"],"build/scale/govern"),("Agent Platform pricing",U["agent_price"],"runtime/memory/storage"),("Model pricing",U["model_price"],"tokens/embeddings"),("Agent Runtime",U["agent_runtime"],"host ADK agents"),("Agent Registry",U["registry"],"agent catalog"),("Agent Gateway",U["gateway"],"agent/tool policy"),("Google ADK",U["adk"],"agent framework"),("A2A registration",U["a2a"],"external agent exposure"),
      ("Workflows",U["workflows"],"bounded saga/retry/callback"),("Pub/Sub delivery",U["pubsub_delivery"],"at-least-once subscriber"),("Pub/Sub exactly-once",U["pubsub_exact"],"regional pull boundary"),("Eventarc retries",U["eventarc_retry"],"duplicate/start semantics"),("Cloud Tasks",U["tasks"],"rate-limited commands"),("Cloud Scheduler",U["scheduler"],"reconciliation trigger"),("Cloud Run Jobs",U["run_jobs"],"agent/parser workers"),("GKE",U["gke"],"coordinated sandboxes"),
      ("AlloyDB",U["alloydb"],"PostgreSQL authority"),("AlloyDB vector",U["alloydb_vector"],"ScaNN/pgvector"),("AlloyDB hybrid",U["alloydb_hybrid"],"RRF hybrid search"),("AlloyDB CDC",U["alloydb_cdc"],"logical decoding/Datastream"),("AlloyDB DR",U["alloydb_dr"],"cross-region replication"),("Spanner Graph",U["spanner"],"typed graph serving"),("Spanner graph schema",U["spanner_schema"],"node/edge mapping"),("Spanner vector",U["spanner_vector"],"ANN search"),("BigQuery",U["bq"],"history/evals"),("BigQuery vector",U["bq_vector"],"offline search/evaluation"),("Vertex embeddings",U["vertex_embed"],"semantic vectors"),("Vertex Vector Search",U["vector"],"scale-out retrieval"),("Knowledge Catalog",U["knowledge"],"metadata/governance"),("Cloud Asset feeds",U["cai_feed"],"resource change stream"),
      ("Secure Source Manager",U["ssm"],"Google-native SCM"),("SSM pull request",U["ssm_pr"],"PR creation"),("Branch protection",U["ssm_branch"],"review/status authority"),("Cloud Build triggers",U["build_trigger"],"PR/push CI"),("Build private pools",U["build_private"],"isolated CI"),("Build provenance",U["build_provenance"],"supply-chain proof"),("Artifact Registry",U["artifact"],"immutable artifacts"),("Artifact Analysis",U["analysis"],"vulnerability metadata"),("Binary Authorization",U["binary"],"admission policy"),("Cloud Deploy",U["deploy"],"promotion"),("Deploy canary",U["deploy_canary"],"progressive delivery"),("Deploy approval",U["deploy_approval"],"human production gate"),("Lighthouse CI",U["lighthouse"],"Google OSS web gate"),
      ("Cloud Monitoring SLO",U["slo"],"burn evidence"),("Monitoring Pub/Sub",U["monitor_pubsub"],"alert automation"),("Managed Prometheus",U["prometheus"],"metrics collection"),("Cloud Run autoscaling",U["run_scale"],"runtime capacity"),("GKE HPA",U["gke_hpa"],"pod capacity"),("Security Command Center",U["scc"],"security findings"),("Policy Controller",U["policy_controller"],"GKE policy"),("IAM/WIF",U["wif"],"workload identity"),("Cloud KMS",U["kms"],"keys/signatures"),("Secret Manager",U["secrets"],"secret references"),("Sensitive Data Protection",U["dlp"],"classification/redaction"),("Audit Logs",U["audit"],"corroborating evidence"),("Organization Policy",U["org_policy"],"resource guardrails"),
      ("Billing export",U["billing_export"],"cost evidence"),("Budgets",U["budgets"],"advisory cost signals"),("Recommender",U["recommender"],"optimization findings"),("Backup and DR",U["backup_dr"],"recovery services"),("Spanner backup",U["spanner_backup"],"graph recovery"),("Storage durability",U["storage_dr"],"evidence recovery"),("Vertex evaluations",U["evals"],"agent regression gates"),("Cloud Storage lock",U["storage"],"immutable evidence")]
    half=(len(src)+1)//2
    for col,items in enumerate([src[:half],src[half:]]):
        x=M+col*(W/2); widths=[260,650,240]
        rows=[(name,url.replace("https://","")[:72],why) for name,url,why in items]
        links={(i,0):items[i][1] for i in range(len(items))}; links.update({(i,1):items[i][1] for i in range(len(items))})
        table(c,x,H-180,widths,["Source","URL","Used for"],rows,31,24,5.2,links)
    footer(c,19,"Official sources only. Links are live annotations inside the PDF.")

def acceptance(c):
    header(c,20,"G-LLD-19","CTO acceptance gates before autonomous production use.","The target is bounded autonomy with measurable proof — not a vendor-shaped promise that Standard alone provides the SDLC.","acceptance")
    section(c,H-155,"PHASE-EXIT CHECKLIST","A phase is unearned until the exit evidence exists.")
    rows=[
      ("ENTITLEMENT","signed Google quote + edition/SKU/region/allowlist sheet; license assignment tested","CFO + platform owner"),
      ("CONNECTOR","GitHub/Jira ACL and action scopes validated; sync freshness and deletion propagation measured","security + SCM owner"),
      ("GRAPH","30/30 repos indexed; candidate service coverage published; stale/unresolved edges below agreed limit","architecture owner"),
      ("RETRIEVAL","ACL leakage = 0 in adversarial suite; symbol recall@10 and graph path accuracy hit target","knowledge owner"),
      ("ORCHESTRATION","duplicate, out-of-order, timeout, callback, compensation and replay tests pass","platform owner"),
      ("FLEET","sandbox escape, secret exposure, overlapping write-set and stale SHA tests fail closed","security + Fleet owner"),
      ("IMPLEMENTATION","agent completes bounded low-risk tasks with typed receipts and no unauthorized writes","delivery owner"),
      ("CI","all gates publish checked/total; zero-input gate fails; provenance and artifact identity verified","quality owner"),
      ("AUTHORITY","protected merge and production approval cannot be bypassed by connector or agent identity","CTO / change authority"),
      ("RELEASE","canary, SLO hold, rollback and database expand/contract rehearsed in production-like environment","SRE owner"),
      ("REPAIR","alert → intent → patch → PR path demonstrated; no direct alert-to-production route","incident owner"),
      ("COST","P50/P95 cost per accepted change, retry rate and quota exhaustion behavior measured","FinOps owner"),
      ("DR","graph/evidence restore, workflow replay, registry dependency and region failure exercises complete","SRE + data owner"),
      ("LEARNING","accepted failures create versioned rules/evals; false-positive and regression trend reviewed monthly","AI governance board"),
    ]
    y=table(c,M,H-180,[300,1600,400],["Gate","Required evidence","Accountable"],rows,47,26,6.1)
    section(c,y-18,"INDEPENDENT VERIFIERS","run after implementation and before any autonomy expansion")
    ver=[
      ("ARCH","Architecture verifier","Every arrow names owner, trigger, schema, delivery, idempotency, retry/timeout, authority and evidence. Every projection names authority/rebuild source. Every multi-repo wave exposes partial success and compensation.","BUILD",U["workflows"],None),
      ("SEC","Security/autonomy verifier","Reject ACL-after-retrieval, bearer passthrough, shared identity, broad GitHub/MCP, mutable image, self-approval, unrestricted shell/network, stale approval and destructive free-form action.","BUILD",U["iam"],None),
      ("COMM","Entitlement/pricing verifier","Reject app-visible capability shown as included runtime, Standard/Enterprise conflation, stale/unitless price, Preview as GA, list price as TCO or any named product without primary source.","HUMAN",U["editions"],None),
    ]
    row_cards(c,y-210,ver,165,14)
    section(c,y-250,"IMPLEMENTATION ORDER","build proof boundaries before autonomy")
    phase=[
      ("01","License + app","quote/SKU/region/overage/spend cap; identities; connector read-only; Model Armor/VPC-SC/CMEK"),
      ("02","Authority","AlloyDB command/receipt/outbox; locked GCS evidence; Pub/Sub/Tasks/Workflows; reconciler"),
      ("03","Knowledge","30 repos snapshot/parsers; canonical identities; Spanner Graph; hybrid retrieval; signed ContextPack"),
      ("04","Fleet","GKE/Cloud Run isolation; leases/fences; worktrees; typed receipts; budgets; no merge credential"),
      ("05","Proof","SSM/GitHub PR; Cloud Build private pools; gate receipt; provenance/scans/attestation; human merge"),
      ("06","Release","Cloud Deploy staging/canary/approval/rollback; SLO/capacity/DR drills; runtime-to-repair loop"),
      ("07","Learning","frozen eval corpus; standards compiler; canary agent/policy rollout; metric-driven autonomy expansion"),
    ]
    table(c,M,y-275,[100,280,1920],["Wave","Name","Exit"],phase,39,24,5.55)
    section(c,113,"RECOMMENDED FIRST PRODUCTION BOUNDARY","Autonomous discovery/planning/implementation; independent deterministic verification; human merge and production promotion.")
    rect(c,M,53,W-2*M,43,BLUE2,BLUE,.8); txt(c,M+14,79,"GO / EXPAND", "Mono-Bold",6,BLUE); para(c,M+118,80,"Expand only when accepted-change rate, escaped-defect rate, P95 cycle time, cost/change, rollback rate, ACL leakage, stale-context refusals and human override rate are all visible by repo and risk tier.",W-2*M-138,"Display",6.5,8,MUTED,2)
    footer(c,20,"This is an implementation reference, not a claim that Gemini Enterprise Standard alone supplies the complete SDLC.")

def build():
    os.makedirs(os.path.dirname(OUT),exist_ok=True)
    c=canvas.Canvas(OUT,pagesize=landscape(A1),pageCompression=1)
    c.setTitle("DevX Google-Native AI SDLC - Deep Implementation Atlas")
    c.setAuthor("DevX Labs / CTO Office")
    c.setSubject("Google-only evidence-bound autonomous SDLC for 30 repositories and 100+ interdependent services, with Gemini Enterprise Standard and app boundaries")
    for fn in [master,entitlement,app_boundary,signals,authority_plane,knowledge,graph_projection,retrieval_context,multirepo,agents,fleet,prove,release,operations_dr,security,contracts,learning_eval,pricing,sources,acceptance]: fn(c)
    c.save(); print(OUT)

if __name__=="__main__": build()
