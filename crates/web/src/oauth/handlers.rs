use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse, ResponseError};
use askama::Template;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

use ticketsystem_auth::{generate_token, hash_token, verify_pkce_s256};
use ticketsystem_core::i18n::Translations;
use ticketsystem_core::models::oauth::OAuthClient;
use ticketsystem_db::repo::oauth;
use ticketsystem_db::rusqlite::Connection;
use ticketsystem_db::DbPool;

use super::{
    is_valid_redirect_uri, normalize_scope, OAuthSettings, CODE_TTL_MINUTES, SCOPE_READ, SCOPE_WRITE,
    SUPPORTED_SCOPES,
};
use crate::errors::AppError;
use crate::middleware::{extract_user, AuthenticatedUser, Lang};

// ---------------------------------------------------------------------------
// Discovery metadata
// ---------------------------------------------------------------------------

/// RFC 9728: tells MCP clients which authorization server protects `/mcp`.
pub async fn protected_resource_metadata(settings: web::Data<OAuthSettings>) -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "resource": settings.resource_url(),
        "authorization_servers": [settings.base_url],
        "scopes_supported": SUPPORTED_SCOPES,
        "bearer_methods_supported": ["header"],
        "resource_name": "Ticketsystem",
    }))
}

/// RFC 8414 authorization server metadata.
pub async fn authorization_server_metadata(settings: web::Data<OAuthSettings>) -> HttpResponse {
    let base = &settings.base_url;
    HttpResponse::Ok().json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth/authorize"),
        "token_endpoint": format!("{base}/oauth/token"),
        "registration_endpoint": format!("{base}/oauth/register"),
        "revocation_endpoint": format!("{base}/oauth/revoke"),
        "scopes_supported": SUPPORTED_SCOPES,
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "token_endpoint_auth_methods_supported": ["none"],
        "revocation_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    }))
}

// ---------------------------------------------------------------------------
// Dynamic client registration (RFC 7591)
// ---------------------------------------------------------------------------

const MAX_REDIRECT_URIS: usize = 10;
const MAX_REDIRECT_URI_LEN: usize = 2000;

#[derive(Deserialize)]
pub struct RegisterRequest {
    #[serde(default)]
    redirect_uris: Vec<String>,
    client_name: Option<String>,
}

/// Registers a public client. Any requested `token_endpoint_auth_method` is
/// overridden with `none`, as permitted by RFC 7591 §3.2.1.
pub async fn register(
    pool: web::Data<DbPool>,
    body: web::Json<RegisterRequest>,
) -> Result<HttpResponse, AppError> {
    let req = body.into_inner();
    let uris_ok = !req.redirect_uris.is_empty()
        && req.redirect_uris.len() <= MAX_REDIRECT_URIS
        && req
            .redirect_uris
            .iter()
            .all(|u| u.len() <= MAX_REDIRECT_URI_LEN && is_valid_redirect_uri(u));
    if !uris_ok {
        return Ok(oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_redirect_uri",
            "redirect_uris must be https URLs or http URLs on localhost/127.0.0.1/[::1]",
        ));
    }

    let name: String = req
        .client_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("MCP client")
        .chars()
        .take(100)
        .collect();
    let client_id = generate_token();

    let conn = pool.get()?;
    oauth::create_client(&conn, &client_id, &name, &req.redirect_uris)?;

    Ok(HttpResponse::Created().json(json!({
        "client_id": client_id,
        "client_name": name,
        "redirect_uris": req.redirect_uris,
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    })))
}

// ---------------------------------------------------------------------------
// Authorization endpoint
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
pub struct AuthorizeParams {
    response_type: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    scope: Option<String>,
    state: Option<String>,
    resource: Option<String>,
}

struct ValidRequest {
    client: OAuthClient,
    redirect_uri: String,
    code_challenge: String,
    scope: String,
    state: Option<String>,
}

/// Validates an authorization request.
///
/// Per RFC 6749 §4.1.2.1, problems with `client_id` or `redirect_uri` are shown to
/// the user and never redirected (otherwise we would be an open redirector). All
/// later problems are reported back to the client through the redirect URI.
fn validate_request(
    conn: &Connection,
    settings: &OAuthSettings,
    p: &AuthorizeParams,
) -> Result<ValidRequest, HttpResponse> {
    let client = p
        .client_id
        .as_deref()
        .and_then(|id| oauth::find_client(conn, id))
        .ok_or_else(|| AppError::BadRequest("Unknown client_id".into()).error_response())?;

    let redirect_uri = match p.redirect_uri.as_deref() {
        Some(uri) if client.redirect_uris.iter().any(|u| u == uri) => uri.to_string(),
        None if client.redirect_uris.len() == 1 => client.redirect_uris[0].clone(),
        _ => {
            return Err(AppError::BadRequest(
                "redirect_uri does not match a registered redirect URI".into(),
            )
            .error_response());
        }
    };

    let state = p.state.as_deref().filter(|s| !s.is_empty());
    let fail = |error: &str, description: &str| {
        redirect_with(&redirect_uri, &[("error", error), ("error_description", description)], state)
    };

    if p.response_type.as_deref() != Some("code") {
        return Err(fail("unsupported_response_type", "Only response_type=code is supported"));
    }
    let code_challenge = match (p.code_challenge.as_deref(), p.code_challenge_method.as_deref()) {
        (Some(c), Some("S256")) if (43..=128).contains(&c.len()) => c.to_string(),
        _ => {
            return Err(fail(
                "invalid_request",
                "PKCE is required: send code_challenge with code_challenge_method=S256",
            ));
        }
    };
    if let Some(resource) = p.resource.as_deref() {
        let resource = resource.trim_end_matches('/');
        if resource != settings.resource_url() && resource != settings.base_url {
            return Err(fail("invalid_target", "Unknown resource"));
        }
    }
    let scope = normalize_scope(p.scope.as_deref())
        .ok_or_else(|| fail("invalid_scope", "Supported scopes: tickets:read tickets:write"))?;

    Ok(ValidRequest {
        client,
        redirect_uri: redirect_uri.clone(),
        code_challenge,
        scope,
        state: state.map(str::to_string),
    })
}

/// Builds a 302 to a (pre-validated) redirect URI with extra query parameters.
fn redirect_with(redirect_uri: &str, params: &[(&str, &str)], state: Option<&str>) -> HttpResponse {
    let Ok(mut url) = url::Url::parse(redirect_uri) else {
        return AppError::BadRequest("Invalid redirect_uri".into()).error_response();
    };
    {
        let mut query = url.query_pairs_mut();
        for (k, v) in params {
            query.append_pair(k, v);
        }
        if let Some(state) = state {
            query.append_pair("state", state);
        }
    }
    HttpResponse::Found()
        .insert_header(("Location", url.as_str()))
        .finish()
}

#[derive(Template)]
#[template(path = "oauth/consent.html")]
struct ConsentTemplate {
    t: &'static Translations,
    user: AuthenticatedUser,
    client_name: String,
    redirect_host: String,
    scope_labels: Vec<&'static str>,
    client_id: String,
    redirect_uri: String,
    code_challenge: String,
    scope: String,
    state: String,
}

/// Human-readable descriptions of a granted scope string.
pub fn scope_labels(t: &'static Translations, scope: &str) -> Vec<&'static str> {
    scope
        .split_whitespace()
        .filter_map(|s| match s {
            SCOPE_READ => Some(t.oauth.scope_read),
            SCOPE_WRITE => Some(t.oauth.scope_write),
            _ => None,
        })
        .collect()
}

pub async fn authorize_page(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    settings: web::Data<OAuthSettings>,
    query: web::Query<AuthorizeParams>,
    Lang(t): Lang,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let v = match validate_request(&conn, &settings, &query) {
        Ok(v) => v,
        Err(response) => return Ok(response),
    };

    // Not logged in: go through the normal login form and come back here.
    let Some(user) = extract_user(&req) else {
        let next: String = url::form_urlencoded::byte_serialize(req.uri().to_string().as_bytes()).collect();
        return Ok(HttpResponse::Found()
            .insert_header(("Location", format!("/login?next={next}")))
            .finish());
    };

    let redirect_host = url::Url::parse(&v.redirect_uri)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();

    let page = ConsentTemplate {
        t,
        user,
        client_name: v.client.name,
        redirect_host,
        scope_labels: scope_labels(t, &v.scope),
        client_id: v.client.id,
        redirect_uri: v.redirect_uri,
        code_challenge: v.code_challenge,
        scope: v.scope,
        state: v.state.unwrap_or_default(),
    };
    let body = page.render().map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(HttpResponse::Ok()
        .content_type("text/html")
        // The consent page must never be framed (clickjacking).
        .insert_header(("X-Frame-Options", "DENY"))
        .insert_header(("Content-Security-Policy", "frame-ancestors 'none'"))
        .body(body))
}

#[derive(Deserialize)]
pub struct ConsentForm {
    client_id: String,
    redirect_uri: String,
    code_challenge: String,
    scope: String,
    #[serde(default)]
    state: String,
    decision: String,
}

/// Handles the Allow/Deny decision. The session cookie is `SameSite=Strict`, so a
/// cross-site form post arrives unauthenticated and is bounced to the login page.
pub async fn authorize_submit(
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    settings: web::Data<OAuthSettings>,
    form: web::Form<ConsentForm>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let form = form.into_inner();

    // Re-validate everything: hidden form fields are user-controlled.
    let params = AuthorizeParams {
        response_type: Some("code".into()),
        client_id: Some(form.client_id),
        redirect_uri: Some(form.redirect_uri),
        code_challenge: Some(form.code_challenge),
        code_challenge_method: Some("S256".into()),
        scope: Some(form.scope),
        state: Some(form.state),
        resource: None,
    };
    let v = match validate_request(&conn, &settings, &params) {
        Ok(v) => v,
        Err(response) => return Ok(response),
    };

    if form.decision != "allow" {
        return Ok(redirect_with(
            &v.redirect_uri,
            &[("error", "access_denied"), ("error_description", "The user denied the request")],
            v.state.as_deref(),
        ));
    }

    let code = generate_token();
    oauth::create_code(
        &conn,
        &hash_token(&code),
        &v.client.id,
        user.id,
        &v.redirect_uri,
        &v.code_challenge,
        &v.scope,
        CODE_TTL_MINUTES,
    )?;

    Ok(redirect_with(&v.redirect_uri, &[("code", &code)], v.state.as_deref()))
}

// ---------------------------------------------------------------------------
// Token endpoint
// ---------------------------------------------------------------------------

fn oauth_error(status: StatusCode, error: &str, description: &str) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header(("Cache-Control", "no-store"))
        .json(json!({ "error": error, "error_description": description }))
}

fn invalid_grant(description: &str) -> HttpResponse {
    oauth_error(StatusCode::BAD_REQUEST, "invalid_grant", description)
}

pub async fn token(
    pool: web::Data<DbPool>,
    settings: web::Data<OAuthSettings>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Ok(conn) = pool.get() else {
        return oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "Database unavailable");
    };
    let _ = oauth::purge_expired(&conn);
    let p = form.into_inner();

    let result = match p.get("grant_type").map(String::as_str) {
        Some("authorization_code") => exchange_code(&conn, &settings, &p),
        Some("refresh_token") => refresh(&conn, &settings, &p),
        _ => Err(oauth_error(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            "Supported grant types: authorization_code, refresh_token",
        )),
    };
    result.unwrap_or_else(|e| e)
}

fn exchange_code(
    conn: &Connection,
    settings: &OAuthSettings,
    p: &HashMap<String, String>,
) -> Result<HttpResponse, HttpResponse> {
    let (Some(code), Some(client_id), Some(verifier)) =
        (p.get("code"), p.get("client_id"), p.get("code_verifier"))
    else {
        return Err(oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "code, client_id and code_verifier are required",
        ));
    };

    // Consumes the code even if a later check fails, so a leaked code can't be retried.
    let grant = oauth::take_code(conn, &hash_token(code))
        .ok_or_else(|| invalid_grant("Authorization code is invalid, expired or already used"))?;

    if &grant.client_id != client_id {
        return Err(invalid_grant("Code was issued to another client"));
    }
    if let Some(uri) = p.get("redirect_uri")
        && uri != &grant.redirect_uri
    {
        return Err(invalid_grant("redirect_uri does not match the authorization request"));
    }
    if !verify_pkce_s256(verifier, &grant.code_challenge) {
        return Err(invalid_grant("PKCE verification failed"));
    }

    issue_tokens(conn, settings, &grant.client_id, grant.user_id, &grant.scope)
}

/// Refresh-token grant with rotation: the presented refresh token is invalidated
/// and a new pair is issued.
fn refresh(
    conn: &Connection,
    settings: &OAuthSettings,
    p: &HashMap<String, String>,
) -> Result<HttpResponse, HttpResponse> {
    let (Some(refresh_token), Some(client_id)) = (p.get("refresh_token"), p.get("client_id")) else {
        return Err(oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "refresh_token and client_id are required",
        ));
    };
    let hash = hash_token(refresh_token);
    let grant = oauth::find_valid_token(conn, &hash, "refresh")
        .ok_or_else(|| invalid_grant("Refresh token is invalid or expired"))?;
    if &grant.client_id != client_id {
        return Err(invalid_grant("Refresh token was issued to another client"));
    }

    oauth::delete_token(conn, &hash).map_err(|_| server_error())?;
    issue_tokens(conn, settings, &grant.client_id, grant.user_id, &grant.scope)
}

fn server_error() -> HttpResponse {
    oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "Could not issue token")
}

fn issue_tokens(
    conn: &Connection,
    settings: &OAuthSettings,
    client_id: &str,
    user_id: i64,
    scope: &str,
) -> Result<HttpResponse, HttpResponse> {
    let access_token = generate_token();
    let refresh_token = generate_token();
    oauth::create_token(
        conn,
        &hash_token(&access_token),
        "access",
        client_id,
        user_id,
        scope,
        settings.access_token_minutes,
    )
    .map_err(|_| server_error())?;
    oauth::create_token(
        conn,
        &hash_token(&refresh_token),
        "refresh",
        client_id,
        user_id,
        scope,
        settings.refresh_token_minutes,
    )
    .map_err(|_| server_error())?;

    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .insert_header(("Pragma", "no-cache"))
        .json(json!({
            "access_token": access_token,
            "token_type": "Bearer",
            "expires_in": settings.access_token_minutes * 60,
            "refresh_token": refresh_token,
            "scope": scope,
        })))
}

// ---------------------------------------------------------------------------
// Revocation (RFC 7009)
// ---------------------------------------------------------------------------

/// Always answers 200, whether or not the token existed (RFC 7009 §2.2).
pub async fn revoke(pool: web::Data<DbPool>, form: web::Form<HashMap<String, String>>) -> HttpResponse {
    if let (Some(token), Ok(conn)) = (form.get("token"), pool.get()) {
        let _ = oauth::delete_token(&conn, &hash_token(token));
    }
    HttpResponse::Ok().finish()
}
