use std::{
    collections::HashMap,
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, anyhow};
use axum::{
    Form, Router,
    extract::{Path as AxumPath, State},
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use handlebars::Handlebars;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
struct ProviderConfig {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    default_headers: HashMap<String, String>,
    login: ActionSpec,
    logout: Option<ActionSpec>,
    browse_channels: BrowseChannelsSpec,
    play_channel: Option<PlayChannelSpec>,
}

#[derive(Debug, Clone, Deserialize)]
struct BrowseChannelsSpec {
    action: ActionSpec,
    response_mapping: ChannelResponseMapping,
}

#[derive(Debug, Clone, Deserialize)]
struct PlayChannelSpec {
    action: ActionSpec,
    response_mapping: Option<PlayResponseMapping>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlayResponseMapping {
    stream_url_path: String,
    drm_license_url_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ChannelResponseMapping {
    items_path: String,
    id_path: String,
    name_path: String,
    stream_url_path: String,
    drm_license_url_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ActionSpec {
    name: String,
    method: String,
    url: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    query: HashMap<String, String>,
    body: Option<String>,
    #[serde(default)]
    input_fields: Vec<InputField>,
    #[serde(default)]
    auth_token_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct InputField {
    key: String,
    label: String,
    #[serde(default = "default_input_type")]
    kind: String,
    #[serde(default)]
    placeholder: String,
}

fn default_input_type() -> String {
    "text".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Channel {
    id: String,
    name: String,
    stream_url: String,
    drm_license_url: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct SessionState {
    auth_token: Option<String>,
}

#[derive(Clone)]
struct AppState {
    providers: Arc<HashMap<String, ProviderConfig>>,
    sessions: Arc<RwLock<HashMap<String, SessionState>>>,
    client: reqwest::Client,
    handlebars: Arc<Handlebars<'static>>,
}

#[derive(Deserialize)]
struct LoginForm {
    session_id: String,
    #[serde(flatten)]
    fields: HashMap<String, String>,
}

#[derive(Deserialize)]
struct ProviderActionForm {
    session_id: String,
}

#[derive(Deserialize)]
struct PlayForm {
    session_id: String,
    channel_id: String,
    stream_url: String,
    drm_license_url: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config_dir = std::env::var("OTVI_CONFIG_DIR").unwrap_or_else(|_| "./providers".to_string());
    let providers = load_providers(Path::new(&config_dir))?;

    if providers.is_empty() {
        return Err(anyhow!(
            "No provider yaml files found in {}. Add at least one provider file.",
            config_dir
        ));
    }

    info!("Loaded {} provider config(s)", providers.len());

    let state = AppState {
        providers: Arc::new(providers),
        sessions: Arc::new(RwLock::new(HashMap::new())),
        client: reqwest::Client::builder().build()?,
        handlebars: Arc::new(Handlebars::new()),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/provider/{provider_id}", get(provider_page))
        .route("/provider/{provider_id}/login", post(login))
        .route("/provider/{provider_id}/logout", post(logout))
        .route("/provider/{provider_id}/browse", post(browse_channels))
        .route("/provider/{provider_id}/play", post(play_channel))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = "0.0.0.0:3000".parse()?;
    info!("Starting server at http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn load_providers(config_dir: &Path) -> anyhow::Result<HashMap<String, ProviderConfig>> {
    let mut providers = HashMap::new();
    if !config_dir.exists() {
        fs::create_dir_all(config_dir).with_context(|| {
            format!(
                "Failed to create provider config directory {}",
                config_dir.display()
            )
        })?;
    }

    for entry in fs::read_dir(config_dir)? {
        let path: PathBuf = entry?.path();
        if !path.is_file() {
            continue;
        }

        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };

        if ext != "yaml" && ext != "yml" {
            continue;
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read provider file {}", path.display()))?;
        let provider: ProviderConfig = serde_yaml::from_str(&raw)
            .with_context(|| format!("Invalid provider yaml in {}", path.display()))?;
        providers.insert(provider.id.clone(), provider);
    }

    Ok(providers)
}

async fn index(State(state): State<AppState>) -> impl IntoResponse {
    let mut providers = state.providers.values().collect::<Vec<_>>();
    providers.sort_by(|a, b| a.name.cmp(&b.name));

    let list = providers
        .iter()
        .map(|p| {
            format!(
                r#"<li><a href=\"/provider/{id}\">{name}</a> - {description}</li>"#,
                id = p.id,
                name = p.name,
                description = p.description
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Html(format!(
        r#"<!doctype html>
<html>
  <head><meta charset=\"utf-8\" /><title>OTVI Providers</title></head>
  <body>
    <h1>Open TV Interface</h1>
    <p>Providers loaded from YAML. Pick one to login, browse channels, and play streams.</p>
    <ul>{}</ul>
  </body>
</html>"#,
        list
    ))
}

async fn provider_page(
    AxumPath(provider_id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::not_found("Unknown provider"))?;

    let session_id = Uuid::new_v4().to_string();
    state
        .sessions
        .write()
        .await
        .insert(session_id.clone(), SessionState::default());

    let login_fields = provider
        .login
        .input_fields
        .iter()
        .map(|f| {
            format!(
                r#"<label>{label}<br/><input type=\"{kind}\" name=\"{key}\" placeholder=\"{placeholder}\" /></label><br/>"#,
                label = f.label,
                kind = f.kind,
                key = f.key,
                placeholder = f.placeholder
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let html = format!(
        r#"<!doctype html>
<html>
  <head><meta charset=\"utf-8\" /><title>{name}</title></head>
  <body>
    <a href=\"/\">← providers</a>
    <h1>{name}</h1>
    <p>{description}</p>

    <h2>Login</h2>
    <form method=\"post\" action=\"/provider/{id}/login\">
      <input type=\"hidden\" name=\"provider_id\" value=\"{id}\"/>
      <input type=\"hidden\" name=\"session_id\" value=\"{session}\"/>
      {fields}
      <button type=\"submit\">Login</button>
    </form>

    <h2>Browse channels</h2>
    <form method=\"post\" action=\"/provider/{id}/browse\">
      <input type=\"hidden\" name=\"session_id\" value=\"{session}\"/>
      <button type=\"submit\">Browse</button>
    </form>

    <h2>Logout</h2>
    <form method=\"post\" action=\"/provider/{id}/logout\">
      <input type=\"hidden\" name=\"session_id\" value=\"{session}\"/>
      <button type=\"submit\">Logout</button>
    </form>
  </body>
</html>"#,
        name = provider.name,
        description = provider.description,
        id = provider.id,
        session = session_id,
        fields = login_fields
    );

    Ok(Html(html))
}

async fn login(
    AxumPath(provider_id): AxumPath<String>,
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Result<Html<String>, AppError> {
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::not_found("Unknown provider"))?;

    let response = execute_action(
        &state,
        provider,
        &provider.login,
        &form.session_id,
        form.fields,
    )
    .await?;

    if let Some(token_path) = &provider.login.auth_token_path {
        let token = extract_path(&response, token_path)
            .and_then(|v| v.as_str().map(ToString::to_string))
            .ok_or_else(|| AppError::bad_request("auth_token_path did not resolve to a string"))?;

        state
            .sessions
            .write()
            .await
            .entry(form.session_id.clone())
            .and_modify(|s| s.auth_token = Some(token.clone()))
            .or_insert(SessionState {
                auth_token: Some(token.clone()),
            });
    }

    Ok(Html(format!(
        "<p>Login successful for provider {}</p><p><a href=\"/provider/{}\">Back</a></p>",
        provider.name, provider.id
    )))
}

async fn logout(
    AxumPath(provider_id): AxumPath<String>,
    State(state): State<AppState>,
    Form(form): Form<ProviderActionForm>,
) -> Result<Html<String>, AppError> {
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::not_found("Unknown provider"))?;

    if let Some(logout) = &provider.logout {
        let _ = execute_action(&state, provider, logout, &form.session_id, HashMap::new()).await?;
    }

    if let Some(session) = state.sessions.write().await.get_mut(&form.session_id) {
        session.auth_token = None;
    }

    Ok(Html(format!(
        "<p>Logged out from provider {}</p><p><a href=\"/provider/{}\">Back</a></p>",
        provider.name, provider.id
    )))
}

async fn browse_channels(
    AxumPath(provider_id): AxumPath<String>,
    State(state): State<AppState>,
    Form(form): Form<ProviderActionForm>,
) -> Result<Html<String>, AppError> {
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::not_found("Unknown provider"))?;

    let payload = execute_action(
        &state,
        provider,
        &provider.browse_channels.action,
        &form.session_id,
        HashMap::new(),
    )
    .await?;

    let channels = map_channels(&payload, &provider.browse_channels.response_mapping)?;
    let channels_html = channels
        .iter()
        .map(|c| {
            format!(
                r#"<li><strong>{name}</strong><br/>HLS/DASH: {stream}<form method=\"post\" action=\"/provider/{id}/play\"><input type=\"hidden\" name=\"session_id\" value=\"{session}\"/><input type=\"hidden\" name=\"channel_id\" value=\"{channel_id}\"/><input type=\"hidden\" name=\"stream_url\" value=\"{stream}\"/><input type=\"hidden\" name=\"drm_license_url\" value=\"{drm}\"/><button type=\"submit\">Play</button></form></li>"#,
                name = c.name,
                stream = c.stream_url,
                id = provider.id,
                session = form.session_id,
                channel_id = c.id,
                drm = c.drm_license_url.clone().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(Html(format!(
        "<h1>{}</h1><h2>Channels</h2><ul>{}</ul><p><a href=\"/provider/{}\">Back</a></p>",
        provider.name, channels_html, provider.id
    )))
}

async fn play_channel(
    AxumPath(provider_id): AxumPath<String>,
    State(state): State<AppState>,
    Form(form): Form<PlayForm>,
) -> Result<Html<String>, AppError> {
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::not_found("Unknown provider"))?;

    let mut stream_url = form.stream_url.clone();
    let mut drm_url = form.drm_license_url.clone();

    if let Some(play) = &provider.play_channel {
        let mut fields = HashMap::new();
        fields.insert("channel_id".to_string(), form.channel_id.clone());
        let payload =
            execute_action(&state, provider, &play.action, &form.session_id, fields).await?;
        if let Some(map) = &play.response_mapping {
            if let Some(found) =
                extract_path(&payload, &map.stream_url_path).and_then(|v| v.as_str())
            {
                stream_url = found.to_string();
            }
            drm_url = map
                .drm_license_url_path
                .as_ref()
                .and_then(|p| extract_path(&payload, p))
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .or(drm_url);
        }
    }

    Ok(Html(format!(
        r#"<h1>Playing {channel}</h1>
<video controls autoplay width=\"800\" src=\"{stream}\"></video>
<p>Stream URL: <code>{stream}</code></p>
<p>DRM license URL: <code>{drm}</code></p>
<p>Note: Browser DRM playback depends on provider, CORS, and EME support.</p>
<p><a href=\"/provider/{provider}\">Back</a></p>"#,
        channel = form.channel_id,
        stream = stream_url,
        drm = drm_url.unwrap_or_else(|| "N/A".to_string()),
        provider = provider.id
    )))
}

async fn execute_action(
    state: &AppState,
    provider: &ProviderConfig,
    action: &ActionSpec,
    session_id: &str,
    extra_fields: HashMap<String, String>,
) -> Result<Value, AppError> {
    let mut context_data = Map::new();

    for (k, v) in extra_fields {
        context_data.insert(k, Value::String(v));
    }

    if let Some(token) = state
        .sessions
        .read()
        .await
        .get(session_id)
        .and_then(|s| s.auth_token.clone())
    {
        context_data.insert("auth_token".to_string(), Value::String(token));
    }

    context_data.insert(
        "session_id".to_string(),
        Value::String(session_id.to_string()),
    );

    let ctx = Value::Object(context_data);
    let method = action
        .method
        .parse::<Method>()
        .map_err(|_| AppError::bad_request("invalid HTTP method in YAML config"))?;

    let url = render_template(&state.handlebars, &action.url, &ctx)?;
    let mut request = state.client.request(method, url);

    for (key, val) in &provider.default_headers {
        request = request.header(key, render_template(&state.handlebars, val, &ctx)?);
    }
    for (key, val) in &action.headers {
        request = request.header(key, render_template(&state.handlebars, val, &ctx)?);
    }

    if !action.query.is_empty() {
        let mut query = HashMap::new();
        for (k, v) in &action.query {
            query.insert(k.to_string(), render_template(&state.handlebars, v, &ctx)?);
        }
        request = request.query(&query);
    }

    if let Some(body_template) = &action.body {
        let body = render_template(&state.handlebars, body_template, &ctx)?;
        request = request
            .header("content-type", "application/json")
            .body(body);
    }

    let response = request
        .send()
        .await
        .map_err(|e| AppError::upstream(format!("{} request failed: {e}", action.name)))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| AppError::upstream(format!("{} response decode failed: {e}", action.name)))?;

    if !status.is_success() {
        return Err(AppError::upstream(format!(
            "{} failed with status {} and body {}",
            action.name, status, text
        )));
    }

    serde_json::from_str(&text)
        .map_err(|e| AppError::upstream(format!("{} did not return JSON: {e}", action.name)))
}

fn map_channels(
    payload: &Value,
    mapping: &ChannelResponseMapping,
) -> Result<Vec<Channel>, AppError> {
    let Some(items) = extract_path(payload, &mapping.items_path).and_then(|v| v.as_array()) else {
        return Err(AppError::bad_request(
            "browse_channels.response_mapping.items_path is invalid",
        ));
    };

    let channels = items
        .iter()
        .filter_map(|item| {
            let id = extract_path(item, &mapping.id_path)?.as_str()?.to_string();
            let name = extract_path(item, &mapping.name_path)?
                .as_str()?
                .to_string();
            let stream_url = extract_path(item, &mapping.stream_url_path)?
                .as_str()?
                .to_string();
            let drm_license_url = mapping
                .drm_license_url_path
                .as_ref()
                .and_then(|p| extract_path(item, p))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            Some(Channel {
                id,
                name,
                stream_url,
                drm_license_url,
            })
        })
        .collect::<Vec<_>>();

    Ok(channels)
}

fn extract_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for piece in path.trim_start_matches('/').split('/') {
        if piece.is_empty() {
            continue;
        }
        current = current.get(piece)?;
    }
    Some(current)
}

fn render_template(reg: &Handlebars<'_>, template: &str, ctx: &Value) -> Result<String, AppError> {
    reg.render_template(template, ctx)
        .map_err(|e| AppError::bad_request(format!("template render failed: {e}")))
}

#[derive(Debug)]
struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn upstream(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: message.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        error!("{}", self.message);
        (
            self.status,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            )],
            format!("<h1>Error</h1><p>{}</p>", self.message),
        )
            .into_response()
    }
}
