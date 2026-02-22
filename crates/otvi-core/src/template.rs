//! A lightweight template engine for resolving `{{variable}}` placeholders.
//!
//! Supported variable prefixes:
//! - `{{input.X}}`   – values provided by the end-user (form fields)
//! - `{{stored.X}}`  – values persisted in the server-side session
//! - `{{extract.X}}` – values extracted in earlier auth steps
//! - `{{uuid}}`      – replaced by a generated UUID on the server

use serde_json::Value;
use std::collections::HashMap;

/// Holds all variable bindings for template resolution.
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    values: HashMap<String, String>,
}

impl TemplateContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a single key-value binding.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.values.insert(key.into(), value.into());
    }

    /// Merge a map of values under a common prefix (e.g. `"input"`, `"stored"`).
    pub fn merge(&mut self, prefix: &str, map: &HashMap<String, String>) {
        for (k, v) in map {
            self.values
                .insert(format!("{prefix}.{k}"), v.clone());
        }
    }

    /// Replace every `{{key}}` occurrence in `template` with the corresponding
    /// value from this context.  Unresolved placeholders are left as-is.
    pub fn resolve(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (key, value) in &self.values {
            result = result.replace(&format!("{{{{{key}}}}}"), value);
        }
        result
    }
}

/// Extract a scalar value from a JSON tree using a simple dot-notation path.
///
/// Paths may start with `$.` (which is stripped) for JSONPath compatibility.
///
/// # Examples
/// ```
/// use serde_json::json;
/// use otvi_core::template::extract_json_path;
///
/// let data = json!({"data": {"token": "abc123"}});
/// assert_eq!(extract_json_path(&data, "$.data.token"), Some("abc123".into()));
/// ```
pub fn extract_json_path(json: &Value, path: &str) -> Option<String> {
    let path = path.strip_prefix("$.").unwrap_or(path);
    let mut current = json;

    for part in path.split('.') {
        match current {
            Value::Object(map) => {
                current = map.get(part)?;
            }
            Value::Array(arr) => {
                let index: usize = part.parse().ok()?;
                current = arr.get(index)?;
            }
            _ => return None,
        }
    }

    match current {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_simple() {
        let mut ctx = TemplateContext::new();
        ctx.set("input.email", "user@example.com");
        ctx.set("input.password", "secret");
        let result = ctx.resolve(r#"{"email":"{{input.email}}","password":"{{input.password}}"}"#);
        assert_eq!(
            result,
            r#"{"email":"user@example.com","password":"secret"}"#
        );
    }

    #[test]
    fn extract_nested_path() {
        let data = json!({
            "data": {
                "user": {
                    "name": "Alice"
                }
            }
        });
        assert_eq!(
            extract_json_path(&data, "$.data.user.name"),
            Some("Alice".into())
        );
    }

    #[test]
    fn extract_missing_path() {
        let data = json!({"a": 1});
        assert_eq!(extract_json_path(&data, "$.b.c"), None);
    }
}
