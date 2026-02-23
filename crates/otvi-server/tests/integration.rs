//! Integration tests that exercise the provider HTTP client against a real
//! httpbin instance running in Docker.
//!
//! These tests are **ignored** by default because they require a running
//! httpbin container.  Start it with:
//!
//! ```sh
//! docker compose -f docker-compose.test.yml up -d
//! ```
//!
//! Then run:
//!
//! ```sh
//! cargo test -p otvi-server --test integration -- --ignored
//! ```
//!
//! httpbin (<https://httpbin.org>) mirrors requests back so we can verify
//! that template resolution, header forwarding, body encoding, and response
//! extraction all work end-to-end.

use std::collections::HashMap;

use otvi_core::config::ProviderConfig;
use otvi_core::template::{TemplateContext, extract_json_path};
use reqwest::Client;
use serde_json::Value;

/// Base URL of the httpbin container started by `docker-compose.test.yml`.
const HTTPBIN_URL: &str = "http://localhost:8888";

/// Helper: build a shared HTTP client.
fn http_client() -> Client {
    Client::builder()
        .build()
        .expect("Failed to build HTTP client")
}

/// Helper: load the httpbin test provider YAML.
fn load_test_provider() -> ProviderConfig {
    let yaml = include_str!("fixtures/httpbin-provider.yaml");
    serde_yaml_ng::from_str(yaml).expect("Failed to parse httpbin-provider.yaml fixture")
}

/// Helper: execute a provider-style HTTP request against httpbin.
///
/// Mirrors the logic of `provider_client::execute_request` but is
/// self-contained so the test doesn't depend on server internals.
async fn execute_request(
    client: &Client,
    base_url: &str,
    default_headers: &HashMap<String, String>,
    spec: &otvi_core::config::RequestSpec,
    context: &TemplateContext,
) -> anyhow::Result<Value> {
    let url = format!("{}{}", base_url, context.resolve(&spec.path));

    let mut builder = match spec.method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        other => anyhow::bail!("Unsupported HTTP method: {other}"),
    };

    for (k, v) in default_headers {
        builder = builder.header(k, context.resolve(v));
    }
    for (k, v) in &spec.headers {
        builder = builder.header(k, context.resolve(v));
    }
    for (k, v) in &spec.params {
        let resolved = context.resolve(v);
        if !resolved.contains("{{") {
            builder = builder.query(&[(k.as_str(), resolved.as_str())]);
        }
    }
    if let Some(body) = &spec.body {
        let resolved_body = context.resolve(body);
        if spec.body_encoding == "form" {
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
    let body: Value = response.json().await?;
    Ok(body)
}

// ── Tests ───────────────────────────────────────────────────────────────────

/// Verify httpbin is reachable.
#[tokio::test]
#[ignore = "requires httpbin Docker container"]
async fn httpbin_is_reachable() {
    let resp = http_client()
        .get(format!("{HTTPBIN_URL}/get"))
        .send()
        .await
        .expect("httpbin should be reachable");
    assert!(resp.status().is_success());
}

/// POST /post with a JSON body – httpbin echoes it back under `$.json`.
#[tokio::test]
#[ignore = "requires httpbin Docker container"]
async fn post_json_body_echoed() {
    let provider = load_test_provider();
    let flow = &provider.auth.flows[0];
    let step = &flow.steps[0];

    let mut ctx = TemplateContext::new();
    ctx.set("input.username", "alice");
    ctx.set("input.password", "s3cret");

    let body = execute_request(
        &http_client(),
        &provider.defaults.base_url,
        &provider.defaults.headers,
        &step.request,
        &ctx,
    )
    .await
    .expect("POST /post should succeed");

    // httpbin returns the posted JSON under $.json
    let username = extract_json_path(&body, "$.json.username");
    let password = extract_json_path(&body, "$.json.password");
    assert_eq!(username.as_deref(), Some("alice"));
    assert_eq!(password.as_deref(), Some("s3cret"));
}

/// Verify that template variables in the auth step's `on_success.extract`
/// correctly extract values from the httpbin response.
#[tokio::test]
#[ignore = "requires httpbin Docker container"]
async fn auth_step_extraction_works() {
    let provider = load_test_provider();
    let flow = &provider.auth.flows[0];
    let step = &flow.steps[0];

    let mut ctx = TemplateContext::new();
    ctx.set("input.username", "bob");
    ctx.set("input.password", "hunter2");

    let body = execute_request(
        &http_client(),
        &provider.defaults.base_url,
        &provider.defaults.headers,
        &step.request,
        &ctx,
    )
    .await
    .expect("POST /post should succeed");

    // Simulate the on_success extraction
    let on_success = step.on_success.as_ref().expect("on_success should exist");
    let mut extracted: HashMap<String, String> = HashMap::new();
    for (key, path) in &on_success.extract {
        if let Some(value) = extract_json_path(&body, path) {
            extracted.insert(key.clone(), value);
        }
    }

    assert_eq!(
        extracted.get("access_token").map(|s| s.as_str()),
        Some("bob")
    );
}

/// GET /get with custom headers – httpbin echoes them back.
#[tokio::test]
#[ignore = "requires httpbin Docker container"]
async fn default_headers_are_forwarded() {
    let client = http_client();
    let resp: Value = client
        .get(format!("{HTTPBIN_URL}/get"))
        .header("User-Agent", "otvi-test/1.0")
        .header("X-Custom", "test-value")
        .send()
        .await
        .expect("GET /get should succeed")
        .json()
        .await
        .expect("response should be JSON");

    // httpbin returns headers under $.headers
    let ua = extract_json_path(&resp, "$.headers.User-Agent");
    assert_eq!(ua.as_deref(), Some("otvi-test/1.0"));

    let custom = extract_json_path(&resp, "$.headers.X-Custom");
    assert_eq!(custom.as_deref(), Some("test-value"));
}

/// GET /get with query parameters – httpbin echoes them back under $.args.
#[tokio::test]
#[ignore = "requires httpbin Docker container"]
async fn query_params_are_forwarded() {
    let provider = load_test_provider();
    let playback_spec = &provider.playback.stream.request;

    let mut ctx = TemplateContext::new();
    ctx.set("input.channel_id", "ch42");

    let body = execute_request(
        &http_client(),
        &provider.defaults.base_url,
        &provider.defaults.headers,
        playback_spec,
        &ctx,
    )
    .await
    .expect("GET /get should succeed");

    let channel_id = extract_json_path(&body, "$.args.channel_id");
    assert_eq!(channel_id.as_deref(), Some("ch42"));
}

/// GET /json returns a fixed JSON response that we can map through
/// the channel list response mapping.
#[tokio::test]
#[ignore = "requires httpbin Docker container"]
async fn channel_list_mapping_works() {
    let provider = load_test_provider();
    let list_spec = &provider.channels.list;

    let ctx = TemplateContext::new();
    let body = execute_request(
        &http_client(),
        &provider.defaults.base_url,
        &provider.defaults.headers,
        &list_spec.request,
        &ctx,
    )
    .await
    .expect("GET /json should succeed");

    // Navigate to items_path: $.slideshow.slides
    let items_path = list_spec
        .response
        .items_path
        .as_deref()
        .expect("items_path should be set");
    let path = items_path.strip_prefix("$.").unwrap_or(items_path);
    let mut current = &body;
    for part in path.split('.') {
        current = current.get(part).expect("path segment should exist");
    }

    let items = current.as_array().expect("items should be an array");
    assert!(
        !items.is_empty(),
        "slideshow should have at least one slide"
    );

    // Extract mapped fields from the first item
    let first = &items[0];
    let mapping = &list_spec.response.mapping;
    if let Some(name_path) = mapping.get("name") {
        let name = extract_json_path(first, name_path);
        assert!(name.is_some(), "channel name should be extracted");
    }
}

/// POST /post with form-encoded body (body_encoding: "form").
#[tokio::test]
#[ignore = "requires httpbin Docker container"]
async fn form_encoded_body_works() {
    let client = http_client();
    let spec = otvi_core::config::RequestSpec {
        method: "POST".to_string(),
        path: "/post".to_string(),
        headers: HashMap::new(),
        params: HashMap::new(),
        body: Some("key1=value1&key2=value2".to_string()),
        body_encoding: "form".to_string(),
    };

    let ctx = TemplateContext::new();
    let body = execute_request(&client, HTTPBIN_URL, &HashMap::new(), &spec, &ctx)
        .await
        .expect("POST /post with form body should succeed");

    // httpbin returns form data under $.form
    let key1 = extract_json_path(&body, "$.form.key1");
    let key2 = extract_json_path(&body, "$.form.key2");
    assert_eq!(key1.as_deref(), Some("value1"));
    assert_eq!(key2.as_deref(), Some("value2"));
}

/// Verify template resolution integrates correctly with a real request.
#[tokio::test]
#[ignore = "requires httpbin Docker container"]
async fn template_resolution_in_request() {
    let client = http_client();
    let spec = otvi_core::config::RequestSpec {
        method: "POST".to_string(),
        path: "/post".to_string(),
        headers: HashMap::new(),
        params: HashMap::new(),
        body: Some(r#"{"token":"{{stored.access_token}}","device":"{{device_id}}"}"#.to_string()),
        body_encoding: "json".to_string(),
    };

    let mut ctx = TemplateContext::new();
    ctx.set("stored.access_token", "tok_abc123");
    ctx.set("device_id", "dev_xyz789");

    let body = execute_request(&client, HTTPBIN_URL, &HashMap::new(), &spec, &ctx)
        .await
        .expect("POST /post should succeed");

    let token = extract_json_path(&body, "$.json.token");
    let device = extract_json_path(&body, "$.json.device");
    assert_eq!(token.as_deref(), Some("tok_abc123"));
    assert_eq!(device.as_deref(), Some("dev_xyz789"));
}
