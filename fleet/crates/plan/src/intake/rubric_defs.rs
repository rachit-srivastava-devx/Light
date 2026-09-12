//! The fixed rubric vocabulary: the 2 core questions (`intake.sh:105-106`) and the 13 derived
//! rows (`intake.sh:111-123`), each paired with its trigger `Matcher`s.

use super::pattern::Matcher;
use super::pattern::Matcher::{CleanUp, Literal, Stem, Word, WordOptS};
use super::question_id::QuestionId;
use super::question_id::QuestionId::*;
use super::pattern::Matcher::CliFlag as MatchCliFlag;

pub(crate) struct Derived {
    pub id: QuestionId,
    pub dimension: &'static str,
    pub question: &'static str,
    pub matchers: &'static [Matcher],
}

pub(crate) const CORE: [(QuestionId, &str, &str); 2] = [
    (Scope, "scope boundary", "What is explicitly in scope, and what boundary must not be crossed?"),
    (Success, "success criteria", "What observable result proves this is done, including the acceptance threshold?"),
];

pub(crate) static DERIVED: &[Derived] = &[
    Derived { id: CliFlag, dimension: "cli flag/option", question: "Which exact flag/option name and default value, and does behaviour change for callers who omit it?", matchers: &[MatchCliFlag, WordOptS("flag"), WordOptS("option"), Word("switch")] },
    Derived { id: SchemaMigration, dimension: "schema/migration", question: "Is the schema/data migration reversible, what happens to in-flight rows during cutover, and is any downtime acceptable?", matchers: &[Stem("migrat"), Word("schema"), Word("backfill"), Word("ddl"), WordOptS("column")] },
    Derived { id: UiView, dimension: "UI surface", question: "Which breakpoints/themes (light/dark, mobile/desktop) and accessibility bar must this view meet?", matchers: &[Word("ui"), WordOptS("view"), Word("screen"), Word("console"), Word("dashboard"), WordOptS("component"), Word("layout"), Word("map"), Stem("render")] },
    Derived { id: ApiEndpoint, dimension: "API contract", question: "What is the exact request/response shape, and can this break an existing caller (is it versioned)?", matchers: &[WordOptS("endpoint"), WordOptS("route"), Word("api"), Word("request"), Word("response"), Word("http")] },
    Derived { id: Deletion, dimension: "deletion/cleanup", question: "Deletion/cleanup of what exactly - is it recoverable (soft-delete, backup) or permanent, and what must survive?", matchers: &[Stem("delete"), Stem("remove"), Word("drop"), Stem("purge"), CleanUp] },
    Derived { id: Scale, dimension: "scale", question: "What peak users, throughput, latency, and growth must the design support?", matchers: &[Word("scale"), Word("throughput"), Word("rps"), Literal("requests per"), Word("users"), Word("latency"), Word("p95"), Word("load"), Stem("concurren")] },
    Derived { id: Tenancy, dimension: "tenancy", question: "Is this single-tenant or multi-tenant, and what isolation boundary is required?", matchers: &[Stem("tenant"), Literal("multi-tenant"), Literal("single-tenant"), Stem("isolat")] },
    Derived { id: Precision, dimension: "money and precision", question: "Which money, token, and count values require integer representation, and what rounding is allowed?", matchers: &[Word("money"), Word("currency"), Word("cost"), Word("price"), WordOptS("token"), Word("quota"), Word("cents"), Word("decimal"), Word("billing")] },
    Derived { id: Auth, dimension: "auth and access", question: "Who may perform each operation, and how are authentication and authorization enforced?", matchers: &[Stem("auth"), WordOptS("permission"), WordOptS("role"), Word("access"), Word("identity"), Word("credentials")] },
    Derived { id: Ownership, dimension: "data ownership", question: "Who owns each input and output, and which system is the source of truth?", matchers: &[Stem("persist"), WordOptS("store"), Word("stored"), Word("database"), WordOptS("record"), Word("retention"), WordOptS("owner"), Word("ownership")] },
    Derived { id: Failure, dimension: "failure behaviour", question: "What must happen on failure, timeout, malformed upstream output, and partial completion?", matchers: &[Word("retry"), WordOptS("timeout"), Word("unavailable"), Word("degraded"), Word("rollback"), WordOptS("error"), Stem("fail")] },
    Derived { id: Unhappy, dimension: "unhappy paths", question: "What should happen for empty, malformed, huge, duplicate, or concurrent input?", matchers: &[WordOptS("input"), Stem("parse"), WordOptS("upload"), WordOptS("import"), WordOptS("file"), Word("form"), Word("payload")] },
    Derived { id: RenameRefactor, dimension: "rename/refactor", question: "What call sites, imports, and references must be updated, and must external behaviour stay identical?", matchers: &[Stem("renam"), Stem("refactor"), Stem("restructur"), Stem("reorganis"), Stem("reorganiz")] },
];
