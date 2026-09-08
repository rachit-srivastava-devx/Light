//! `language_for` (graph.rs:767-774, retyped to this crate's `Language` enum).

use crate::types::Language;
use std::path::Path;

pub fn language_for(path: &Path) -> Option<Language> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => Some(Language::Rust),
        Some("sh") | Some("bash") => Some(Language::Bash),
        Some("py") => Some(Language::Python),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_extensions() {
        assert_eq!(language_for(Path::new("a.rs")), Some(Language::Rust));
        assert_eq!(language_for(Path::new("a.py")), Some(Language::Python));
        assert_eq!(language_for(Path::new("a.sh")), Some(Language::Bash));
        assert_eq!(language_for(Path::new("a.txt")), None);
    }
}
