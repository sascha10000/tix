pub struct OAuthClient {
    pub id: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub created_at: String,
}

/// A consumed authorization code, as needed to finish the token exchange.
pub struct AuthorizationCode {
    pub client_id: String,
    pub user_id: i64,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub scope: String,
}

/// A valid (unexpired, active-user) access or refresh token.
pub struct TokenGrant {
    pub client_id: String,
    pub user_id: i64,
    pub username: String,
    pub is_admin: bool,
    pub is_manager: bool,
    pub scope: String,
}

/// An app the user has authorized, shown on the profile page.
pub struct OAuthConnection {
    pub client_id: String,
    pub client_name: String,
    pub scope: String,
    pub last_issued_at: String,
}
