use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;

use otvi_core::types::*;

use crate::auth_middleware::Claims;
use crate::db;
use crate::error::AppError;
use crate::state::AppState;

/// `GET /api/providers` — list providers accessible to the authenticated user.
///
/// If the user has an explicit provider allow-list, only those providers are
/// returned.  An empty allow-list means access to all loaded providers.
pub async fn list(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Json<Vec<ProviderInfo>>, AppError> {
    let allowed = db::get_user_providers(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let providers: Vec<ProviderInfo> = state
        .providers
        .values()
        .filter(|cfg| allowed.is_empty() || allowed.contains(&cfg.provider.id))
        .map(provider_to_info)
        .collect();

    Ok(Json(providers))
}

/// `GET /api/providers/:id` — get details for a single provider, if accessible.
pub async fn get_info(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    claims: Claims,
) -> Result<Json<ProviderInfo>, AppError> {
    // Check access.
    let allowed = db::get_user_providers(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !allowed.is_empty() && !allowed.contains(&id) {
        return Err(AppError::NotFound(format!("Provider '{id}' not found")));
    }

    let cfg = state
        .providers
        .get(&id)
        .ok_or_else(|| AppError::NotFound(format!("Provider '{id}' not found")))?;

    Ok(Json(provider_to_info(cfg)))
}

fn provider_to_info(cfg: &otvi_core::config::ProviderConfig) -> ProviderInfo {
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
