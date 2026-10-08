//! MCP server over the "Streamable HTTP" transport, in stateless JSON mode:
//! every JSON-RPC message is a `POST /mcp`, answered with a single JSON body
//! (no SSE stream, no `Mcp-Session-Id`). Every request needs an OAuth bearer token.

mod auth;
mod tools;

use actix_web::{web, HttpResponse};
use serde_json::{json, Value};

use ticketsystem_db::DbPool;

pub use auth::McpUser;

/// Newest protocol revision we implement; older ones are accepted on request.
const PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const MAX_BODY_BYTES: usize = 1 << 20;

const INSTRUCTIONS: &str = "Ticket system access. Start with list_projects to find project ids. \
Call get_project before creating tickets: it lists the active statuses, ticket types with their \
custom fields, allowed status transitions and project members (valid assignees). \
Use list_tickets to search a project and get_ticket for full details and allowed transitions. \
Dates use YYYY-MM-DD.";

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/mcp")
            .app_data(web::PayloadConfig::new(MAX_BODY_BYTES))
            .route(web::post().to(handle))
            // We never open server-initiated SSE streams or sessions.
            .route(web::get().to(method_not_allowed))
            .route(web::delete().to(method_not_allowed)),
    );
}

async fn method_not_allowed() -> HttpResponse {
    HttpResponse::MethodNotAllowed()
        .insert_header(("Allow", "POST"))
        .finish()
}

/// A JSON-RPC protocol-level error (as opposed to a tool execution error).
pub struct RpcError {
    code: i64,
    message: String,
}

impl RpcError {
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self { code: -32602, message: message.into() }
    }
}

fn rpc_response(id: Value, outcome: Result<Value, RpcError>) -> HttpResponse {
    let body = match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": e.code, "message": e.message } }),
    };
    HttpResponse::Ok().json(body)
}

async fn handle(user: McpUser, pool: web::Data<DbPool>, body: web::Bytes) -> HttpResponse {
    let Ok(msg) = serde_json::from_slice::<Value>(&body) else {
        return rpc_response(Value::Null, Err(RpcError { code: -32700, message: "Parse error".into() }));
    };
    let invalid = |id: Value| rpc_response(id, Err(RpcError { code: -32600, message: "Invalid Request".into() }));

    if !msg.is_object() {
        // JSON-RPC batches were removed from MCP in 2025-06-18.
        return invalid(Value::Null);
    }
    let id = msg.get("id").cloned();
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        // Responses from the client (we never send requests) need no answer.
        if msg.get("result").is_some() || msg.get("error").is_some() {
            return HttpResponse::Accepted().finish();
        }
        return invalid(id.unwrap_or(Value::Null));
    };
    // Notifications (no id), e.g. notifications/initialized: acknowledge only.
    let Some(id) = id else {
        return HttpResponse::Accepted().finish();
    };
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));

    let outcome = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools::definitions() })),
        "tools/call" => match pool.get() {
            Ok(conn) => tools::call(&conn, &user, &params),
            Err(e) => Err(RpcError { code: -32603, message: format!("Database unavailable: {e}") }),
        },
        _ => Err(RpcError { code: -32601, message: format!("Method not found: {method}") }),
    };
    rpc_response(id, outcome)
}

fn initialize(params: &Value) -> Value {
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .filter(|v| SUPPORTED_VERSIONS.contains(v))
        .unwrap_or(PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "ticketsystem",
            "title": "Ticketsystem",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": INSTRUCTIONS,
    })
}
