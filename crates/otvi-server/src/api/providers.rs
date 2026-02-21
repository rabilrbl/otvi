use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;

use otvi_core::types::*;

use crate::error::AppError;
use crate::state::AppState;

/// `GET /api/providers` — list all loaded providers.
pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ProviderInfo>>, AppError> {
    let providers: Vec<ProviderInfo> = state
        .providers
        .values()
        .map(|cfg| ProviderInfo {
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
        })
        .collect();

    Ok(Json(providers))
}

/// `GET /api/providers/:id` — get details for a single provider.
pub async fn get_info(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ProviderInfo>, AppError> {
    let cfg = state
        .providers
        .get(&id)
        .ok_or_else(|| AppError::NotFound(format!("Provider '{id}' not found")))?;

    Ok(Json(ProviderInfo {
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
    }))
}
