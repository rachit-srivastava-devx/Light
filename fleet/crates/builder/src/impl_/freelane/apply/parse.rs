//! Extracts fenced code blocks and their declared target from a freelane reply. A target is
//! declared in exactly one of two places, checked in this order: the fence's own info string
//! (` ```path:src/foo.rs `), or a `path:`/`file:` line on the nearest non-blank line above the
//! fence. Anything else leaves `declared_path` `None` -- the caller refuses rather than guessing.

pub struct ProposedFile {
    /// 1-based index of this fence in the reply, for error messages.
    pub fence_index: usize,
    pub declared_path: Option<String>,
    pub content: String,
}

pub fn extract_fences(reply: &str) -> Vec<ProposedFile> {
    let lines: Vec<&str> = reply.lines().collect();
    let mut out = Vec::new();
    let mut fence_index = 0;
    let mut last_nonblank: Option<&str> = None;
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        let Some(info) = trimmed.strip_prefix("```") else {
            if !trimmed.is_empty() {
                last_nonblank = Some(trimmed);
            }
            i += 1;
            continue;
        };
        fence_index += 1;
        let declared_path = info_string_path(info.trim()).or_else(|| last_nonblank.and_then(line_path));
        let mut content = String::new();
        i += 1;
        while i < lines.len() && !lines[i].trim_start().starts_with("```") {
            content.push_str(lines[i]);
            content.push('\n');
            i += 1;
        }
        i += 1; // skip the closing fence (or step past EOF harmlessly)
        out.push(ProposedFile { fence_index, declared_path, content });
        last_nonblank = None;
    }
    out
}

fn info_string_path(info: &str) -> Option<String> {
    line_path(info)
}

/// `path:`/`file:` prefix, case-insensitive, with the value trimmed of surrounding whitespace
/// and any inline-code backticks (`` `path: src/foo.rs` `` is a common way a model phrases it).
fn line_path(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    if !(lower.starts_with("path:") || lower.starts_with("file:")) {
        return None;
    }
    let value = line[5..].trim().trim_matches('`').trim();
    (!value.is_empty()).then(|| value.to_string())
}
