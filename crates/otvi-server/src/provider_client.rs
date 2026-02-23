//! HTTP client that executes provider API requests according to a
//! [`RequestSpec`] and a [`TemplateContext`].

use std::collections::HashMap;

use otvi_core::config::RequestSpec;
use otvi_core::template::TemplateContext;
use reqwest::Client;
use serde_json::Value;

/// Result of executing a provider request, including the HTTP status code.
#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use otvi_core::config::RequestSpec;
    use otvi_core::template::TemplateContext;
    use serde_json::json;
    use wiremock::matchers::{body_string, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_execute_request_get() {
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let mut context = TemplateContext::new();
        context.set("stored.channel_id", "123");

        Mock::given(method("GET"))
            .and(path("/api/channels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"channels": []})))
            .mount(&mock_server)
            .await;

        let spec = RequestSpec {
            method: "GET".to_string(),
            path: "/api/channels".to_string(),
            headers: HashMap::new(),
            params: HashMap::new(),
            body: None,
            body_encoding: "json".to_string(),
        };

        let response = execute_request(
            &client,
            &mock_server.uri(),
            &HashMap::new(),
            &spec,
            &context,
        )
        .await
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, json!({"channels": []}));
    }

    #[tokio::test]
    async fn test_execute_request_post_json() {
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let mut context = TemplateContext::new();
        context.set("input.email", "test@example.com");
        context.set("input.password", "secret");

        let body_json = json!({"email": "test@example.com", "password": "secret"}).to_string();

        Mock::given(method("POST"))
            .and(path("/api/login"))
            .and(body_string(body_json.clone()))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"token": "abc123"})))
            .mount(&mock_server)
            .await;

        let spec = RequestSpec {
            method: "POST".to_string(),
            path: "/api/login".to_string(),
            headers: HashMap::new(),
            params: HashMap::new(),
            body: Some(body_json),
            body_encoding: "json".to_string(),
        };

        let response = execute_request(
            &client,
            &mock_server.uri(),
            &HashMap::new(),
            &spec,
            &context,
        )
        .await
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["token"], "abc123");
    }

    #[tokio::test]
    async fn test_execute_request_post_form() {
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let mut context = TemplateContext::new();
        context.set("input.user", "alice");
        context.set("input.pass", "pw123");

        Mock::given(method("POST"))
            .and(path("/api/auth"))
            .and(header("content-type", "application/x-www-form-urlencoded"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .mount(&mock_server)
            .await;

        let spec = RequestSpec {
            method: "POST".to_string(),
            path: "/api/auth".to_string(),
            headers: HashMap::new(),
            params: HashMap::new(),
            body: Some("user={{input.user}}&pass={{input.pass}}".to_string()),
            body_encoding: "form".to_string(),
        };

        let response = execute_request(
            &client,
            &mock_server.uri(),
            &HashMap::new(),
            &spec,
            &context,
        )
        .await
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["ok"], true);
    }

    #[tokio::test]
    async fn test_execute_request_with_headers() {
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let mut context = TemplateContext::new();
        context.set("stored.api_key", "secret123");

        Mock::given(method("GET"))
            .and(path("/api/data"))
            .and(header("X-API-Key", "secret123"))
            .and(header("User-Agent", "TestApp/1.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": "value"})))
            .mount(&mock_server)
            .await;

        let mut default_headers = HashMap::new();
        default_headers.insert("User-Agent".to_string(), "TestApp/1.0".to_string());

        let mut req_headers = HashMap::new();
        req_headers.insert("X-API-Key".to_string(), "{{stored.api_key}}".to_string());

        let spec = RequestSpec {
            method: "GET".to_string(),
            path: "/api/data".to_string(),
            headers: req_headers,
            params: HashMap::new(),
            body: None,
            body_encoding: "json".to_string(),
        };

        let response = execute_request(
            &client,
            &mock_server.uri(),
            &default_headers,
            &spec,
            &context,
        )
        .await
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["data"], "value");
    }

    #[tokio::test]
    async fn test_execute_request_with_query_params() {
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let mut context = TemplateContext::new();
        context.set("stored.limit", "10");
        context.set("stored.offset", "5");

        Mock::given(method("GET"))
            .and(path("/api/items"))
            .and(query_param("limit", "10"))
            .and(query_param("offset", "5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"items": [1, 2, 3]})))
            .mount(&mock_server)
            .await;

        let mut params = HashMap::new();
        params.insert("limit".to_string(), "{{stored.limit}}".to_string());
        params.insert("offset".to_string(), "{{stored.offset}}".to_string());

        let spec = RequestSpec {
            method: "GET".to_string(),
            path: "/api/items".to_string(),
            headers: HashMap::new(),
            params,
            body: None,
            body_encoding: "json".to_string(),
        };

        let response = execute_request(
            &client,
            &mock_server.uri(),
            &HashMap::new(),
            &spec,
            &context,
        )
        .await
        .unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body["items"].is_array());
    }

    #[tokio::test]
    async fn test_execute_request_skips_unresolved_params() {
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let context = TemplateContext::new();

        Mock::given(method("GET"))
            .and(path("/api/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"results": []})))
            .mount(&mock_server)
            .await;

        let mut params = HashMap::new();
        params.insert("q".to_string(), "{{input.query}}".to_string());
        params.insert("valid".to_string(), "ok".to_string());

        let spec = RequestSpec {
            method: "GET".to_string(),
            path: "/api/search".to_string(),
            headers: HashMap::new(),
            params,
            body: None,
            body_encoding: "json".to_string(),
        };

        let response = execute_request(
            &client,
            &mock_server.uri(),
            &HashMap::new(),
            &spec,
            &context,
        )
        .await
        .unwrap();

        assert_eq!(response.status, 200);
    }

    #[tokio::test]
    async fn test_execute_request_put() {
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let context = TemplateContext::new();

        Mock::given(method("PUT"))
            .and(path("/api/update"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"updated": true})))
            .mount(&mock_server)
            .await;

        let spec = RequestSpec {
            method: "PUT".to_string(),
            path: "/api/update".to_string(),
            headers: HashMap::new(),
            params: HashMap::new(),
            body: Some(json!({"field": "value"}).to_string()),
            body_encoding: "json".to_string(),
        };

        let response = execute_request(
            &client,
            &mock_server.uri(),
            &HashMap::new(),
            &spec,
            &context,
        )
        .await
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["updated"], true);
    }

    #[tokio::test]
    async fn test_execute_request_delete() {
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let context = TemplateContext::new();

        Mock::given(method("DELETE"))
            .and(path("/api/delete"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"deleted": true})))
            .mount(&mock_server)
            .await;

        let spec = RequestSpec {
            method: "DELETE".to_string(),
            path: "/api/delete".to_string(),
            headers: HashMap::new(),
            params: HashMap::new(),
            body: None,
            body_encoding: "json".to_string(),
        };

        let response = execute_request(
            &client,
            &mock_server.uri(),
            &HashMap::new(),
            &spec,
            &context,
        )
        .await
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["deleted"], true);
    }

    #[tokio::test]
    async fn test_execute_request_patch() {
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let context = TemplateContext::new();

        Mock::given(method("PATCH"))
            .and(path("/api/patch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"patched": true})))
            .mount(&mock_server)
            .await;

        let spec = RequestSpec {
            method: "PATCH".to_string(),
            path: "/api/patch".to_string(),
            headers: HashMap::new(),
            params: HashMap::new(),
            body: Some(json!({"op": "replace"}).to_string()),
            body_encoding: "json".to_string(),
        };

        let response = execute_request(
            &client,
            &mock_server.uri(),
            &HashMap::new(),
            &spec,
            &context,
        )
        .await
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["patched"], true);
    }

    #[tokio::test]
    async fn test_execute_request_unsupported_method() {
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let context = TemplateContext::new();

        let spec = RequestSpec {
            method: "TRACE".to_string(),
            path: "/api/test".to_string(),
            headers: HashMap::new(),
            params: HashMap::new(),
            body: None,
            body_encoding: "json".to_string(),
        };

        let result = execute_request(
            &client,
            &mock_server.uri(),
            &HashMap::new(),
            &spec,
            &context,
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
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let context = TemplateContext::new();

        Mock::given(method("GET"))
            .and(path("/api/error"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({"error": "not found"})))
            .mount(&mock_server)
            .await;

        let spec = RequestSpec {
            method: "GET".to_string(),
            path: "/api/error".to_string(),
            headers: HashMap::new(),
            params: HashMap::new(),
            body: None,
            body_encoding: "json".to_string(),
        };

        let result = execute_request(
            &client,
            &mock_server.uri(),
            &HashMap::new(),
            &spec,
            &context,
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("404"));
    }

    #[tokio::test]
    async fn test_execute_request_body_convenience() {
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let context = TemplateContext::new();

        Mock::given(method("GET"))
            .and(path("/api/data"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"key": "value"})))
            .mount(&mock_server)
            .await;

        let spec = RequestSpec {
            method: "GET".to_string(),
            path: "/api/data".to_string(),
            headers: HashMap::new(),
            params: HashMap::new(),
            body: None,
            body_encoding: "json".to_string(),
        };

        let body = execute_request_body(
            &client,
            &mock_server.uri(),
            &HashMap::new(),
            &spec,
            &context,
        )
        .await
        .unwrap();

        assert_eq!(body["key"], "value");
    }

    #[tokio::test]
    async fn test_execute_request_template_in_path() {
        let mock_server = MockServer::start().await;
        let client = Client::new();
        let mut context = TemplateContext::new();
        context.set("stored.user_id", "42");

        Mock::given(method("GET"))
            .and(path("/api/users/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 42})))
            .mount(&mock_server)
            .await;

        let spec = RequestSpec {
            method: "GET".to_string(),
            path: "/api/users/{{stored.user_id}}".to_string(),
            headers: HashMap::new(),
            params: HashMap::new(),
            body: None,
            body_encoding: "json".to_string(),
        };

        let response = execute_request(
            &client,
            &mock_server.uri(),
            &HashMap::new(),
            &spec,
            &context,
        )
        .await
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["id"], 42);
    }
}
