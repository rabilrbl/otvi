use std::sync::Arc;

use otvi_core::types::{Category, CategoryListResponse, Channel, ChannelListResponse};
use tracing::{debug, error};

use crate::api::auth::with_refresh_retry;
use crate::api::channels::{load_all_channels, map_categories};
use crate::api::provider_access::authorize_provider_route;
use crate::auth_middleware::ActiveClaims;
use crate::error::{AppError, InternalSource};
use crate::provider_client;
use crate::state::{AppState, CacheScope, CachedCategories, ChannelCacheKey};

#[derive(Debug, Clone, Default)]
pub struct ChannelQuery {
    pub category: Option<String>,
    pub search: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Load the provider channel catalog and apply the backend query contract.
pub async fn list_channels(
    state: &Arc<AppState>,
    claims: &ActiveClaims,
    provider_id: &str,
    query: ChannelQuery,
) -> Result<ChannelListResponse, AppError> {
    let scope = authorize_provider_route(state, claims, provider_id, false).await?;

    let provider_data = state
        .with_provider(provider_id, |p| {
            (
                p.defaults.base_url.clone(),
                p.defaults.headers.clone(),
                p.channels.list.request.clone(),
                p.channels.list.response.clone(),
            )
        })
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let (base_url, default_headers, list_request, list_response) = provider_data;
    let cache_key = ChannelCacheKey::from_auth_scope(provider_id, &scope, &claims.sub);
    let uid = session_uid(&cache_key);

    let mut extra_ctx: Vec<(&str, &str)> = Vec::new();
    if let Some(cat) = &query.category {
        extra_ctx.push(("input.category", cat));
    }
    if let Some(search) = &query.search {
        extra_ctx.push(("input.search", search));
    }

    let all_channels = load_all_channels(
        state,
        provider_id,
        &uid,
        &cache_key,
        &base_url,
        &default_headers,
        &list_request,
        &list_response,
        &extra_ctx,
    )
    .await?;

    Ok(query_channels(&all_channels, &query))
}

/// Load channel categories for a provider, using static categories or cache as appropriate.
pub async fn list_categories(
    state: &Arc<AppState>,
    claims: &ActiveClaims,
    provider_id: &str,
) -> Result<CategoryListResponse, AppError> {
    let scope = authorize_provider_route(state, claims, provider_id, false).await?;

    let provider_data = state
        .with_provider(provider_id, |p| {
            (
                p.defaults.base_url.clone(),
                p.defaults.headers.clone(),
                p.channels.static_categories.clone(),
                p.channels.categories.clone(),
            )
        })
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let (base_url, default_headers, static_categories, dynamic_endpoint) = provider_data;

    if !static_categories.is_empty() {
        let categories = static_categories
            .iter()
            .map(|category| Category {
                id: category.id.clone(),
                name: category.name.clone(),
            })
            .collect();
        return Ok(CategoryListResponse { categories });
    }

    let cat_endpoint =
        dynamic_endpoint.ok_or_else(|| AppError::NotFound("Categories not configured".into()))?;

    let cache_key = ChannelCacheKey::from_auth_scope(provider_id, &scope, &claims.sub);
    let uid = session_uid(&cache_key);

    let categories = if let Some(cached) = state.channel_cache.categories.get(&cache_key).await {
        debug!(provider = %provider_id, "categories cache HIT");
        cached.categories
    } else {
        debug!(provider = %provider_id, "categories cache MISS - fetching from upstream");

        let base_url = base_url.clone();
        let default_headers = default_headers.clone();
        let cat_request = cat_endpoint.request.clone();
        let http_client = state.http_client.clone();

        let resp = with_refresh_retry(state, provider_id, &uid, |ctx| {
            let http_client = http_client.clone();
            let base_url = base_url.clone();
            let default_headers = default_headers.clone();
            let cat_request = cat_request.clone();
            async move {
                provider_client::execute_request_raw(
                    &http_client,
                    &base_url,
                    &default_headers,
                    &cat_request,
                    &ctx,
                )
                .await
            }
        })
        .await?;

        if !(200..300).contains(&resp.status) {
            error!(
                provider = %provider_id,
                status = resp.status,
                "Upstream categories error after refresh retry"
            );
            return Err(AppError::Internal(InternalSource(format!(
                "Upstream categories returned status {}",
                resp.status
            ))));
        }

        let mapped = Arc::<[Category]>::from(map_categories(&resp.body, &cat_endpoint.response)?);
        state
            .channel_cache
            .categories
            .insert(
                cache_key,
                CachedCategories {
                    categories: mapped.clone(),
                },
            )
            .await;
        mapped
    };

    Ok(CategoryListResponse {
        categories: categories.iter().cloned().collect(),
    })
}

fn session_uid(cache_key: &ChannelCacheKey) -> String {
    match &cache_key.scope {
        CacheScope::Global => String::new(),
        CacheScope::PerUser(uid) => uid.clone(),
    }
}

fn query_channels(channels: &[Channel], query: &ChannelQuery) -> ChannelListResponse {
    let mut filtered = channels.iter().collect::<Vec<_>>();

    if let Some(category) = &query.category
        && !category.is_empty()
    {
        filtered.retain(|channel| channel.category.as_deref() == Some(category.as_str()));
    }

    if let Some(term) = &query.search
        && !term.is_empty()
    {
        let term = term.to_lowercase();
        filtered.retain(|channel| channel.name.to_lowercase().contains(&term));
    }

    let total = filtered.len();
    let offset = query.offset.unwrap_or(0);
    let channels = if let Some(limit) = query.limit {
        filtered
            .into_iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect()
    } else if offset > 0 {
        filtered.into_iter().skip(offset).cloned().collect()
    } else {
        filtered.into_iter().cloned().collect()
    };

    ChannelListResponse {
        channels,
        total: Some(total),
    }
}
