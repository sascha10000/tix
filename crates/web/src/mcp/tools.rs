//! MCP tool definitions and implementations.
//!
//! Every tool runs as the token's user and enforces the same rules as the web UI
//! (see `crate::access`). Domain failures are returned as tool results with
//! `isError: true` so the model can read the message and correct its call.

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use ticketsystem_core::models::project::Project;
use ticketsystem_core::models::ticket::Ticket;
use ticketsystem_core::models::ticket_type::CustomField;
use ticketsystem_db::repo::{project, status, ticket, ticket_type};
use ticketsystem_db::rusqlite::Connection;

use super::{McpUser, RpcError};
use crate::access::{can_edit_ticket, can_transition_ticket, check_project_access};
use crate::errors::AppError;
use crate::middleware::AuthenticatedUser;
use crate::oauth::{SCOPE_READ, SCOPE_WRITE};

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 200;
const MAX_TITLE_LEN: usize = 500;
const PREVIEW_CHARS: usize = 200;

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// A failure the model should see as tool output.
struct ToolError(String);

type ToolResult = Result<Value, ToolError>;

impl From<AppError> for ToolError {
    fn from(e: AppError) -> Self {
        match e {
            AppError::NotFound(msg) | AppError::BadRequest(msg) => ToolError(msg),
            AppError::Forbidden => ToolError("You do not have access to this project".into()),
            AppError::Internal(msg) => {
                eprintln!("MCP tool internal error: {msg}");
                ToolError("Internal error".into())
            }
        }
    }
}

impl From<ticketsystem_db::rusqlite::Error> for ToolError {
    fn from(e: ticketsystem_db::rusqlite::Error) -> Self {
        AppError::from(e).into()
    }
}

fn fail<T>(msg: impl Into<String>) -> Result<T, ToolError> {
    Err(ToolError(msg.into()))
}

fn parse_args<T: DeserializeOwned>(args: Value) -> Result<T, ToolError> {
    serde_json::from_value(args).map_err(|e| ToolError(format!("Invalid arguments: {e}")))
}

fn tool_result(outcome: ToolResult) -> Value {
    match outcome {
        Ok(value) => json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
            "structuredContent": value,
            "isError": false,
        }),
        Err(ToolError(msg)) => json!({
            "content": [{ "type": "text", "text": msg }],
            "isError": true,
        }),
    }
}

pub fn call(conn: &Connection, mcp_user: &McpUser, params: &Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params("Missing tool name"))?;
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

    let required_scope = match name {
        "list_projects" | "get_project" | "list_tickets" | "get_ticket" | "list_my_tickets" => SCOPE_READ,
        "create_ticket" | "update_ticket" | "transition_ticket" => SCOPE_WRITE,
        _ => return Err(RpcError::invalid_params(format!("Unknown tool: {name}"))),
    };
    if !mcp_user.has_scope(required_scope) {
        return Ok(tool_result(fail(format!(
            "The access token was not granted the '{required_scope}' scope. Reconnect and approve it."
        ))));
    }

    let user = &mcp_user.user;
    let outcome = match name {
        "list_projects" => list_projects(conn, user),
        "get_project" => parse_args(args).and_then(|a| get_project(conn, user, a)),
        "list_tickets" => parse_args(args).and_then(|a| list_tickets(conn, user, a)),
        "get_ticket" => parse_args(args).and_then(|a: TicketIdArgs| ticket_detail(conn, user, a.ticket_id)),
        "list_my_tickets" => parse_args(args).and_then(|a| list_my_tickets(conn, user, a)),
        "create_ticket" => parse_args(args).and_then(|a| create_ticket(conn, user, a)),
        "update_ticket" => parse_args(args).and_then(|a| update_ticket(conn, user, a)),
        "transition_ticket" => parse_args(args).and_then(|a| transition_ticket(conn, user, a)),
        _ => unreachable!("tool names are matched above"),
    };
    Ok(tool_result(outcome))
}

// ---------------------------------------------------------------------------
// Tool definitions (tools/list)
// ---------------------------------------------------------------------------

fn read_only() -> Value {
    json!({ "readOnlyHint": true, "openWorldHint": false })
}

fn writes(idempotent: bool) -> Value {
    json!({ "readOnlyHint": false, "destructiveHint": false, "idempotentHint": idempotent, "openWorldHint": false })
}

pub fn definitions() -> Value {
    let fields_schema = json!({
        "type": "object",
        "description": "Custom field values keyed by field name (or field id). See get_project for the fields of each ticket type. User fields take a user id, ticket fields a ticket id from the same project, date fields YYYY-MM-DD.",
        "additionalProperties": { "type": ["string", "number", "null"] }
    });
    json!([
        {
            "name": "list_projects",
            "title": "List projects",
            "description": "List all projects you can access (admins see every project), with your role in each.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": read_only(),
        },
        {
            "name": "get_project",
            "title": "Get project",
            "description": "Project details needed to work with its tickets: active statuses (in order), allowed status transitions, active ticket types with their custom fields, and members (valid assignees).",
            "inputSchema": {
                "type": "object",
                "properties": { "project_id": { "type": "integer" } },
                "required": ["project_id"],
            },
            "annotations": read_only(),
        },
        {
            "name": "list_tickets",
            "title": "List tickets in a project",
            "description": "Select tickets in a project, newest activity first. All filters are optional and combined with AND.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "integer" },
                    "status_id": { "type": "integer", "description": "Only tickets in this status" },
                    "assignee_id": { "type": "integer", "description": "Only tickets assigned to this user" },
                    "ticket_type_id": { "type": "integer", "description": "Only tickets of this type" },
                    "query": { "type": "string", "description": "Case-insensitive text search in title and description" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "default": DEFAULT_LIMIT },
                    "offset": { "type": "integer", "minimum": 0, "default": 0 },
                },
                "required": ["project_id"],
            },
            "annotations": read_only(),
        },
        {
            "name": "get_ticket",
            "title": "Get ticket",
            "description": "Full ticket details: description, custom field values, the statuses it can transition to, and whether you may edit or transition it.",
            "inputSchema": {
                "type": "object",
                "properties": { "ticket_id": { "type": "integer" } },
                "required": ["ticket_id"],
            },
            "annotations": read_only(),
        },
        {
            "name": "list_my_tickets",
            "title": "List my tickets",
            "description": "Tickets across all your projects that are assigned to you or created by you.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "default": DEFAULT_LIMIT },
                },
            },
            "annotations": read_only(),
        },
        {
            "name": "create_ticket",
            "title": "Create ticket",
            "description": "Create a ticket in a project. Call get_project first to find valid ticket types, statuses, members and required custom fields.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "integer" },
                    "title": { "type": "string" },
                    "ticket_type_id": { "type": "integer", "description": "Must be active in the project" },
                    "due_date": { "type": "string", "format": "date", "description": "YYYY-MM-DD" },
                    "text": { "type": "string", "description": "Description" },
                    "status_id": { "type": "integer", "description": "Defaults to the project's first active status" },
                    "assignee_id": { "type": "integer", "description": "A project member; defaults to you" },
                    "fields": fields_schema,
                },
                "required": ["project_id", "title", "ticket_type_id", "due_date"],
            },
            "annotations": writes(false),
        },
        {
            "name": "update_ticket",
            "title": "Update ticket",
            "description": "Change a ticket's title, description, assignee, due date or custom fields. Omitted values stay unchanged. Use transition_ticket to change the status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer" },
                    "title": { "type": "string" },
                    "text": { "type": "string" },
                    "assignee_id": { "type": "integer" },
                    "due_date": { "type": "string", "format": "date", "description": "YYYY-MM-DD" },
                    "fields": fields_schema,
                },
                "required": ["ticket_id"],
            },
            "annotations": writes(true),
        },
        {
            "name": "transition_ticket",
            "title": "Change ticket status",
            "description": "Move a ticket to another status. Only transitions allowed by the workflow are possible; get_ticket lists them.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer" },
                    "status_id": { "type": "integer" },
                },
                "required": ["ticket_id", "status_id"],
            },
            "annotations": writes(false),
        },
    ])
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

fn ticket_summary(t: &Ticket) -> Value {
    let preview: String = t.text.chars().take(PREVIEW_CHARS).collect();
    json!({
        "id": t.id,
        "project_id": t.project_id,
        "title": t.title,
        "text_preview": preview,
        "status": { "id": t.status_id, "name": t.status_name },
        "type": { "id": t.ticket_type_id, "name": t.type_name },
        "assignee": { "id": t.assignee_id, "username": t.assignee_name },
        "creator": { "id": t.creator_id, "username": t.creator_name },
        "due_date": t.due_date,
        "created_at": t.created_at,
        "updated_at": t.updated_at,
    })
}

fn project_ref(p: &Project) -> Value {
    json!({ "id": p.id, "name": p.name })
}

fn clamp_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

/// Loads a ticket the user may see. Tickets in projects the user can't access
/// are reported as not found, so ticket ids can't be probed across projects.
fn load_ticket(conn: &Connection, user: &AuthenticatedUser, ticket_id: i64) -> Result<(Ticket, Project), ToolError> {
    let not_found = || ToolError(format!("Ticket {ticket_id} not found"));
    let t = ticket::find_by_id(conn, ticket_id)
        .filter(|t| !t.is_deleted)
        .ok_or_else(not_found)?;
    let p = check_project_access(conn, user, t.project_id).map_err(|_| not_found())?;
    Ok((t, p))
}

fn allowed_transitions(conn: &Connection, project_id: i64, from_status_id: i64) -> Vec<Value> {
    let active = project::list_active_status_ids(conn, project_id);
    status::list_all(conn)
        .into_iter()
        .filter(|s| {
            s.id != from_status_id && active.contains(&s.id) && status::has_transition(conn, from_status_id, s.id)
        })
        .map(|s| json!({ "id": s.id, "name": s.name }))
        .collect()
}

// ---------------------------------------------------------------------------
// Read tools
// ---------------------------------------------------------------------------

fn list_projects(conn: &Connection, user: &AuthenticatedUser) -> ToolResult {
    let projects = if user.is_admin() {
        project::list_all(conn)
    } else {
        project::list_for_user(conn, user.id)
    };
    let items: Vec<Value> = projects
        .iter()
        .map(|p| {
            let role = project::get_member_role(conn, p.id, user.id)
                .or_else(|| user.is_admin().then(|| "admin".to_string()));
            json!({
                "id": p.id,
                "name": p.name,
                "description": p.description,
                "your_role": role,
                "created_at": p.created_at,
            })
        })
        .collect();
    Ok(json!({ "projects": items }))
}

#[derive(Deserialize)]
struct ProjectIdArgs {
    project_id: i64,
}

fn get_project(conn: &Connection, user: &AuthenticatedUser, args: ProjectIdArgs) -> ToolResult {
    let p = check_project_access(conn, user, args.project_id)?;

    let active_status_ids = project::list_active_status_ids(conn, p.id);
    let statuses: Vec<Value> = status::list_all(conn)
        .into_iter()
        .filter(|s| active_status_ids.contains(&s.id))
        .map(|s| json!({ "id": s.id, "name": s.name, "color": s.color, "position": s.position }))
        .collect();

    let transitions: Vec<Value> = status::list_workflows(conn)
        .into_iter()
        .filter(|w| active_status_ids.contains(&w.from_status_id) && active_status_ids.contains(&w.to_status_id))
        .map(|w| json!({ "from_status_id": w.from_status_id, "to_status_id": w.to_status_id }))
        .collect();

    let active_type_ids = project::list_active_ticket_type_ids(conn, p.id);
    let ticket_types: Vec<Value> = ticket_type::list_all(conn)
        .into_iter()
        .filter(|tt| active_type_ids.contains(&tt.id))
        .map(|tt| {
            let fields: Vec<Value> = ticket_type::list_fields(conn, tt.id)
                .into_iter()
                .map(|f| {
                    json!({
                        "id": f.id,
                        "name": f.name,
                        "type": f.field_type,
                        "required": f.is_required,
                        "min": f.num_min,
                        "max": f.num_max,
                        "step": f.num_step,
                        "default_value": f.default_value,
                    })
                })
                .collect();
            json!({ "id": tt.id, "name": tt.name, "description": tt.description, "fields": fields })
        })
        .collect();

    let members: Vec<Value> = project::list_members(conn, p.id)
        .into_iter()
        .map(|m| json!({ "user_id": m.user_id, "username": m.username, "role": m.role }))
        .collect();

    Ok(json!({
        "project": {
            "id": p.id,
            "name": p.name,
            "description": p.description,
            "created_at": p.created_at,
        },
        "statuses": statuses,
        "transitions": transitions,
        "ticket_types": ticket_types,
        "members": members,
    }))
}

#[derive(Deserialize)]
struct ListTicketsArgs {
    project_id: i64,
    status_id: Option<i64>,
    assignee_id: Option<i64>,
    ticket_type_id: Option<i64>,
    query: Option<String>,
    limit: Option<usize>,
    #[serde(default)]
    offset: usize,
}

fn list_tickets(conn: &Connection, user: &AuthenticatedUser, args: ListTicketsArgs) -> ToolResult {
    let p = check_project_access(conn, user, args.project_id)?;
    let needle = args
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(str::to_lowercase);

    let matching: Vec<Ticket> = ticket::list_for_project(conn, p.id)
        .into_iter()
        .filter(|t| args.status_id.is_none_or(|id| t.status_id == id))
        .filter(|t| args.assignee_id.is_none_or(|id| t.assignee_id == id))
        .filter(|t| args.ticket_type_id.is_none_or(|id| t.ticket_type_id == id))
        .filter(|t| {
            needle.as_deref().is_none_or(|q| {
                t.title.to_lowercase().contains(q) || t.text.to_lowercase().contains(q)
            })
        })
        .collect();

    let limit = clamp_limit(args.limit);
    let page: Vec<Value> = matching.iter().skip(args.offset).take(limit).map(ticket_summary).collect();

    Ok(json!({
        "project": project_ref(&p),
        "total": matching.len(),
        "offset": args.offset,
        "limit": limit,
        "tickets": page,
    }))
}

#[derive(Deserialize)]
struct TicketIdArgs {
    ticket_id: i64,
}

fn ticket_detail(conn: &Connection, user: &AuthenticatedUser, ticket_id: i64) -> ToolResult {
    let (t, p) = load_ticket(conn, user, ticket_id)?;

    let fields: Vec<Value> = ticket::get_field_values(conn, t.id)
        .into_iter()
        .map(|fv| {
            // Resolve references for readability, but only within this project.
            let display = match fv.field_type.as_str() {
                "user" => fv
                    .value
                    .parse::<i64>()
                    .ok()
                    .and_then(|uid| project::list_members(conn, p.id).into_iter().find(|m| m.user_id == uid))
                    .map(|m| m.username),
                "ticket" => fv
                    .value
                    .parse::<i64>()
                    .ok()
                    .and_then(|tid| ticket::find_by_id(conn, tid))
                    .filter(|rt| rt.project_id == p.id && !rt.is_deleted)
                    .map(|rt| format!("#{} - {}", rt.id, rt.title)),
                _ => None,
            };
            json!({
                "id": fv.custom_field_id,
                "name": fv.field_name,
                "type": fv.field_type,
                "required": fv.is_required,
                "value": fv.value,
                "display": display,
            })
        })
        .collect();

    let role = project::get_member_role(conn, p.id, user.id);
    let mut detail = ticket_summary(&t);
    detail["text"] = json!(t.text);
    if let Some(obj) = detail.as_object_mut() {
        obj.remove("text_preview");
    }

    Ok(json!({
        "ticket": detail,
        "project": project_ref(&p),
        "fields": fields,
        "allowed_transitions": allowed_transitions(conn, p.id, t.status_id),
        "permissions": {
            "can_edit": can_edit_ticket(user, role.as_deref(), t.creator_id),
            "can_transition": can_transition_ticket(user, role.as_deref()),
        },
    }))
}

#[derive(Deserialize)]
struct ListMyTicketsArgs {
    limit: Option<usize>,
}

fn list_my_tickets(conn: &Connection, user: &AuthenticatedUser, args: ListMyTicketsArgs) -> ToolResult {
    // Only include projects the user can still access (membership may have been removed).
    let mut access: HashMap<i64, bool> = HashMap::new();
    let tickets: Vec<Value> = ticket::list_for_user(conn, user.id)
        .into_iter()
        .filter(|(t, _)| {
            *access
                .entry(t.project_id)
                .or_insert_with(|| user.is_admin() || project::is_member(conn, t.project_id, user.id))
        })
        .take(clamp_limit(args.limit))
        .map(|(t, project_name)| {
            let mut v = ticket_summary(&t);
            v["project_name"] = json!(project_name);
            v
        })
        .collect();
    Ok(json!({ "tickets": tickets }))
}

// ---------------------------------------------------------------------------
// Write tools
// ---------------------------------------------------------------------------

fn validate_title(title: &str) -> Result<String, ToolError> {
    let title = title.trim();
    if title.is_empty() {
        return fail("title must not be empty");
    }
    if title.chars().count() > MAX_TITLE_LEN {
        return fail(format!("title must be at most {MAX_TITLE_LEN} characters"));
    }
    Ok(title.to_string())
}

fn validate_date(field: &str, value: &str) -> Result<String, ToolError> {
    chrono::NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .map_err(|_| ToolError(format!("{field} must be a date in YYYY-MM-DD format, got '{value}'")))
}

fn validate_assignee(conn: &Connection, project_id: i64, assignee_id: i64) -> Result<(), ToolError> {
    if project::is_member(conn, project_id, assignee_id) {
        Ok(())
    } else {
        fail(format!(
            "User {assignee_id} is not a member of this project; get_project lists valid assignees"
        ))
    }
}

/// Converts a JSON argument into the string stored for a custom field.
fn field_value_to_string(value: &Value) -> Result<String, ToolError> {
    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Null => Ok(String::new()),
        other => fail(format!("Unsupported custom field value: {other}")),
    }
}

/// Validates a single (non-empty) custom field value against its definition.
fn validate_field_value(
    conn: &Connection,
    project_id: i64,
    field: &CustomField,
    value: &str,
) -> Result<String, ToolError> {
    let name = &field.name;
    match field.field_type.as_str() {
        "number" => {
            let n: f64 = value
                .trim()
                .parse()
                .map_err(|_| ToolError(format!("Field '{name}' must be a number")))?;
            if field.num_min.is_some_and(|min| n < min) || field.num_max.is_some_and(|max| n > max) {
                return fail(format!(
                    "Field '{name}' must be between {} and {}",
                    field.num_min.map_or("-inf".into(), |v| v.to_string()),
                    field.num_max.map_or("inf".into(), |v| v.to_string()),
                ));
            }
            Ok(value.trim().to_string())
        }
        "date" => validate_date(&format!("Field '{name}'"), value),
        "user" => {
            let uid: i64 = value
                .trim()
                .parse()
                .map_err(|_| ToolError(format!("Field '{name}' must be a user id")))?;
            validate_assignee(conn, project_id, uid)?;
            Ok(uid.to_string())
        }
        "ticket" => {
            let tid: i64 = value
                .trim()
                .parse()
                .map_err(|_| ToolError(format!("Field '{name}' must be a ticket id")))?;
            ticket::find_by_id(conn, tid)
                .filter(|t| t.project_id == project_id && !t.is_deleted)
                .ok_or_else(|| ToolError(format!("Field '{name}': ticket {tid} not found in this project")))?;
            Ok(tid.to_string())
        }
        _ => Ok(value.to_string()),
    }
}

/// Resolves and validates custom field input (keyed by field name or id).
///
/// `existing` holds current values when updating; on create, unspecified fields
/// fall back to the field's default value, matching the web form. Returns the
/// values to write.
fn resolve_fields(
    conn: &Connection,
    project_id: i64,
    defs: &[CustomField],
    input: &Map<String, Value>,
    existing: Option<&HashMap<i64, String>>,
) -> Result<Vec<(i64, String)>, ToolError> {
    let mut values: Vec<(i64, String)> = Vec::new();
    for (key, raw) in input {
        let def = defs
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(key) || d.id.to_string() == *key)
            .ok_or_else(|| {
                let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
                ToolError(format!("Unknown custom field '{key}'. Fields of this ticket type: {names:?}"))
            })?;
        let value = field_value_to_string(raw)?;
        let value = if value.trim().is_empty() {
            String::new()
        } else {
            validate_field_value(conn, project_id, def, &value)?
        };
        values.retain(|(id, _)| *id != def.id);
        values.push((def.id, value));
    }

    if existing.is_none() {
        for def in defs {
            if !def.default_value.is_empty() && !values.iter().any(|(id, _)| *id == def.id) {
                values.push((def.id, def.default_value.clone()));
            }
        }
    }

    for def in defs.iter().filter(|d| d.is_required) {
        let final_value = values
            .iter()
            .find(|(id, _)| *id == def.id)
            .map(|(_, v)| v.as_str())
            .or_else(|| existing.and_then(|e| e.get(&def.id)).map(String::as_str))
            .unwrap_or("");
        if final_value.trim().is_empty() {
            return fail(format!("Required custom field '{}' is missing", def.name));
        }
    }
    Ok(values)
}

#[derive(Deserialize)]
struct CreateTicketArgs {
    project_id: i64,
    title: String,
    ticket_type_id: i64,
    due_date: String,
    #[serde(default)]
    text: String,
    status_id: Option<i64>,
    assignee_id: Option<i64>,
    #[serde(default)]
    fields: Map<String, Value>,
}

fn create_ticket(conn: &Connection, user: &AuthenticatedUser, args: CreateTicketArgs) -> ToolResult {
    let p = check_project_access(conn, user, args.project_id)?;
    let title = validate_title(&args.title)?;
    let due_date = validate_date("due_date", &args.due_date)?;

    if !project::list_active_ticket_type_ids(conn, p.id).contains(&args.ticket_type_id) {
        return fail(format!(
            "Ticket type {} is not active in this project; get_project lists the active types",
            args.ticket_type_id
        ));
    }

    let active_status_ids = project::list_active_status_ids(conn, p.id);
    let status_id = match args.status_id {
        Some(id) if active_status_ids.contains(&id) => id,
        Some(id) => return fail(format!("Status {id} is not active in this project")),
        // status::list_all is ordered by position, so this is the first workflow step.
        None => status::list_all(conn)
            .into_iter()
            .find(|s| active_status_ids.contains(&s.id))
            .map(|s| s.id)
            .ok_or_else(|| ToolError("This project has no active statuses".into()))?,
    };

    let assignee_id = args.assignee_id.unwrap_or(user.id);
    validate_assignee(conn, p.id, assignee_id)?;

    let defs = ticket_type::list_fields(conn, args.ticket_type_id);
    let field_values = resolve_fields(conn, p.id, &defs, &args.fields, None)?;

    let tx = conn.unchecked_transaction()?;
    let ticket_id = ticket::create(
        &tx,
        p.id,
        args.ticket_type_id,
        status_id,
        user.id,
        assignee_id,
        &title,
        &args.text,
        &due_date,
    )?;
    for (field_id, value) in &field_values {
        ticket::set_field_value(&tx, ticket_id, *field_id, value)?;
    }
    tx.commit()?;

    ticket_detail(conn, user, ticket_id)
}

#[derive(Deserialize)]
struct UpdateTicketArgs {
    ticket_id: i64,
    title: Option<String>,
    text: Option<String>,
    assignee_id: Option<i64>,
    due_date: Option<String>,
    #[serde(default)]
    fields: Map<String, Value>,
}

fn update_ticket(conn: &Connection, user: &AuthenticatedUser, args: UpdateTicketArgs) -> ToolResult {
    let (t, p) = load_ticket(conn, user, args.ticket_id)?;
    let role = project::get_member_role(conn, p.id, user.id);
    if !can_edit_ticket(user, role.as_deref(), t.creator_id) {
        return fail("You are not allowed to edit this ticket");
    }
    if args.title.is_none()
        && args.text.is_none()
        && args.assignee_id.is_none()
        && args.due_date.is_none()
        && args.fields.is_empty()
    {
        return fail("Nothing to update: pass at least one of title, text, assignee_id, due_date, fields");
    }

    let title = match &args.title {
        Some(title) => validate_title(title)?,
        None => t.title.clone(),
    };
    let due_date = match &args.due_date {
        Some(d) => validate_date("due_date", d)?,
        None => t.due_date.clone(),
    };
    let assignee_id = args.assignee_id.unwrap_or(t.assignee_id);
    if args.assignee_id.is_some() {
        validate_assignee(conn, p.id, assignee_id)?;
    }
    let text = args.text.unwrap_or_else(|| t.text.clone());

    let existing: HashMap<i64, String> = ticket::get_field_values(conn, t.id)
        .into_iter()
        .map(|fv| (fv.custom_field_id, fv.value))
        .collect();
    let defs = ticket_type::list_fields(conn, t.ticket_type_id);
    let field_values = resolve_fields(conn, p.id, &defs, &args.fields, Some(&existing))?;

    let tx = conn.unchecked_transaction()?;
    ticket::update(&tx, t.id, &title, &text, assignee_id, &due_date)?;
    for (field_id, value) in &field_values {
        ticket::set_field_value(&tx, t.id, *field_id, value)?;
    }
    tx.commit()?;

    ticket_detail(conn, user, t.id)
}

#[derive(Deserialize)]
struct TransitionArgs {
    ticket_id: i64,
    status_id: i64,
}

fn transition_ticket(conn: &Connection, user: &AuthenticatedUser, args: TransitionArgs) -> ToolResult {
    let (t, p) = load_ticket(conn, user, args.ticket_id)?;
    let role = project::get_member_role(conn, p.id, user.id);
    if !can_transition_ticket(user, role.as_deref()) {
        return fail("You are not allowed to change the status of tickets in this project");
    }
    if t.status_id == args.status_id {
        return fail(format!("Ticket is already in status '{}'", t.status_name));
    }

    let allowed = allowed_transitions(conn, p.id, t.status_id);
    if !allowed.iter().any(|s| s["id"] == args.status_id) {
        return fail(format!(
            "Transition from '{}' to status {} is not allowed. Allowed targets: {}",
            t.status_name,
            args.status_id,
            Value::Array(allowed)
        ));
    }

    ticket::transition_status(conn, t.id, args.status_id)?;
    let updated = ticket::find_by_id(conn, t.id).ok_or_else(|| ToolError("Ticket vanished".into()))?;
    Ok(json!({
        "ticket": ticket_summary(&updated),
        "previous_status": { "id": t.status_id, "name": t.status_name },
    }))
}
