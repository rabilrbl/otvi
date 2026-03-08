//! A lightweight template engine for resolving `{{variable}}` placeholders.
//!
//! Supported variable prefixes:
//! - `{{input.X}}`   – values provided by the end-user (form fields)
//! - `{{stored.X}}`  – values persisted in the server-side session
//! - `{{extract.X}}` – values extracted in earlier auth steps
//! - `{{uuid}}`      – replaced by a generated UUID on the server
//!
//! ## Unresolved placeholders
//!
//! `resolve` returns a `ResolveResult` that carries both the rendered string
//! and a list of placeholder keys that had no matching binding.  Use
//! `resolve_lossy` when you want the old "leave as-is" behaviour without
//! inspecting the warnings.

use serde_json::Value;
use std::collections::HashMap;

// ── Template context ──────────────────────────────────────────────────────────

/// Holds all variable bindings for template resolution.
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    values: HashMap<String, String>,
}

/// The result of resolving a template string.
#[derive(Debug, Clone)]
pub struct ResolveResult {
    /// The rendered string (unresolved placeholders are left as-is).
    pub rendered: String,
    /// Keys that appeared in the template but had no binding.
    pub unresolved: Vec<String>,
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

    /// Resolve all `{{key}}` placeholders in `template`.
    ///
    /// Returns a [`ResolveResult`] containing both the rendered string and
    /// a (possibly empty) list of placeholder keys that could not be resolved.
    /// Unresolved placeholders are left verbatim in `rendered`.
    pub fn resolve(&self, template: &str) -> ResolveResult {
        let mut rendered = String::with_capacity(template.len());
        let mut unresolved = Vec::new();

        let mut remaining = template;
        while let Some(start) = remaining.find("{{") {
            let (prefix, after_prefix) = remaining.split_at(start);
            rendered.push_str(prefix);

            let after_open = &after_prefix[2..];
            let Some(end) = after_open.find("}}") else {
                rendered.push_str(after_prefix);
                return ResolveResult {
                    rendered,
                    unresolved,
                };
            };

            let key = &after_open[..end];
            if let Some(value) = self.values.get(key) {
                rendered.push_str(value);
            } else {
                unresolved.push(key.to_string());
                rendered.push_str("{{");
                rendered.push_str(key);
                rendered.push_str("}}");
            }

            remaining = &after_open[end + 2..];
        }

        rendered.push_str(remaining);

        ResolveResult {
            rendered,
            unresolved,
        }
    }

    /// Convenience wrapper: resolve and return only the rendered string.
    /// Unresolved placeholders are silently left as-is (backwards-compatible).
    pub fn resolve_lossy(&self, template: &str) -> String {
        self.resolve(template).rendered
    }
}

// ── JSONPath extraction ───────────────────────────────────────────────────────

/// Extract a scalar value from a JSON tree using a JSONPath expression.
///
/// Full JSONPath syntax is supported via the `jsonpath-rust` crate.  For
/// convenience, the function also handles simple dot-notation paths that do
/// **not** start with `$` (e.g. `data.token`) by prepending `$.` automatically.
///
/// When the path matches multiple nodes the first value is returned.
///
/// # Examples
/// ```
/// use serde_json::json;
/// use otvi_core::template::extract_json_path;
///
/// let data = json!({"data": {"token": "abc123"}});
/// assert_eq!(extract_json_path(&data, "$.data.token"), Some("abc123".into()));
///
/// // Filter expression
/// let list = json!({"items": [{"id": 1, "name": "a"}, {"id": 2, "name": "b"}]});
/// assert_eq!(extract_json_path(&list, "$.items[?(@.id == 2)].name"), Some("b".into()));
/// ```
pub fn extract_json_path(json: &Value, path: &str) -> Option<String> {
    // Normalise: ensure the path starts with "$."
    let normalised: String = if path.starts_with("$.") || path == "$" || path.starts_with("$[") {
        path.to_string()
    } else {
        format!("$.{path}")
    };

    // Try full JSONPath via jsonpath-rust 1.x trait-based API.
    // `JsonPath::query` is implemented directly on `serde_json::Value` and
    // returns `Queried<Vec<&Value>>` (Ok on valid paths, Err on parse failure).
    use jsonpath_rust::JsonPath;

    match json.query(&normalised) {
        Ok(results) => results.into_iter().next().and_then(scalar_to_string),
        // If the JSONPath parser rejects the expression fall back to the
        // original simple dot-notation walk so existing configs keep working.
        Err(_) => extract_dot_path(json, path),
    }
}

/// Scalar JSON value → `String`.  Returns `None` for `null` and objects/arrays
/// (unless you want the raw JSON of a complex node; handle that at call site).
fn scalar_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

/// Fallback simple dot-notation walker (no filter expressions).
/// Paths may start with `$.` (stripped) or be bare (`data.token`).
fn extract_dot_path(json: &Value, path: &str) -> Option<String> {
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

    scalar_to_string(current)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── TemplateContext ───────────────────────────────────────────────────

    #[test]
    fn resolve_simple() {
        let mut ctx = TemplateContext::new();
        ctx.set("input.email", "user@example.com");
        ctx.set("input.password", "secret");
        let r = ctx.resolve(r#"{"email":"{{input.email}}","password":"{{input.password}}"}"#);
        assert_eq!(
            r.rendered,
            r#"{"email":"user@example.com","password":"secret"}"#
        );
        assert!(r.unresolved.is_empty());
    }

    #[test]
    fn resolve_reports_unresolved_placeholders() {
        let mut ctx = TemplateContext::new();
        ctx.set("a", "1");
        let r = ctx.resolve("{{a}} and {{b}}");
        assert_eq!(r.rendered, "1 and {{b}}");
        assert_eq!(r.unresolved, vec!["b".to_string()]);
    }

    #[test]
    fn resolve_lossy_leaves_unresolved_as_is() {
        let ctx = TemplateContext::new();
        let rendered = ctx.resolve_lossy("{{foo}} stays {{bar}}");
        assert_eq!(rendered, "{{foo}} stays {{bar}}");
    }

    #[test]
    fn resolve_empty_context_all_unresolved() {
        let ctx = TemplateContext::new();
        let r = ctx.resolve("{{foo}} stays {{bar}}");
        assert_eq!(r.rendered, "{{foo}} stays {{bar}}");
        let mut keys = r.unresolved.clone();
        keys.sort();
        assert_eq!(keys, vec!["bar".to_string(), "foo".to_string()]);
    }

    #[test]
    fn resolve_no_placeholders_empty_unresolved() {
        let mut ctx = TemplateContext::new();
        ctx.set("key", "val");
        let r = ctx.resolve("plain text");
        assert_eq!(r.rendered, "plain text");
        assert!(r.unresolved.is_empty());
    }

    #[test]
    fn merge_adds_prefixed_keys() {
        let mut ctx = TemplateContext::new();
        let mut map = HashMap::new();
        map.insert("email".into(), "a@b.com".into());
        map.insert("pass".into(), "secret".into());
        ctx.merge("input", &map);

        let r = ctx.resolve("{{input.email}} {{input.pass}}");
        assert_eq!(r.rendered, "a@b.com secret");
        assert!(r.unresolved.is_empty());
    }

    #[test]
    fn merge_overwrites_existing_key() {
        let mut ctx = TemplateContext::new();
        ctx.set("input.email", "old@example.com");
        let mut map = HashMap::new();
        map.insert("email".into(), "new@example.com".into());
        ctx.merge("input", &map);

        assert_eq!(ctx.resolve_lossy("{{input.email}}"), "new@example.com");
    }

    // ── extract_json_path – simple dot-notation ───────────────────────────

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
    fn extract_array_index() {
        let data = json!({"items": ["zero", "one", "two"]});
        assert_eq!(extract_json_path(&data, "$.items[0]"), Some("zero".into()));
        assert_eq!(extract_json_path(&data, "$.items[2]"), Some("two".into()));
    }

    #[test]
    fn extract_array_out_of_bounds() {
        let data = json!({"items": [1, 2]});
        assert_eq!(extract_json_path(&data, "$.items[5]"), None);
    }

    #[test]
    fn extract_array_nested_object() {
        let data = json!({"users": [{"name": "Alice"}, {"name": "Bob"}]});
        assert_eq!(
            extract_json_path(&data, "$.users[1].name"),
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
        assert_eq!(extract_json_path(&data, "data.token"), Some("xyz".into()));
    }

    #[test]
    fn extract_single_key() {
        let data = json!({"key": "value"});
        assert_eq!(extract_json_path(&data, "key"), Some("value".into()));
    }

    // ── extract_json_path – full JSONPath expressions ─────────────────────

    #[test]
    fn extract_filter_expression() {
        let data = json!({
            "items": [
                {"id": 1, "name": "alpha"},
                {"id": 2, "name": "beta"}
            ]
        });
        assert_eq!(
            extract_json_path(&data, "$.items[?(@.id == 2)].name"),
            Some("beta".into())
        );
    }

    #[test]
    fn extract_wildcard_returns_first() {
        let data = json!({"scores": [10, 20, 30]});
        // $.scores[*] matches all; we return the first one
        assert_eq!(extract_json_path(&data, "$.scores[*]"), Some("10".into()));
    }

    #[test]
    fn extract_recursive_descent() {
        let data = json!({
            "level1": {
                "level2": {
                    "target": "found"
                }
            }
        });
        assert_eq!(extract_json_path(&data, "$..target"), Some("found".into()));
    }
}
