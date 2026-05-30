use std::sync::LazyLock;

use prometheus::{
    CounterVec, HistogramVec, IntGauge, IntGaugeVec, Opts, Registry, TextEncoder, histogram_opts,
};

static REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::new);

macro_rules! metric {
    ($typ:ty, $opts:expr) => {{
        let m: $typ = <$typ>::with_opts($opts).unwrap();
        REGISTRY.register(Box::new(m.clone())).unwrap();
        m
    }};
}

// ── HTTP ──────────────────────────────────────────────────────────────────
pub static HTTP_REQUESTS_TOTAL: LazyLock<CounterVec> = LazyLock::new(|| {
    let opts = Opts::new("otvi_http_requests_total", "Total HTTP requests");
    let m = CounterVec::new(opts, &["method", "path", "status"]).unwrap();
    REGISTRY.register(Box::new(m.clone())).unwrap();
    m
});

pub static HTTP_REQUEST_DURATION_SECONDS: LazyLock<HistogramVec> = LazyLock::new(|| {
    let opts = histogram_opts!(
        "otvi_http_request_duration_seconds",
        "Request latency",
        vec![
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0
        ]
    );
    let m = HistogramVec::new(opts, &["method", "path"]).unwrap();
    REGISTRY.register(Box::new(m.clone())).unwrap();
    m
});

// ── Proxy ────────────────────────────────────────────────────────────────
pub static PROXY_CONTEXTS_ACTIVE: LazyLock<IntGauge> = LazyLock::new(|| {
    metric!(
        IntGauge,
        Opts::new(
            "otvi_proxy_contexts_active",
            "Current proxy context cache size"
        )
    )
});

// ── Channel cache ───────────────────────────────────────────────────────
pub static CHANNEL_CACHE_ENTRIES: LazyLock<IntGaugeVec> = LazyLock::new(|| {
    let opts = Opts::new("otvi_channel_cache_entries", "Cached channel list entries");
    let m = IntGaugeVec::new(opts, &["provider", "scope"]).unwrap();
    REGISTRY.register(Box::new(m.clone())).unwrap();
    m
});

pub static CHANNEL_CACHE_HITS_TOTAL: LazyLock<CounterVec> = LazyLock::new(|| {
    let opts = Opts::new("otvi_channel_cache_hits_total", "Cache hits");
    let m = CounterVec::new(opts, &["provider", "scope"]).unwrap();
    REGISTRY.register(Box::new(m.clone())).unwrap();
    m
});

pub static CHANNEL_CACHE_MISSES_TOTAL: LazyLock<CounterVec> = LazyLock::new(|| {
    let opts = Opts::new("otvi_channel_cache_misses_total", "Cache misses");
    let m = CounterVec::new(opts, &["provider", "scope"]).unwrap();
    REGISTRY.register(Box::new(m.clone())).unwrap();
    m
});

// ── Upstream ─────────────────────────────────────────────────────────────
pub static UPSTREAM_REQUESTS_TOTAL: LazyLock<CounterVec> = LazyLock::new(|| {
    let opts = Opts::new(
        "otvi_provider_upstream_requests_total",
        "Upstream API calls",
    );
    let m = CounterVec::new(opts, &["provider", "method", "status"]).unwrap();
    REGISTRY.register(Box::new(m.clone())).unwrap();
    m
});

// ── Refresh locks ────────────────────────────────────────────────────────
pub static REFRESH_LOCKS_ACTIVE: LazyLock<IntGauge> = LazyLock::new(|| {
    metric!(
        IntGauge,
        Opts::new("otvi_refresh_locks_active", "Current refresh lock entries")
    )
});

/// Ensure all metrics are registered. Call once at startup.
pub fn register() {
    let _ = &*HTTP_REQUESTS_TOTAL;
    let _ = &*HTTP_REQUEST_DURATION_SECONDS;
    let _ = &*PROXY_CONTEXTS_ACTIVE;
    let _ = &*CHANNEL_CACHE_ENTRIES;
    let _ = &*CHANNEL_CACHE_HITS_TOTAL;
    let _ = &*CHANNEL_CACHE_MISSES_TOTAL;
    let _ = &*UPSTREAM_REQUESTS_TOTAL;
    let _ = &*REFRESH_LOCKS_ACTIVE;
}

/// Render all metrics in Prometheus text format.
pub fn render() -> String {
    TextEncoder::new()
        .encode_to_string(&REGISTRY.gather())
        .unwrap()
}
