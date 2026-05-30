use std::collections::HashSet;

use url::Url;

/// Rewrite URLs in an m3u8 playlist so they are proxied through `/api/proxy`.
///
/// Handles:
/// - Absolute URLs (`https://…`)
/// - Relative paths (`segment001.ts`, `../fallback/…`)
/// - URI attributes in EXT tags (`URI="…"`)
///
/// `ctx_token` is the opaque proxy-context token that should be appended to
/// every rewritten proxy URL so that segment/key requests use the same
/// server-side header set as the initial manifest request.
///
/// `manifest_query` is the raw query string from the original manifest URL
/// (e.g. `minrate=80000&__hdnea__=st%3D…`).  It is appended to key file
/// URLs before proxying so that the upstream CDN receives the auth token as
/// a URL param.
///
/// `key_uri_patterns` controls which URIs within `EXT-X-KEY` lines receive
/// the `manifest_query` append.  An empty slice means «apply to all»;
/// otherwise a URI must contain at least one pattern (case-insensitive).
pub(crate) struct RewriteResult {
    pub(crate) content: String,
    pub(crate) discovered_hosts: Vec<String>,
}

#[cfg(test)]
impl RewriteResult {
    pub(crate) fn contains(&self, needle: &str) -> bool {
        self.content.contains(needle)
    }
}

pub(crate) fn rewrite_m3u8(
    content: &str,
    playlist_url: &str,
    ctx_token: Option<&str>,
    manifest_query: Option<&str>,
    key_uri_patterns: &[String],
) -> RewriteResult {
    let base = Url::parse(playlist_url).unwrap_or_else(|_| Url::parse("http://unknown").unwrap());

    let mut output = String::with_capacity(content.len());
    let mut discovered_hosts = HashSet::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            output.push('\n');
            continue;
        }

        if trimmed.starts_with('#') {
            // Rewrite URI="…" attributes in EXT tags (e.g. EXT-X-KEY, EXT-X-MAP).
            // Pass manifest_query only for EXT-X-KEY lines (HLS encryption key
            // declarations) — controlled by the caller via `manifest_query`.
            let is_key_tag = trimmed.to_uppercase().starts_with("#EXT-X-KEY");
            let extra = if is_key_tag { manifest_query } else { None };
            let rewritten_line = rewrite_uri_attributes(
                trimmed,
                &base,
                ctx_token,
                extra,
                key_uri_patterns,
                &mut discovered_hosts,
            );
            output.push_str(&rewritten_line);
            output.push('\n');
        } else {
            // This is a URL line (segment or sub-playlist)
            let resolved =
                resolve_and_proxy(trimmed, &base, ctx_token, None, &mut discovered_hosts);
            output.push_str(&resolved);
            output.push('\n');
        }
    }

    RewriteResult {
        content: output,
        discovered_hosts: discovered_hosts.into_iter().collect(),
    }
}

/// Resolve a URL (potentially relative) against the playlist base and wrap it
/// in the proxy endpoint.  `ctx_token` is forwarded as-is if present.
///
/// `extra_query` is an optional raw query string to append to the resolved URL
/// before percent-encoding it.  Used for key file URLs to carry the manifest's
/// Akamai token so the upstream CDN authorises the request.
pub(crate) fn resolve_and_proxy(
    url_str: &str,
    base: &Url,
    ctx_token: Option<&str>,
    extra_query: Option<&str>,
    discovered_hosts: &mut HashSet<String>,
) -> String {
    let absolute = if url_str.starts_with("http://") || url_str.starts_with("https://") {
        url_str.to_string()
    } else {
        base.join(url_str)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| url_str.to_string())
    };

    let with_query = match extra_query {
        Some(q) if !q.is_empty() => {
            if absolute.contains('?') {
                format!("{absolute}&{q}")
            } else {
                format!("{absolute}?{q}")
            }
        }
        _ => absolute,
    };

    if let Ok(parsed) = Url::parse(&with_query)
        && let Some(host) = parsed.host_str()
    {
        discovered_hosts.insert(host.to_string());
    }

    match ctx_token {
        Some(token) => format!(
            "/api/proxy?url={}&ctx={}",
            urlencoding::encode(&with_query),
            token,
        ),
        None => format!("/api/proxy?url={}", urlencoding::encode(&with_query)),
    }
}

/// Rewrite `URI="…"` attributes inside EXT-X tags.
///
/// When `manifest_query` is `Some`, it is appended to the target URI so the
/// upstream CDN receives the session token as a URL param.  Whether to append
/// is further gated by `key_uri_patterns`: if the slice is non-empty, the URI
/// must contain at least one pattern (case-insensitive) for the append to
/// occur; an empty slice means «always append» (the caller is already
/// responsible for only passing `manifest_query` for `EXT-X-KEY` lines).
pub(crate) fn rewrite_uri_attributes(
    line: &str,
    base: &Url,
    ctx_token: Option<&str>,
    manifest_query: Option<&str>,
    key_uri_patterns: &[String],
    discovered_hosts: &mut HashSet<String>,
) -> String {
    // Find URI="…" pattern (case-insensitive)
    let mut result = line.to_string();

    // Handle URI="..." pattern
    if let Some(uri_start) = result.to_uppercase().find("URI=\"") {
        let actual_start = uri_start + 5; // skip past URI="
        if let Some(uri_end) = result[actual_start..].find('"') {
            let uri_val = &result[actual_start..actual_start + uri_end].to_string();
            // Append manifest query params to key file URLs so the upstream CDN
            // receives the auth token in the URL.  Which URIs qualify is
            // controlled by the provider-configured `key_uri_patterns`; an
            // empty list means «apply to all URIs in EXT-X-KEY lines».
            let lower = uri_val.to_lowercase();
            let is_key = key_uri_patterns.is_empty()
                || key_uri_patterns
                    .iter()
                    .any(|p| lower.contains(p.to_lowercase().as_str()));
            let extra = if is_key { manifest_query } else { None };
            let proxied = resolve_and_proxy(uri_val, base, ctx_token, extra, discovered_hosts);
            result = format!(
                "{}URI=\"{}\"{}",
                &line[..uri_start],
                proxied,
                &line[actual_start + uri_end + 1..]
            );
        }
    }

    result
}

// ── DASH MPD rewriting ────────────────────────────────────────────────────

pub(crate) struct MpdRewriteResult {
    pub(crate) content: String,
    pub(crate) discovered_hosts: Vec<String>,
}

/// Rewrite `<BaseURL>` elements in a DASH MPD manifest so that segment URLs
/// route back through the proxy.
///
/// Uses string replacement rather than full XML DOM parsing — this mirrors
/// JioTV-Go's approach and avoids pulling in an XML crate.  Each
/// `<BaseURL>…</BaseURL>` is resolved against the manifest URL and wrapped in
/// the proxy endpoint.
///
/// Returns `Err` only on unrecoverable errors (none currently — best-effort).
pub(crate) fn rewrite_mpd(
    content: &str,
    manifest_url: &str,
    ctx_token: Option<&str>,
) -> Result<MpdRewriteResult, String> {
    let base = Url::parse(manifest_url).map_err(|e| format!("invalid manifest URL: {e}"))?;

    let mut output = String::with_capacity(content.len());
    let mut discovered_hosts = HashSet::new();
    let mut remaining = content;

    // Rewrite all <BaseURL>…</BaseURL> occurrences.
    while let Some(start_idx) = remaining.find("<BaseURL>") {
        let tag_content_start = start_idx + "<BaseURL>".len();
        if let Some(end_idx) = remaining[tag_content_start..].find("</BaseURL>") {
            let url_val = &remaining[tag_content_start..tag_content_start + end_idx];

            // Resolve relative URLs against the manifest base.
            let absolute = if url_val.starts_with("http://") || url_val.starts_with("https://") {
                url_val.to_string()
            } else {
                base.join(url_val)
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| url_val.to_string())
            };

            if let Ok(parsed) = Url::parse(&absolute)
                && let Some(host) = parsed.host_str()
            {
                discovered_hosts.insert(host.to_string());
            }

            let proxied = match ctx_token {
                Some(token) => format!(
                    "/api/proxy?url={}&ctx={}",
                    urlencoding::encode(&absolute),
                    token,
                ),
                None => format!("/api/proxy?url={}", urlencoding::encode(&absolute)),
            };

            output.push_str(&remaining[..start_idx]);
            output.push_str("<BaseURL>");
            output.push_str(&proxied);
            output.push_str("</BaseURL>");
            remaining = &remaining[tag_content_start + end_idx + "</BaseURL>".len()..];
        } else {
            // Malformed: <BaseURL> without closing tag — emit as-is.
            output.push_str(&remaining[..tag_content_start]);
            remaining = &remaining[tag_content_start..];
        }
    }

    // Append any trailing content after the last <BaseURL>.
    output.push_str(remaining);

    Ok(MpdRewriteResult {
        content: output,
        discovered_hosts: discovered_hosts.into_iter().collect(),
    })
}
