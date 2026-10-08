//! CORS for the token-authenticated endpoints, so browser-based MCP clients
//! (e.g. MCP Inspector) can run discovery, registration, token exchange and MCP calls.
//!
//! A wildcard origin is safe here because these endpoints never use cookies:
//! `/mcp` requires a bearer token and the OAuth endpoints use explicit parameters.
//! The cookie-authenticated consent page (`/oauth/authorize`) is deliberately excluded.

use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::http::Method;
use actix_web::middleware::Next;
use actix_web::{Error, HttpResponse};

fn applies_to(path: &str) -> bool {
    path == "/mcp"
        || path.starts_with("/.well-known/oauth-")
        || matches!(path, "/oauth/token" | "/oauth/register" | "/oauth/revoke")
}

const HEADERS: [(&str, &str); 4] = [
    ("access-control-allow-origin", "*"),
    ("access-control-allow-methods", "GET, POST, DELETE, OPTIONS"),
    (
        "access-control-allow-headers",
        "Authorization, Content-Type, Accept, Mcp-Protocol-Version, Mcp-Session-Id",
    ),
    ("access-control-expose-headers", "WWW-Authenticate, Mcp-Session-Id"),
];

pub async fn cors(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    if !applies_to(req.path()) {
        return Ok(next.call(req).await?.map_into_left_body());
    }

    let mut res = if req.method() == Method::OPTIONS {
        req.into_response(HttpResponse::NoContent().finish()).map_into_right_body()
    } else {
        next.call(req).await?.map_into_left_body()
    };

    let headers = res.headers_mut();
    for (name, value) in HEADERS {
        headers.insert(HeaderName::from_static(name), HeaderValue::from_static(value));
    }
    headers.insert(
        HeaderName::from_static("access-control-max-age"),
        HeaderValue::from_static("86400"),
    );
    Ok(res)
}
