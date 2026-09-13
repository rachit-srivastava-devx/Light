// TODO: Integrate with fleet-types::Module once parallel module execution is fully wired
#[allow(dead_code)]
/// A unique identifier for a module within a pipeline run.
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ModuleId(String);

#[allow(dead_code)]
impl ModuleId {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err("module_id must not be empty".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
