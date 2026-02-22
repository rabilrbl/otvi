//! HTTP client that executes provider API requests according to a
//! [`RequestSpec`] and a [`TemplateContext`].

use std::collections::HashMap;

use otvi_core::config::RequestSpec;
use otvi_core::template::TemplateContext;
use reqwest::Client;
use serde_json::Value;

/// Result of executing a provider request, including the HTTP status code.
#[allow(dead_code)]
pub struct ProviderResponse {
    pub status: u16,
    pub body: Value,
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
    let url = format!("{}{}", base_url, context.resolve(&spec.path));

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
        builder = builder.header(k, context.resolve(v));
    }

    // Per-request headers (may override defaults)
    for (k, v) in &spec.headers {
        builder = builder.header(k, context.resolve(v));
    }

    // Query parameters – skip any that still contain unresolved `{{…}}`
    for (k, v) in &spec.params {
        let resolved = context.resolve(v);
        if !resolved.contains("{{") {
            builder = builder.query(&[(k.as_str(), resolved.as_str())]);
        }
    }

    // Body
    if let Some(body) = &spec.body {
        let resolved_body = context.resolve(body);
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
    let status = response.status();
    let status_code = status.as_u16();
    let body: Value = response.json().await.unwrap_or(Value::Null);

    if !status.is_success() {
        anyhow::bail!("Provider API returned {status}: {body}");
    }

    Ok(ProviderResponse {
        status: status_code,
        body,
    })
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
