use std::collections::HashSet;
use std::sync::Arc;

use otvi_core::config::AuthScope;

use crate::auth_middleware::ActiveClaims;
use crate::db;
use crate::error::AppError;
use crate::state::AppState;

pub async fn authorize_provider_route(
    state: &Arc<AppState>,
    claims: &ActiveClaims,
    provider_id: &str,
    require_global_admin: bool,
) -> Result<AuthScope, AppError> {
    let scope = state
        .with_provider(provider_id, |p| p.auth.scope.clone())
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let allowed = db::get_user_providers(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let allowed: HashSet<String> = allowed.into_iter().collect();

    if !allowed.is_empty() && !allowed.contains(provider_id) {
        return Err(AppError::Forbidden(format!(
            "Access denied for provider '{provider_id}'"
        )));
    }

    if require_global_admin && scope == AuthScope::Global && !claims.is_admin() {
        return Err(AppError::Forbidden(
            "Admin access required to manage global provider credentials".into(),
        ));
    }

    Ok(scope)
}
