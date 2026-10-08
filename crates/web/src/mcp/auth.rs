use actix_web::dev::Payload;
use actix_web::http::header::AUTHORIZATION;
use actix_web::{web, FromRequest, HttpRequest, HttpResponse};
use serde_json::json;
use std::future::{ready, Ready};

use ticketsystem_db::repo::oauth;
use ticketsystem_db::DbPool;

use crate::middleware::AuthenticatedUser;
use crate::oauth::OAuthSettings;

/// The user behind a valid OAuth access token, plus the scopes it grants.
pub struct McpUser {
    pub user: AuthenticatedUser,
    scopes: Vec<String>,
}

impl McpUser {
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

fn bearer_token(req: &HttpRequest) -> Option<&str> {
    let value = req.headers().get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

/// 401 with the `WWW-Authenticate` header that lets MCP clients discover the
/// authorization server (RFC 9728 §5.1).
fn unauthorized(req: &HttpRequest, token_was_sent: bool) -> actix_web::Error {
    let metadata_url = req
        .app_data::<web::Data<OAuthSettings>>()
        .map(|s| s.resource_metadata_url())
        .unwrap_or_default();
    let challenge = if token_was_sent {
        format!(r#"Bearer error="invalid_token", resource_metadata="{metadata_url}""#)
    } else {
        format!(r#"Bearer resource_metadata="{metadata_url}""#)
    };
    let response = HttpResponse::Unauthorized()
        .insert_header(("WWW-Authenticate", challenge))
        .json(json!({
            "error": if token_was_sent { "invalid_token" } else { "unauthorized" },
            "error_description": "A valid OAuth access token is required",
        }));
    actix_web::error::InternalError::from_response("Unauthorized", response).into()
}

impl FromRequest for McpUser {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let Some(token) = bearer_token(req) else {
            return ready(Err(unauthorized(req, false)));
        };
        let grant = req
            .app_data::<web::Data<DbPool>>()
            .and_then(|pool| pool.get().ok())
            .and_then(|conn| oauth::find_valid_token(&conn, &ticketsystem_auth::hash_token(token), "access"));

        ready(match grant {
            Some(g) => Ok(McpUser {
                user: AuthenticatedUser {
                    id: g.user_id,
                    username: g.username,
                    is_admin: g.is_admin,
                    is_manager: g.is_manager,
                },
                scopes: g.scope.split_whitespace().map(str::to_string).collect(),
            }),
            None => Err(unauthorized(req, true)),
        })
    }
}
