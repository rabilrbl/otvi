//! HTTP client that executes provider API requests according to a
//! [`RequestSpec`] and a [`TemplateContext`].

use std::collections::HashMap;

use otvi_core::config::RequestSpec;
use otvi_core::template::TemplateContext;
use reqwest::Client;
use reqwest::header::SET_COOKIE;
use serde_json::Value;
use tracing::warn;

const STORED_COOKIE_PREFIX: &str = "stored.__cookie_";

/// Result of executing a provider request, including the HTTP status code.
#[derive(Debug)]
#[allow(dead_code)]
pub struct ProviderResponse {
    pub status: u16,
    pub body: Value,
    pub cookies: HashMap<String, String>,
}

/// Resolve `template` against `context`, emitting a `tracing::warn` for every
/// placeholder that could not be resolved.
fn resolve_warn(context: &TemplateContext, template: &str, field: &str) -> String {
    let result = context.resolve(template);
    if !result.unresolved.is_empty() {
        warn!(
            field,
            unresolved = ?result.unresolved,
            "Unresolved template placeholders in provider request – check your YAML config"
        );
    }
    result.rendered
}

async fn parse_response_body(response: reqwest::Response) -> anyhow::Result<(u16, Value)> {
    let status = response.status();
    let status_code = status.as_u16();
    let bytes = response.bytes().await?;

    if bytes.is_empty() {
        return Ok((status_code, Value::Null));
    }

    match serde_json::from_slice::<Value>(&bytes) {
        Ok(json) => Ok((status_code, json)),
        Err(_) => {
            let text = String::from_utf8_lossy(&bytes).into_owned();
            Ok((status_code, Value::String(text)))
        }
    }
}

/// Build and send an HTTP request described by `spec`, resolving template
/// variables from `context`.  Default headers are merged first, then
/// per-request headers override them.
pub async fn execute_request(
    client: &Client,
    base_url: &str,
    default_headers: &HashMap<String, String>,
    spec: &RequestSpec,
    context: &TemplateContext,
) -> anyhow::Result<ProviderResponse> {
    let path = resolve_warn(context, &spec.path, "path");
    let url = format!("{}{}", base_url, path);

    let mut builder = match spec.method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        other => anyhow::bail!("Unsupported HTTP method: {other}"),
    };

    // Default headers
    for (k, v) in default_headers {
        builder = builder.header(k, resolve_warn(context, v, "default_header"));
    }

    // Per-request headers (may override defaults)
    for (k, v) in &spec.headers {
        builder = builder.header(k, resolve_warn(context, v, "header"));
    }

    let explicit_cookie_header = default_headers
        .keys()
        .chain(spec.headers.keys())
        .any(|key| key.eq_ignore_ascii_case("cookie"));
    if !explicit_cookie_header {
        let cookie_pairs = context
            .values_with_prefix(STORED_COOKIE_PREFIX)
            .into_iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>();
        if !cookie_pairs.is_empty() {
            builder = builder.header("Cookie", cookie_pairs.join("; "));
        }
    }

    // Query parameters – skip any that still contain unresolved `{{…}}`
    for (k, v) in &spec.params {
        let resolved = resolve_warn(context, v, "param");
        if !resolved.contains("{{") {
            builder = builder.query(&[(k.as_str(), resolved.as_str())]);
        }
    }

    // Body
    if let Some(body) = &spec.body {
        let resolved_body = resolve_warn(context, body, "body");
        if spec.body_encoding == "form" {
            // Parse the resolved body as key=value pairs and send as form data
            let pairs: Vec<(String, String)> = resolved_body
                .split('&')
                .filter_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    let key = parts.next()?.trim().to_string();
                    let value = parts.next().unwrap_or("").trim().to_string();
                    if key.is_empty() {
                        None
                    } else {
                        Some((key, value))
                    }
                })
                .collect();
            builder = builder.form(&pairs);
        } else {
            builder = builder.body(resolved_body);
        }
    }

    let response = builder.send().await?;
    let cookies = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(parse_set_cookie)
        .collect::<HashMap<_, _>>();
    let status = response.status();
    let (status_code, body) = parse_response_body(response).await?;

    if !status.is_success() {
        anyhow::bail!("Provider API returned non-success status {status_code}");
    }

    Ok(ProviderResponse {
        status: status_code,
        body,
        cookies,
    })
}

fn parse_set_cookie(header: &str) -> Option<(String, String)> {
    let first = header.split(';').next()?.trim();
    let mut parts = first.splitn(2, '=');
    let name = parts.next()?.trim();
    let value = parts.next()?.trim();
    if name.is_empty() {
        return None;
    }
    Some((name.to_string(), value.to_string()))
}

/// Convenience wrapper that returns just the JSON body (backwards-compatible).
pub async fn execute_request_body(
    client: &Client,
    base_url: &str,
    default_headers: &HashMap<String, String>,
    spec: &RequestSpec,
    context: &TemplateContext,
) -> anyhow::Result<Value> {
    let resp = execute_request(client, base_url, default_headers, spec, context).await?;
    Ok(resp.body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use otvi_core::config::RequestSpec;
    use otvi_core::template::TemplateContext;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn minimal_spec(http_method: &str, endpoint_path: &str) -> RequestSpec {
        RequestSpec {
            method: http_method.into(),
            path: endpoint_path.into(),
            headers: Default::default(),
            params: Default::default(),
            body: None,
            body_encoding: "json".into(),
        }
    }

    #[tokio::test]
    async fn test_execute_request_get() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let ctx = TemplateContext::new();
        let spec = minimal_spec("GET", "/test");
        let result = execute_request(&client, &server.uri(), &Default::default(), &spec, &ctx)
            .await
            .unwrap();
        assert_eq!(result.status, 200);
        assert_eq!(result.body["ok"], true);
    }

    #[tokio::test]
    async fn test_execute_request_post_json() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/submit"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"received": true})),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let mut ctx = TemplateContext::new();
        ctx.set("input.name", "Alice");
        let mut spec = minimal_spec("POST", "/submit");
        spec.body = Some(r#"{"name":"{{input.name}}"}"#.into());

        let result = execute_request(&client, &server.uri(), &Default::default(), &spec, &ctx)
            .await
            .unwrap();
        assert_eq!(result.status, 200);
    }

    #[tokio::test]
    async fn test_execute_request_post_form() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/form"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let mut ctx = TemplateContext::new();
        ctx.set("input.user", "bob");
        let mut spec = minimal_spec("POST", "/form");
        spec.body = Some("user={{input.user}}&active=1".into());
        spec.body_encoding = "form".into();

        let result = execute_request(&client, &server.uri(), &Default::default(), &spec, &ctx)
            .await
            .unwrap();
        assert_eq!(result.status, 200);
    }

    #[tokio::test]
    async fn test_execute_request_with_headers() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/secure"))
            .and(wiremock::matchers::header("X-Token", "tok123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let mut ctx = TemplateContext::new();
        ctx.set("stored.token", "tok123");
        let mut spec = minimal_spec("GET", "/secure");
        spec.headers
            .insert("X-Token".into(), "{{stored.token}}".into());

        let result = execute_request(&client, &server.uri(), &Default::default(), &spec, &ctx)
            .await
            .unwrap();
        assert_eq!(result.status, 200);
    }

    #[tokio::test]
    async fn test_execute_request_with_query_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .and(wiremock::matchers::query_param("q", "rust"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"results": []})),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let mut ctx = TemplateContext::new();
        ctx.set("input.q", "rust");
        let mut spec = minimal_spec("GET", "/search");
        spec.params.insert("q".into(), "{{input.q}}".into());

        let result = execute_request(&client, &server.uri(), &Default::default(), &spec, &ctx)
            .await
            .unwrap();
        assert_eq!(result.status, 200);
    }

    #[tokio::test]
    async fn test_execute_request_skips_unresolved_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/data"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let ctx = TemplateContext::new(); // no bindings → param unresolved
        let mut spec = minimal_spec("GET", "/data");
        spec.params
            .insert("token".into(), "{{stored.token}}".into());

        // Should not fail; unresolved param is simply dropped
        let result = execute_request(&client, &server.uri(), &Default::default(), &spec, &ctx)
            .await
            .unwrap();
        assert_eq!(result.status, 200);
    }

    #[tokio::test]
    async fn test_execute_request_put() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/item/1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"updated": true})),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let ctx = TemplateContext::new();
        let spec = minimal_spec("PUT", "/item/1");
        let result = execute_request(&client, &server.uri(), &Default::default(), &spec, &ctx)
            .await
            .unwrap();
        assert_eq!(result.status, 200);
    }

    #[tokio::test]
    async fn test_execute_request_delete() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/item/2"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"deleted": true})),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let ctx = TemplateContext::new();
        let spec = minimal_spec("DELETE", "/item/2");
        let result = execute_request(&client, &server.uri(), &Default::default(), &spec, &ctx)
            .await
            .unwrap();
        assert_eq!(result.status, 200);
    }

    #[tokio::test]
    async fn test_execute_request_patch() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/item/3"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"patched": true})),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let ctx = TemplateContext::new();
        let spec = minimal_spec("PATCH", "/item/3");
        let result = execute_request(&client, &server.uri(), &Default::default(), &spec, &ctx)
            .await
            .unwrap();
        assert_eq!(result.status, 200);
    }

    #[tokio::test]
    async fn test_execute_request_unsupported_method() {
        let client = reqwest::Client::new();
        let ctx = TemplateContext::new();
        let spec = minimal_spec("OPTIONS", "/anything");
        let result = execute_request(
            &client,
            "http://localhost",
            &Default::default(),
            &spec,
            &ctx,
        )
        .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported HTTP method")
        );
    }

    #[tokio::test]
    async fn test_execute_request_error_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/fail"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(serde_json::json!({"error": "unauthorized"})),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let ctx = TemplateContext::new();
        let spec = minimal_spec("GET", "/fail");
        let result =
            execute_request(&client, &server.uri(), &Default::default(), &spec, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("401"));
    }

    #[tokio::test]
    async fn test_execute_request_plain_text_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/text"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let ctx = TemplateContext::new();
        let spec = minimal_spec("GET", "/text");
        let result = execute_request(&client, &server.uri(), &Default::default(), &spec, &ctx)
            .await
            .unwrap();
        assert_eq!(result.status, 200);
        assert_eq!(result.body, Value::String("ok".into()));
    }

    #[tokio::test]
    async fn test_execute_request_body_convenience() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/post"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let ctx = TemplateContext::new();
        let spec = minimal_spec("POST", "/post");
        let body = execute_request_body(&client, &server.uri(), &Default::default(), &spec, &ctx)
            .await
            .unwrap();
        assert_eq!(body["id"], 42);
    }

    #[tokio::test]
    async fn test_execute_request_template_in_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/42"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"channel": 42})),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let mut ctx = TemplateContext::new();
        ctx.set("input.id", "42");
        let spec = minimal_spec("GET", "/channels/{{input.id}}");
        let result = execute_request(&client, &server.uri(), &Default::default(), &spec, &ctx)
            .await
            .unwrap();
        assert_eq!(result.status, 200);
        assert_eq!(result.body["channel"], 42);
    }
}
