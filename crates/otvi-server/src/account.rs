use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use otvi_core::types::{
    AppLoginRequest, AppLoginResponse, ChangePasswordRequest, CreateUserRequest, RegisterRequest,
    UserInfo, UserRole,
};

use crate::auth_middleware::{Claims, create_token};
use crate::db::{self, UserRow};
use crate::error::AppError;
use crate::state::AppState;

/// Shared password-strength validator used by registration, change-password,
/// and admin reset.
pub fn validate_password(password: &str) -> Result<(), AppError> {
    let char_count = password.chars().count();
    if char_count < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".into(),
        ));
    }
    if char_count > 128 {
        return Err(AppError::BadRequest(
            "Password must be at most 128 characters".into(),
        ));
    }
    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        return Err(AppError::BadRequest(
            "Password must contain at least one uppercase letter".into(),
        ));
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest(
            "Password must contain at least one digit".into(),
        ));
    }
    if !password.chars().any(|c| !c.is_alphanumeric()) {
        return Err(AppError::BadRequest(
            "Password must contain at least one special character".into(),
        ));
    }
    Ok(())
}

/// Hash `password` with Argon2id.
pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("Password hash error: {e}")))
}

/// Verify `password` against an Argon2 `hash`.
pub fn verify_password(password: &str, hash: &str) -> Result<(), AppError> {
    let parsed =
        PasswordHash::new(hash).map_err(|e| AppError::Internal(format!("Invalid hash: {e}")))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| AppError::Unauthorized)
}

pub async fn register(
    state: &AppState,
    req: RegisterRequest,
) -> Result<AppLoginResponse, AppError> {
    if db::is_signup_disabled(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
    {
        return Err(AppError::BadRequest(
            "Public registration is disabled. Contact your administrator.".into(),
        ));
    }

    if req.username.trim().is_empty() || req.password.is_empty() {
        return Err(AppError::BadRequest(
            "Username and password are required".into(),
        ));
    }
    validate_password(&req.password)?;

    if db::get_user_by_username(&state.db, &req.username)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .is_some()
    {
        return Err(AppError::BadRequest("Username already taken".into()));
    }

    let count = db::user_count(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let role = if count == 0 {
        UserRole::Admin
    } else {
        UserRole::User
    };

    let hash = hash_password(&req.password)?;
    let user_id = db::create_user(&state.db, &req.username, &hash, &role, false)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let providers = db::get_user_providers(&state.db, &user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(login_response(
        state,
        user_id,
        req.username,
        role,
        providers,
        false,
    ))
}

pub async fn login(state: &AppState, req: AppLoginRequest) -> Result<AppLoginResponse, AppError> {
    let row = db::get_user_by_username(&state.db, &req.username)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    verify_password(&req.password, &row.password_hash)?;
    login_response_for_row(state, row).await
}

pub async fn current_user(state: &AppState, claims: &Claims) -> Result<UserInfo, AppError> {
    let row = db::get_user_by_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    user_info_for_row(state, row).await
}

pub async fn change_password(
    state: &AppState,
    claims: &Claims,
    req: ChangePasswordRequest,
) -> Result<AppLoginResponse, AppError> {
    validate_password(&req.new_password)?;

    let row = db::get_user_by_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    verify_password(&req.current_password, &row.password_hash)?;

    let new_hash = hash_password(&req.new_password)?;
    db::update_password(&state.db, &claims.sub, &new_hash)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let providers = db::get_user_providers(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(login_response(
        state,
        claims.sub.clone(),
        claims.username.clone(),
        claims.role(),
        providers,
        false,
    ))
}

pub async fn list_users(state: &AppState) -> Result<Vec<UserInfo>, AppError> {
    let (rows, mut providers_by_user) = tokio::try_join!(
        db::list_users(&state.db),
        db::get_all_user_providers(&state.db),
    )
    .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let providers = providers_by_user.remove(&row.id).unwrap_or_default();
            UserInfo {
                id: row.id,
                username: row.username,
                role: role_from_db(&row.role),
                providers,
                must_change_password: row.must_change_password,
            }
        })
        .collect())
}

pub async fn create_user(state: &AppState, req: CreateUserRequest) -> Result<UserInfo, AppError> {
    if req.username.trim().is_empty() || req.password.is_empty() {
        return Err(AppError::BadRequest(
            "Username and password are required".into(),
        ));
    }
    validate_password(&req.password)?;

    if db::get_user_by_username(&state.db, &req.username)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .is_some()
    {
        return Err(AppError::BadRequest("Username already taken".into()));
    }

    let hash = hash_password(&req.password)?;
    let user_id = db::create_user(&state.db, &req.username, &hash, &req.role, true)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    db::set_user_providers(&state.db, &user_id, &req.providers)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(UserInfo {
        id: user_id,
        username: req.username,
        role: req.role,
        providers: req.providers,
        must_change_password: true,
    })
}

pub async fn reset_user_password(
    state: &AppState,
    user_id: &str,
    new_password: &str,
) -> Result<(), AppError> {
    if new_password.is_empty() {
        return Err(AppError::BadRequest("Password must not be empty".into()));
    }
    validate_password(new_password)?;

    let hash = hash_password(new_password)?;
    db::update_password(&state.db, user_id, &hash)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    db::set_must_change_password(&state.db, user_id, true)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(())
}

async fn login_response_for_row(
    state: &AppState,
    row: UserRow,
) -> Result<AppLoginResponse, AppError> {
    let role = role_from_db(&row.role);
    let providers = db::get_user_providers(&state.db, &row.id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(login_response(
        state,
        row.id,
        row.username,
        role,
        providers,
        row.must_change_password,
    ))
}

async fn user_info_for_row(state: &AppState, row: UserRow) -> Result<UserInfo, AppError> {
    let providers = db::get_user_providers(&state.db, &row.id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(UserInfo {
        id: row.id,
        username: row.username,
        role: role_from_db(&row.role),
        providers,
        must_change_password: row.must_change_password,
    })
}

fn login_response(
    state: &AppState,
    user_id: String,
    username: String,
    role: UserRole,
    providers: Vec<String>,
    must_change_password: bool,
) -> AppLoginResponse {
    let token = create_token(
        &state.jwt_keys,
        &user_id,
        &username,
        &role,
        must_change_password,
    );

    AppLoginResponse {
        token,
        user: UserInfo {
            id: user_id,
            username,
            role,
            providers,
            must_change_password,
        },
    }
}

fn role_from_db(role: &str) -> UserRole {
    match role {
        "admin" => UserRole::Admin,
        _ => UserRole::User,
    }
}

#[cfg(test)]
mod tests {
    use super::validate_password;

    fn policy_fixture(
        has_uppercase: bool,
        has_digit: bool,
        has_special: bool,
        len: usize,
    ) -> String {
        let mut password = String::new();
        password.push(if has_uppercase { 'A' } else { 'a' });
        password.push(if has_digit { '1' } else { 'b' });
        password.push(if has_special { '!' } else { 'c' });
        password.extend(std::iter::repeat_n('d', len.saturating_sub(password.len())));
        password
    }

    #[test]
    fn password_too_short_rejected() {
        assert!(validate_password(&policy_fixture(true, true, true, 6)).is_err());
    }

    #[test]
    fn password_exactly_min_length_passes() {
        assert!(validate_password(&policy_fixture(true, true, true, 8)).is_ok());
    }

    #[test]
    fn password_exactly_max_length_passes() {
        let password = format!("A1!{}", "a".repeat(125));
        assert_eq!(password.chars().count(), 128);
        assert!(validate_password(&password).is_ok());
    }

    #[test]
    fn password_over_max_length_rejected() {
        let password = format!("A1!{}", "a".repeat(126));
        assert_eq!(password.chars().count(), 129);
        assert!(validate_password(&password).is_err());
    }

    #[test]
    fn password_missing_uppercase_rejected() {
        assert!(validate_password(&policy_fixture(false, true, true, 8)).is_err());
    }

    #[test]
    fn password_missing_digit_rejected() {
        assert!(validate_password(&policy_fixture(true, false, true, 11)).is_err());
    }

    #[test]
    fn password_missing_special_char_rejected() {
        assert!(validate_password(&policy_fixture(true, true, false, 10)).is_err());
    }

    #[test]
    fn password_valid_passes() {
        assert!(validate_password(&policy_fixture(true, true, true, 10)).is_ok());
    }

    #[test]
    fn password_max_length_is_char_count_not_bytes() {
        let password = format!("A1!{}", "Á".repeat(125));
        assert_eq!(password.chars().count(), 128);
        assert!(password.len() > 128, "sanity: byte count exceeds 128");
        assert!(validate_password(&password).is_ok());

        let too_long = format!("A1!{}", "Á".repeat(126));
        assert_eq!(too_long.chars().count(), 129);
        assert!(validate_password(&too_long).is_err());
    }
}
