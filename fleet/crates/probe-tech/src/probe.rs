use crate::port::{CodebaseReader, ProbeError};

pub struct TechnicalInput {
    pub text: String,
    pub repo_digest: String,
}

pub struct Question {
    pub text: String,
}

pub fn probe(
    input: &TechnicalInput,
    _reader: &dyn CodebaseReader,
) -> Result<Vec<Question>, ProbeError> {
    let context = extract_code_context(&input.text);
    let question_text = generate_tech_question(&input.text, &context);
    Ok(vec![Question {
        text: question_text,
    }])
}

fn extract_code_context(text: &str) -> String {
    let lang_terms = [
        "function",
        "struct",
        "trait",
        "module",
        "type",
        "interface",
        "dependency",
    ];
    let text_lower = text.to_lowercase();
    let found = lang_terms
        .iter()
        .find(|&&t| text_lower.contains(t))
        .copied()
        .unwrap_or("module");
    format!("Code context: {} analysis needed", found)
}

fn generate_tech_question(text: &str, _context: &str) -> String {
    if text.trim().is_empty() {
        "What module structure and type definitions are needed for this task?".to_string()
    } else {
        format!(
            "What {} and type dependencies exist for: {}?",
            "module",
            text.chars().take(60).collect::<String>()
        )
    }
}
