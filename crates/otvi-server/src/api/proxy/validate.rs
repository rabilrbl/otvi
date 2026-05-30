use axum::http::StatusCode;
use url::Url;

/// Returns `true` when `host` is a literal IP address (or the bare string
/// `"localhost"`) that falls into a range that must never be reachable via the
/// proxy.
///
/// **Note**: only *literal* IP addresses are inspected.  Hostnames that are
/// not numeric IPs are treated as public (DNS resolution is not performed here).
/// A DNS-rebinding attack using a hostname that resolves to a private IP is
/// therefore not mitigated by this function alone — callers should also enforce
/// an explicit `allowed_hosts` list populated from the provider YAML.
///
/// Blocked ranges:
/// - `0.0.0.0`       — INADDR_ANY (routes to loopback on Linux)
/// - `127.0.0.0/8`   — IPv4 loopback
/// - `10.0.0.0/8`    — RFC-1918 private
/// - `172.16.0.0/12` — RFC-1918 private
/// - `192.168.0.0/16`— RFC-1918 private
/// - `169.254.0.0/16`— link-local / AWS instance metadata
/// - `::`            — IPv6 unspecified (routes to `::1` on Linux)
/// - `::1`           — IPv6 loopback
/// - `fe80::/10`     — IPv6 link-local
/// - `"localhost"`   — literal hostname (case-insensitive)
pub(crate) fn is_private_host(host: &str) -> bool {
    use std::net::IpAddr;

    // Bare "localhost" hostname check.
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }

    // Strip IPv6 brackets: "[::1]" → "::1"
    let stripped = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);

    match stripped.parse() {
        // Use stable Ipv4Addr predicates — each covers one blocked range.
        Ok(IpAddr::V4(v4)) => {
            v4.is_unspecified() // 0.0.0.0 (INADDR_ANY)
                || v4.is_loopback()   // 127.0.0.0/8
                || v4.is_private()    // 10/8, 172.16/12, 192.168/16
                || v4.is_link_local() // 169.254.0.0/16
        }
        Ok(IpAddr::V6(v6)) => {
            v6.is_unspecified() // :: (routes to ::1 on Linux)
                || v6.is_loopback()  // ::1
                || (v6.segments()[0] & 0xFFC0) == 0xFE80 // fe80::/10
        }
        // Not a bare IP — treat as public hostname (e.g. "cdn.example.com").
        Err(_) => false,
    }
}

pub(crate) fn validate_proxy_target(
    ctx: &crate::state::ProxyContext,
    parsed: &Url,
    allow_private_hosts: bool,
) -> Result<(), (StatusCode, String)> {
    let Some(host) = parsed.host_str() else {
        return Err((
            StatusCode::BAD_REQUEST,
            "Proxy URL must include a host".to_string(),
        ));
    };

    // Block SSRF to loopback / private / link-local ranges.
    // Skipped in test mode where httpbin runs on localhost.
    if !allow_private_hosts && is_private_host(host) {
        return Err((
            StatusCode::FORBIDDEN,
            "Proxy target is not allowed".to_string(),
        ));
    }

    // An empty allowed_hosts list means the context was never populated — deny
    // rather than allow everything (fail-closed).
    if ctx.allowed_hosts.is_empty() {
        return Err((
            StatusCode::FORBIDDEN,
            "Proxy target is not allowed for this playback context".to_string(),
        ));
    }

    if !ctx.allowed_hosts.iter().any(|allowed| allowed == host) {
        return Err((
            StatusCode::FORBIDDEN,
            "Proxy target is not allowed for this playback context".to_string(),
        ));
    }

    Ok(())
}

pub(crate) fn merge_allowed_host(hosts: &mut Vec<String>, host: Option<&str>) {
    if let Some(host) = host
        && !hosts.iter().any(|existing| existing == host)
    {
        hosts.push(host.to_string());
    }
}

pub(crate) fn merge_allowed_hosts(hosts: &mut Vec<String>, discovered: &[String]) {
    for host in discovered {
        if !hosts.iter().any(|existing| existing == host) {
            hosts.push(host.clone());
        }
    }
}
