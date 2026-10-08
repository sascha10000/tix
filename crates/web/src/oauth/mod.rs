//! OAuth 2.1 authorization server for the MCP endpoint.
//!
//! Implements the subset required by the MCP authorization spec:
//! protected-resource metadata (RFC 9728), authorization-server metadata (RFC 8414),
//! dynamic client registration (RFC 7591), the authorization-code grant with
//! mandatory PKCE S256, refresh-token rotation and token revocation (RFC 7009).
//! All clients are public clients; tokens are opaque and stored hashed.

mod handlers;

pub use handlers::scope_labels;

use actix_web::web;

pub const SCOPE_READ: &str = "tickets:read";
pub const SCOPE_WRITE: &str = "tickets:write";
pub const SUPPORTED_SCOPES: [&str; 2] = [SCOPE_READ, SCOPE_WRITE];

/// Lifetime of an authorization code. Clients redeem it within seconds.
const CODE_TTL_MINUTES: i64 = 10;

#[derive(Clone)]
pub struct OAuthSettings {
    /// Public base URL without trailing slash, e.g. `https://tickets.example.com`.
    pub base_url: String,
    pub access_token_minutes: i64,
    pub refresh_token_minutes: i64,
}

impl OAuthSettings {
    /// The protected resource (RFC 8707 resource indicator).
    pub fn resource_url(&self) -> String {
        format!("{}/mcp", self.base_url)
    }

    pub fn resource_metadata_url(&self) -> String {
        format!("{}/.well-known/oauth-protected-resource", self.base_url)
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route(
        "/.well-known/oauth-protected-resource",
        web::get().to(handlers::protected_resource_metadata),
    )
    .route(
        "/.well-known/oauth-protected-resource/mcp",
        web::get().to(handlers::protected_resource_metadata),
    )
    .route(
        "/.well-known/oauth-authorization-server",
        web::get().to(handlers::authorization_server_metadata),
    )
    .route("/oauth/register", web::post().to(handlers::register))
    .route("/oauth/authorize", web::get().to(handlers::authorize_page))
    .route("/oauth/authorize", web::post().to(handlers::authorize_submit))
    .route("/oauth/token", web::post().to(handlers::token))
    .route("/oauth/revoke", web::post().to(handlers::revoke));
}

/// Space-separated scope string as granted: requested scopes filtered to the
/// supported set, in canonical order. An absent/empty request grants all scopes.
/// Returns `None` if any requested scope is unknown.
pub fn normalize_scope(requested: Option<&str>) -> Option<String> {
    let requested: Vec<&str> = requested.unwrap_or("").split_whitespace().collect();
    if requested.iter().any(|s| !SUPPORTED_SCOPES.contains(s)) {
        return None;
    }
    let granted: Vec<&str> = SUPPORTED_SCOPES
        .iter()
        .copied()
        .filter(|s| requested.is_empty() || requested.contains(s))
        .collect();
    Some(granted.join(" "))
}

/// Redirect URIs must be absolute https URLs, or http on a loopback host
/// (native/CLI clients), and must not carry a fragment.
pub fn is_valid_redirect_uri(uri: &str) -> bool {
    let Ok(url) = url::Url::parse(uri) else {
        return false;
    };
    if url.fragment().is_some() {
        return false;
    }
    match url.scheme() {
        "https" => url.host().is_some(),
        "http" => matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]")
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_uri_rules() {
        assert!(is_valid_redirect_uri("https://claude.ai/api/mcp/auth_callback"));
        assert!(is_valid_redirect_uri("http://localhost:33418/callback"));
        assert!(is_valid_redirect_uri("http://127.0.0.1:6274/oauth/callback"));
        assert!(is_valid_redirect_uri("http://[::1]:8000/cb"));
        assert!(!is_valid_redirect_uri("http://evil.example.com/cb"));
        assert!(!is_valid_redirect_uri("https://example.com/cb#frag"));
        assert!(!is_valid_redirect_uri("javascript:alert(1)"));
        assert!(!is_valid_redirect_uri("/relative"));
    }

    #[test]
    fn scope_normalization() {
        assert_eq!(normalize_scope(None).as_deref(), Some("tickets:read tickets:write"));
        assert_eq!(normalize_scope(Some("")).as_deref(), Some("tickets:read tickets:write"));
        assert_eq!(normalize_scope(Some("tickets:read")).as_deref(), Some("tickets:read"));
        assert_eq!(
            normalize_scope(Some("tickets:write tickets:read")).as_deref(),
            Some("tickets:read tickets:write")
        );
        assert_eq!(normalize_scope(Some("tickets:read admin")), None);
    }
}
