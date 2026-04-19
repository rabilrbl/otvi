use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tower_http::services::{ServeDir, ServeFile};
use url::Url;

use otvi_server::auth_middleware;
use otvi_server::db;
use otvi_server::state;
use otvi_server::state::RateLimitConfig;
use otvi_server::watcher;

fn redact_database_url(database_url: &str) -> String {
    match Url::parse(database_url) {
        Ok(mut url) => {
            if !url.username().is_empty() {
                let _ = url.set_username("REDACTED");
            }
            if url.password().is_some() {
                let _ = url.set_password(Some("REDACTED"));
            }
            url.to_string()
        }
        Err(_) => "[invalid DATABASE_URL]".to_string(),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (silently ignored when absent).
    dotenvy::dotenv().ok();

    // ── Structured logging ──────────────────────────────────────────────────
    // Set LOG_FORMAT=json for machine-readable output (e.g. Loki, Datadog).
    // Defaults to human-readable text for local development.
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_else(|_| "text".to_string());

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "otvi_server=info".into());

    match log_format.to_lowercase().as_str() {
        "json" => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(env_filter)
                .with_current_span(false)
                .with_span_list(false)
                .init();
        }
        _ => {
            tracing_subscriber::fmt().with_env_filter(env_filter).init();
        }
    }

    let providers_dir = std::env::var("PROVIDERS_DIR").unwrap_or_else(|_| "providers".to_string());
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "dist".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());

    // ── Database ────────────────────────────────────────────────────────────
    // Register all bundled drivers so AnyPool can inspect the URL scheme.
    sqlx::any::install_default_drivers();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data.db".to_string());
    tracing::info!(
        database_url = %redact_database_url(&database_url),
        "Connecting to database"
    );
    let db = db::init(&database_url).await?;

    // ── JWT secret ──────────────────────────────────────────────────────────
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        tracing::warn!(
            "JWT_SECRET not set – using random secret (tokens valid only until restart)"
        );
        uuid::Uuid::new_v4().to_string()
    });
    let jwt_keys = auth_middleware::JwtKeys::new(jwt_secret.as_bytes());

    // ── Providers ───────────────────────────────────────────────────────────
    let app_state = state::AppState::load_providers(&providers_dir, db, jwt_keys)?;
    let provider_count = app_state.providers_rw.read().map(|g| g.len()).unwrap_or(0);
    tracing::info!("Loaded {provider_count} provider(s)");

    // ── Rate limiting ───────────────────────────────────────────────────────
    let rate_limit = RateLimitConfig::from_env();

    // ── Routes ──────────────────────────────────────────────────────────────
    let state = Arc::new(app_state);

    // ── Hot-reload watcher ──────────────────────────────────────────────────
    // Watches the providers directory for YAML changes and reloads the
    // provider map in-place without restarting the server.
    watcher::spawn(state.clone(), providers_dir.clone());
    tracing::info!(dir = %providers_dir, "Provider hot-reload enabled");

    let app = if otvi_server::has_embedded_frontend() {
        tracing::info!("Serving embedded frontend assets from the otvi-server binary");
        otvi_server::build_router(state, rate_limit).fallback(otvi_server::serve_embedded_frontend)
    } else {
        tracing::info!(dir = %static_dir, "Serving frontend assets from the filesystem");
        otvi_server::build_router(state, rate_limit).fallback_service(
            ServeDir::new(&static_dir)
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new(format!("{static_dir}/index.html"))),
        )
    };

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // ── Graceful shutdown with drain timeout ────────────────────────────────
    // When a shutdown signal arrives the server stops accepting new connections
    // and waits for in-flight requests to complete.  A stalled long-lived
    // connection (e.g. a hung stream proxy) would otherwise block the shutdown
    // indefinitely.  We race the drain against SHUTDOWN_DRAIN_SECS (default 30)
    // so deployments always complete within a bounded time.
    let drain_secs = shutdown_drain_secs();
    let (drain_tx, drain_rx) = tokio::sync::oneshot::channel::<()>();

    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_signal().await;
        // Notify the drain-timeout future that the signal has fired.
        let _ = drain_tx.send(());
    });

    tokio::select! {
        result = server => { result?; }
        _ = async move {
            // Wait until shutdown signal fires, then enforce the drain deadline.
            drain_rx.await.ok();
            tokio::time::sleep(Duration::from_secs(drain_secs)).await;
            tracing::warn!(
                drain_secs,
                "Graceful drain timeout exceeded — forcing shutdown"
            );
        } => {}
    }

    tracing::info!("Server stopped");
    Ok(())
}

/// Returns the maximum number of seconds to wait for in-flight requests to
/// drain after a shutdown signal before forcing exit.
///
/// Read from `SHUTDOWN_DRAIN_SECS` env var (default: 30 s).
fn shutdown_drain_secs() -> u64 {
    match std::env::var("SHUTDOWN_DRAIN_SECS") {
        Ok(val) => val.parse().unwrap_or_else(|_| {
            tracing::warn!(
                val = %val,
                "SHUTDOWN_DRAIN_SECS is not a valid integer — using default 30 s"
            );
            30
        }),
        Err(_) => 30,
    }
}

/// Wait for SIGINT (Ctrl-C) or SIGTERM (Docker/Kubernetes stop).
///
/// On non-Unix platforms only SIGINT is handled.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, draining in-flight requests…");
}

#[cfg(test)]
mod tests {
    use super::redact_database_url;

    #[test]
    fn preserves_normal_urls_without_credentials() {
        let redacted = redact_database_url("sqlite://db.sqlite");
        assert_eq!(redacted, "sqlite://db.sqlite");
    }

    #[test]
    fn hides_invalid_url_instead_of_echoing_it_back() {
        let redacted = redact_database_url("postgres://alice@[invalid host");
        assert_eq!(redacted, "[invalid DATABASE_URL]");
    }
}
