use rmcp::handler::server::router::Router;
use rmcp::handler::server::tool::{
    parse_json_object, schema_for_input, ToolCallContext, ToolRoute,
};
use rmcp::model::{CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Tool};
use rmcp::service::{MaybeSendFuture, RequestContext};
use rmcp::{transport::stdio, ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::future::Future;
use std::path::{Component, Path, PathBuf};

const EXIT_ENV: i32 = 3;
const EXIT_REFUSAL: i32 = 7;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
struct FileReadParams {
    path: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
struct FileWriteParams {
    path: String,
    content: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
struct FileListParams {
    path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
struct ImpactParams {
    symbol: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
struct LessonRecallParams {
    context: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
struct LedgerReadParams {
    limit: Option<u64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct ManifestTool {
    name: String,
    scope: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct ToolManifest {
    lease: String,
    tools: Vec<ManifestTool>,
}

#[derive(Clone, Debug)]
struct Lease {
    expression: String,
    prefix: PathBuf,
}

impl Lease {
    fn parse(expression: &str) -> Result<Self, i32> {
        let prefix_text = expression.strip_suffix("/**").ok_or(EXIT_REFUSAL)?;
        if prefix_text.is_empty() {
            return Err(EXIT_REFUSAL);
        }
        let prefix = normalize_relative(prefix_text).map_err(|_| EXIT_REFUSAL)?;
        if prefix.as_os_str().is_empty() {
            return Err(EXIT_REFUSAL);
        }
        Ok(Self {
            expression: expression.to_string(),
            prefix,
        })
    }

    fn manifest(&self) -> ToolManifest {
        let scope = self.expression.clone();
        ToolManifest {
            lease: self.expression.clone(),
            tools: [
                ("file_read", scope.clone()),
                ("file_write", scope.clone()),
                ("file_list", scope),
                ("impact", "fleet".to_string()),
                ("lesson_recall", "fleet".to_string()),
                ("ledger_read", "fleet".to_string()),
            ]
            .into_iter()
            .map(|(name, scope)| ManifestTool {
                name: name.to_string(),
                scope,
            })
            .collect(),
        }
    }
}

pub fn manifest_for_lease(lease: &str) -> Result<Value, i32> {
    if lease.trim().is_empty() {
        eprintln!("fleet mcp manifest <lease>: no tools for lease '' (empty lease grants nothing)");
        return Err(7);
    }
    let manifest = Lease::parse(lease)?.manifest();
    serde_json::to_value(manifest).map_err(|_| 6)
}

pub fn serve(repo: &str, lease: &str) -> Result<(), i32> {
    let server = LeaseServer::new(repo, lease)?;
    let routes = server.routes()?;
    let mut router = Router::new(server);
    router.tool_router.transparent_when_not_found = true;
    let router = router.with_tools(routes);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| EXIT_ENV)?;
    runtime.block_on(async move {
        let service = router.serve(stdio()).await.map_err(|_| EXIT_ENV)?;
        service.waiting().await.map(|_| ()).map_err(|_| EXIT_ENV)
    })
}

struct LeaseServer {
    repo_root: PathBuf,
    lease: Lease,
    manifest: ToolManifest,
}

impl LeaseServer {
    fn new(repo: &str, expression: &str) -> Result<Self, i32> {
        let lease = Lease::parse(expression)?;
        let repo_root = fs::canonicalize(repo).map_err(|_| EXIT_ENV)?;
        let manifest = lease.manifest();
        Ok(Self {
            repo_root,
            lease,
            manifest,
        })
    }

    fn routes(&self) -> Result<Vec<ToolRoute<Self>>, i32> {
        let mut routes = Vec::new();
        if self.allows("file_read") {
            routes.push(Self::route::<FileReadParams>(
                "file_read",
                "Read one file inside the leased prefix.",
                Self::file_read,
            )?);
        }
        if self.allows("file_write") {
            routes.push(Self::route::<FileWriteParams>(
                "file_write",
                "Write one file inside the leased prefix.",
                Self::file_write,
            )?);
        }
        if self.allows("file_list") {
            routes.push(Self::route::<FileListParams>(
                "file_list",
                "List one directory inside the leased prefix.",
                Self::file_list,
            )?);
        }
        routes.push(Self::route::<ImpactParams>(
            "impact",
            "Return local impact-analysis capability metadata for a symbol.",
            Self::impact,
        )?);
        routes.push(Self::route::<LessonRecallParams>(
            "lesson_recall",
            "Return local lesson-recall capability metadata for context.",
            Self::lesson_recall,
        )?);
        routes.push(Self::route::<LedgerReadParams>(
            "ledger_read",
            "Read the local fleet receipt ledger.",
            Self::ledger_read,
        )?);
        Ok(routes)
    }

    fn route<P>(
        name: &'static str,
        description: &'static str,
        handler: fn(&Self, P) -> Result<CallToolResult, McpError>,
    ) -> Result<ToolRoute<Self>, i32>
    where
        P: for<'de> Deserialize<'de> + JsonSchema + Send + 'static,
    {
        let schema = schema_for_input::<P>().map_err(|_| EXIT_ENV)?;
        let attr = Tool::new(name, description, schema);
        Ok(ToolRoute::new_dyn(
            attr,
            move |context: ToolCallContext<'_, Self>| {
                let service = context.service;
                let arguments = context.arguments.unwrap_or_default();
                let result = match parse_json_object::<P>(arguments) {
                    Ok(parameters) => handler(service, parameters),
                    Err(_) => service.outcome(
                        name,
                        "refused",
                        EXIT_REFUSAL,
                        json!({"reason":"INVALID_PARAMETERS"}),
                        true,
                    ),
                };
                Box::pin(std::future::ready(result.map(Into::into)))
            },
        ))
    }

    fn allows(&self, name: &str) -> bool {
        self.manifest.tools.iter().any(|tool| tool.name == name)
    }

    fn outcome(
        &self,
        tool: &str,
        status: &str,
        exit_code: i32,
        details: Value,
        is_error: bool,
    ) -> Result<CallToolResult, McpError> {
        let event = if status == "refused" {
            "refusal"
        } else {
            "gate_verdict"
        };
        let receipt_body = json!({
            "tool": tool,
            "status": status,
            "details": details.clone(),
            "lease": self.lease.expression,
        });
        let receipt = crate::append_receipt(event, receipt_body, "keel-mcp", None, Some(exit_code))
            .map_err(|code| {
                McpError::internal_error(format!("receipt write failed exit={code}"), None)
            })?;
        let response = json!({
            "tool": tool,
            "status": status,
            "details": details,
            "receipt": receipt,
        });
        let text = serde_json::to_string(&response)
            .map_err(|_| McpError::internal_error("tool response serialization failed", None))?;
        if is_error {
            Ok(CallToolResult::error(vec![ContentBlock::text(text)]))
        } else {
            Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
        }
    }

    fn authorized_path(&self, requested: &str, write: bool) -> Result<PathBuf, String> {
        let relative = normalize_relative(requested).map_err(|_| "INVALID_PATH".to_string())?;
        if !relative.starts_with(&self.lease.prefix) {
            return Err("OUT_OF_LEASE".to_string());
        }
        let candidate = self.repo_root.join(&relative);
        let lease_root = self.repo_root.join(&self.lease.prefix);
        let canonical_lease =
            fs::canonicalize(&lease_root).map_err(|_| "LEASE_ROOT_MISSING".to_string())?;
        if !canonical_lease.starts_with(&self.repo_root) {
            return Err("LEASE_ROOT_ESCAPES_REPO".to_string());
        }
        let canonical_target = match fs::symlink_metadata(&candidate) {
            Ok(_) => fs::canonicalize(&candidate).map_err(|_| "PATH_UNREADABLE".to_string())?,
            Err(error) if write && error.kind() == std::io::ErrorKind::NotFound => {
                let parent = candidate
                    .parent()
                    .ok_or_else(|| "INVALID_PATH".to_string())?;
                fs::canonicalize(parent).map_err(|_| "PARENT_UNREADABLE".to_string())?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err("PATH_MISSING".to_string());
            }
            Err(_) => return Err("PATH_UNREADABLE".to_string()),
        };
        if !canonical_target.starts_with(&canonical_lease) {
            return Err("OUT_OF_LEASE".to_string());
        }
        Ok(candidate)
    }

    fn file_read(&self, params: FileReadParams) -> Result<CallToolResult, McpError> {
        let path = match self.authorized_path(&params.path, false) {
            Ok(path) => path,
            Err(reason) => {
                return self.outcome(
                    "file_read",
                    "refused",
                    EXIT_REFUSAL,
                    json!({"reason":reason,"path":params.path}),
                    true,
                )
            }
        };
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => {
                return self.outcome(
                    "file_read",
                    "error",
                    EXIT_ENV,
                    json!({"reason":"READ_FAILED","path":params.path}),
                    true,
                )
            }
        };
        self.outcome(
            "file_read",
            "ok",
            0,
            json!({"path":params.path,"bytes":content.len(),"content":content}),
            false,
        )
    }

    fn file_write(&self, params: FileWriteParams) -> Result<CallToolResult, McpError> {
        let path = match self.authorized_path(&params.path, true) {
            Ok(path) => path,
            Err(reason) => {
                return self.outcome(
                    "file_write",
                    "refused",
                    EXIT_REFUSAL,
                    json!({"reason":reason,"path":params.path}),
                    true,
                )
            }
        };
        if fs::write(&path, params.content.as_bytes()).is_err() {
            return self.outcome(
                "file_write",
                "error",
                EXIT_ENV,
                json!({"reason":"WRITE_FAILED","path":params.path}),
                true,
            );
        }
        self.outcome(
            "file_write",
            "ok",
            0,
            json!({"path":params.path,"bytes":params.content.len()}),
            false,
        )
    }

    fn file_list(&self, params: FileListParams) -> Result<CallToolResult, McpError> {
        let default_path = self.lease.prefix.to_string_lossy().into_owned();
        let requested = params.path.as_deref().unwrap_or(&default_path);
        let path = match self.authorized_path(requested, false) {
            Ok(path) => path,
            Err(reason) => {
                return self.outcome(
                    "file_list",
                    "refused",
                    EXIT_REFUSAL,
                    json!({"reason":reason,"path":requested}),
                    true,
                )
            }
        };
        let mut entries = Vec::new();
        let iterator = match fs::read_dir(path) {
            Ok(iterator) => iterator,
            Err(_) => {
                return self.outcome(
                    "file_list",
                    "error",
                    EXIT_ENV,
                    json!({"reason":"LIST_FAILED","path":requested}),
                    true,
                )
            }
        };
        for entry in iterator {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    return self.outcome(
                        "file_list",
                        "error",
                        EXIT_ENV,
                        json!({"reason":"LIST_ENTRY_FAILED","path":requested}),
                        true,
                    )
                }
            };
            entries.push(entry.file_name().to_string_lossy().into_owned());
        }
        entries.sort();
        self.outcome(
            "file_list",
            "ok",
            0,
            json!({"path":requested,"entries":entries}),
            false,
        )
    }

    fn impact(&self, params: ImpactParams) -> Result<CallToolResult, McpError> {
        self.outcome(
            "impact",
            "ok",
            0,
            json!({"symbol":params.symbol,"status":"available","scope":"local-repo"}),
            false,
        )
    }

    fn lesson_recall(&self, params: LessonRecallParams) -> Result<CallToolResult, McpError> {
        self.outcome(
            "lesson_recall",
            "ok",
            0,
            json!({"context":params.context,"status":"available","scope":"local-repo"}),
            false,
        )
    }

    fn ledger_read(&self, params: LedgerReadParams) -> Result<CallToolResult, McpError> {
        if params.limit == Some(0) {
            return self.outcome(
                "ledger_read",
                "refused",
                EXIT_REFUSAL,
                json!({"reason":"ZERO_LIMIT"}),
                true,
            );
        }
        let rows = match crate::ledger_rows(true) {
            Ok(rows) => rows,
            Err(code) => {
                return self.outcome(
                    "ledger_read",
                    "error",
                    code,
                    json!({"reason":"LEDGER_READ_FAILED"}),
                    true,
                )
            }
        };
        let rows = match params.limit {
            Some(limit) => {
                let limit = usize::try_from(limit)
                    .map_err(|_| McpError::internal_error("limit does not fit usize", None))?;
                rows.into_iter().rev().take(limit).rev().collect::<Vec<_>>()
            }
            None => rows,
        };
        self.outcome(
            "ledger_read",
            "ok",
            0,
            json!({
                "status": if rows.is_empty() { "empty" } else { "ok" },
                "rows":rows,
            }),
            false,
        )
    }
}

impl ServerHandler for LeaseServer {
    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResponse, McpError>> + MaybeSendFuture + '_ {
        let tool = request.name.to_string();
        let result = self
            .outcome(
                &tool,
                "refused",
                EXIT_REFUSAL,
                json!({"reason":"TOOL_NOT_IN_MANIFEST"}),
                true,
            )
            .map(Into::into);
        std::future::ready(result)
    }
}

fn normalize_relative(value: &str) -> Result<PathBuf, ()> {
    let mut path = PathBuf::new();
    for component in Path::new(value).components() {
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => return Err(()),
        }
    }
    if path.as_os_str().is_empty() {
        Err(())
    } else {
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_derived_from_the_lease() {
        let manifest = manifest_for_lease("keel/**").expect("GOOD lease must produce a manifest");
        let tools = manifest
            .get("tools")
            .and_then(Value::as_array)
            .expect("manifest must publish tools");
        assert_eq!(
            manifest.get("lease").and_then(Value::as_str),
            Some("keel/**")
        );
        assert_eq!(tools.len(), 6);
        assert!(tools.iter().all(|tool| {
            tool.get("scope").and_then(Value::as_str) == Some("keel/**")
                || tool.get("scope").and_then(Value::as_str) == Some("fleet")
        }));
    }

    #[test]
    fn traversal_is_rejected_and_in_lease_path_is_accepted() {
        assert!(normalize_relative("keel/src/main.rs").is_ok());
        assert!(normalize_relative("keel/../AGENTS.md").is_err());
        assert!(Lease::parse("../**").is_err());
    }
}
