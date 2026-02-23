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
            self.values.insert(format!("{prefix}.{k}"), v.clone());
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

    #[test]
    fn merge_adds_prefixed_keys() {
        let mut ctx = TemplateContext::new();
        let mut map = HashMap::new();
        map.insert("email".into(), "a@b.com".into());
        map.insert("pass".into(), "secret".into());
        ctx.merge("input", &map);

        let result = ctx.resolve("{{input.email}} {{input.pass}}");
        assert_eq!(result, "a@b.com secret");
    }

    #[test]
    fn merge_overwrites_existing_key() {
        let mut ctx = TemplateContext::new();
        ctx.set("input.email", "old@example.com");
        let mut map = HashMap::new();
        map.insert("email".into(), "new@example.com".into());
        ctx.merge("input", &map);

        assert_eq!(ctx.resolve("{{input.email}}"), "new@example.com");
    }

    #[test]
    fn resolve_unresolved_placeholders_left_as_is() {
        let mut ctx = TemplateContext::new();
        ctx.set("a", "1");
        let result = ctx.resolve("{{a}} and {{b}}");
        assert_eq!(result, "1 and {{b}}");
    }

    #[test]
    fn resolve_empty_context() {
        let ctx = TemplateContext::new();
        let result = ctx.resolve("{{foo}} stays {{bar}}");
        assert_eq!(result, "{{foo}} stays {{bar}}");
    }

    #[test]
    fn resolve_no_placeholders() {
        let mut ctx = TemplateContext::new();
        ctx.set("key", "val");
        assert_eq!(ctx.resolve("plain text"), "plain text");
    }

    #[test]
    fn extract_array_index() {
        let data = json!({"items": ["zero", "one", "two"]});
        assert_eq!(extract_json_path(&data, "$.items.0"), Some("zero".into()));
        assert_eq!(extract_json_path(&data, "$.items.2"), Some("two".into()));
    }

    #[test]
    fn extract_array_out_of_bounds() {
        let data = json!({"items": [1, 2]});
        assert_eq!(extract_json_path(&data, "$.items.5"), None);
    }

    #[test]
    fn extract_array_nested_object() {
        let data = json!({"users": [{"name": "Alice"}, {"name": "Bob"}]});
        assert_eq!(
            extract_json_path(&data, "$.users.1.name"),
            Some("Bob".into())
        );
    }

    #[test]
    fn extract_number() {
        let data = json!({"count": 42});
        assert_eq!(extract_json_path(&data, "$.count"), Some("42".into()));
    }

    #[test]
    fn extract_boolean() {
        let data = json!({"active": true});
        assert_eq!(extract_json_path(&data, "$.active"), Some("true".into()));
    }

    #[test]
    fn extract_null_returns_none() {
        let data = json!({"val": null});
        assert_eq!(extract_json_path(&data, "$.val"), None);
    }

    #[test]
    fn extract_object_returns_json_string() {
        let data = json!({"nested": {"a": 1}});
        let result = extract_json_path(&data, "$.nested").unwrap();
        assert!(result.contains("\"a\""));
        assert!(result.contains("1"));
    }

    #[test]
    fn extract_without_dollar_prefix() {
        let data = json!({"data": {"token": "xyz"}});
        assert_eq!(
            extract_json_path(&data, "data.token"),
            Some("xyz".into())
        );
    }

    #[test]
    fn extract_single_key() {
        let data = json!({"key": "value"});
        assert_eq!(extract_json_path(&data, "key"), Some("value".into()));
    }
}
