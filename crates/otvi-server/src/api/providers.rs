use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};

use otvi_core::types::*;

use crate::auth_middleware::ActiveClaims;
use crate::db;
use crate::error::AppError;
use crate::state::AppState;

/// `GET /api/providers` — list providers accessible to the authenticated user.
///
/// If the user has an explicit provider allow-list, only those providers are
/// returned.  An empty allow-list means access to all loaded providers.
#[utoipa::path(
    get,
    path = "/api/providers",
    tag = "providers",
    security(("bearer_token" = [])),
    responses(
        (status = 200, description = "List of providers accessible to the user", body = Vec<ProviderInfo>),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Password change required"),
    ),
)]
pub async fn list(
    State(state): State<Arc<AppState>>,
    claims: ActiveClaims,
) -> Result<Json<Vec<ProviderInfo>>, AppError> {
    let allowed = db::get_user_providers(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let providers = state.with_providers(|map| {
        map.values()
            .filter(|cfg| allowed.is_empty() || allowed.contains(&cfg.provider.id))
            .map(provider_to_info)
            .collect::<Vec<_>>()
    });

    Ok(Json(providers))
}

/// `GET /api/providers/:id` — get details for a single provider, if accessible.
#[utoipa::path(
    get,
    path = "/api/providers/{id}",
    tag = "providers",
    security(("bearer_token" = [])),
    params(
        ("id" = String, Path, description = "Provider ID"),
    ),
    responses(
        (status = 200, description = "Provider details and auth flow definitions", body = ProviderInfo),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Password change required or access denied"),
        (status = 404, description = "Provider not found or not accessible"),
    ),
)]
pub async fn get_info(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    claims: ActiveClaims,
) -> Result<Json<ProviderInfo>, AppError> {
    // Check access.
    let allowed = db::get_user_providers(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !allowed.is_empty() && !allowed.contains(&id) {
        return Err(AppError::NotFound(format!("Provider '{id}' not found")));
    }

    let info = state
        .with_provider(&id, provider_to_info)
        .ok_or_else(|| AppError::NotFound(format!("Provider '{id}' not found")))?;

    Ok(Json(info))
}

pub fn provider_to_info(cfg: &otvi_core::config::ProviderConfig) -> ProviderInfo {
    ProviderInfo {
        id: cfg.provider.id.clone(),
        name: cfg.provider.name.clone(),
        logo: cfg.provider.logo.clone(),
        auth_flows: cfg
            .auth
            .flows
            .iter()
            .map(|f| AuthFlowInfo {
                id: f.id.clone(),
                name: f.name.clone(),
                fields: f
                    .inputs
                    .iter()
                    .map(|i| FieldInfo {
                        key: i.key.clone(),
                        label: i.label.clone(),
                        field_type: i.field_type.clone(),
                        required: i.required,
                    })
                    .collect(),
            })
            .collect(),
    }
}
