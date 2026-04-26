use actix_web::web;
use actix_web::HttpResponse;
use askama::Template;
use serde::Deserialize;
use std::collections::HashMap;

use ticketsystem_db::DbPool;
use ticketsystem_db::repo::{project, status, ticket, ticket_type, user};
use ticketsystem_core::i18n::Translations;
use crate::errors::AppError;
use crate::middleware::{AuthenticatedUser, Lang};

#[derive(Template)]
#[template(path = "tickets/list.html")]
struct TicketsListTemplate {
    t: &'static Translations,
    user: AuthenticatedUser,
    project_id: i64,
    project_name: String,
    tickets: Vec<TicketView>,
}

#[allow(dead_code)]
struct TicketView {
    id: i64,
    title: String,
    status_name: String,
    status_color: String,
    type_name: String,
    creator_name: String,
    assignee_name: String,
    due_date: String,
    created_at: String,
    updated_at: String,
}

#[derive(Template)]
#[template(path = "tickets/form.html")]
struct TicketFormTemplate {
    t: &'static Translations,
    user: AuthenticatedUser,
    project_id: i64,
    project_name: String,
    edit: Option<TicketEditView>,
    ticket_types: Vec<TypeOption>,
    statuses: Vec<StatusOption>,
    fields: Vec<FieldInput>,
    selected_type_id: Option<i64>,
    project_members: Vec<SelectOption>,
    project_tickets: Vec<SelectOption>,
}

struct SelectOption {
    id: String,
    label: String,
}

struct TicketEditView {
    id: i64,
    title: String,
    text: String,
    ticket_type_id: i64,
    status_id: i64,
    assignee_id: i64,
    due_date: String,
}

struct TypeOption {
    id: i64,
    name: String,
}

struct StatusOption {
    id: i64,
    name: String,
}

#[allow(dead_code)]
struct FieldInput {
    id: i64,
    name: String,
    field_type: String,
    is_required: bool,
    value: String,
    num_min: Option<f64>,
    num_max: Option<f64>,
    num_step: Option<f64>,
    placeholder: String,
    default_value: String,
}

#[derive(Template)]
#[template(path = "tickets/detail.html")]
struct TicketDetailTemplate {
    t: &'static Translations,
    user: AuthenticatedUser,
    project_id: i64,
    project_name: String,
    ticket: TicketDetailView,
    field_values: Vec<FieldDisplay>,
    transitions: Vec<StatusOption>,
    can_edit: bool,
    can_transition: bool,
}

#[allow(dead_code)]
struct TicketDetailView {
    id: i64,
    title: String,
    text: String,
    status_name: String,
    status_color: String,
    status_id: i64,
    type_name: String,
    creator_name: String,
    creator_id: i64,
    assignee_name: String,
    due_date: String,
    created_at: String,
    updated_at: String,
}

struct FieldDisplay {
    name: String,
    value: String,
}

#[derive(Deserialize)]
pub struct TransitionForm {
    status_id: i64,
}

#[derive(Deserialize)]
pub struct TypeSelectQuery {
    type_id: Option<i64>,
}

fn load_project_members(conn: &ticketsystem_db::rusqlite::Connection, project_id: i64) -> Vec<SelectOption> {
    project::list_member_ids(conn, project_id)
        .into_iter()
        .filter_map(|uid| {
            user::find_by_id(conn, uid).map(|u| SelectOption {
                id: u.id.to_string(),
                label: u.username,
            })
        })
        .collect()
}

fn load_project_tickets(conn: &ticketsystem_db::rusqlite::Connection, project_id: i64) -> Vec<SelectOption> {
    ticket::list_for_project(conn, project_id)
        .into_iter()
        .map(|t| SelectOption {
            id: t.id.to_string(),
            label: format!("#{} - {}", t.id, t.title),
        })
        .collect()
}

fn check_project_access(
    conn: &ticketsystem_db::rusqlite::Connection,
    user: &AuthenticatedUser,
    project_id: i64,
) -> Result<ticketsystem_core::models::project::Project, AppError> {
    let p = project::find_by_id(conn, project_id)
        .ok_or(AppError::NotFound("Project not found".into()))?;
    if !user.is_admin() && !project::is_member(conn, project_id, user.id) {
        return Err(AppError::Forbidden);
    }
    Ok(p)
}

fn can_edit_ticket(user: &AuthenticatedUser, project_role: Option<&str>, ticket_creator_id: i64) -> bool {
    user.is_admin()
        || matches!(project_role, Some("manager" | "member"))
        || user.id == ticket_creator_id
}

fn can_transition_ticket(user: &AuthenticatedUser, project_role: Option<&str>) -> bool {
    user.is_admin() || matches!(project_role, Some("manager" | "member"))
}

pub async fn list(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
    Lang(t): Lang,
) -> Result<impl actix_web::Responder, AppError> {
    let conn = pool.get()?;
    let project_id = path.into_inner();
    let p = check_project_access(&conn, &auth_user, project_id)?;

    let tickets: Vec<TicketView> = ticket::list_for_project(&conn, project_id)
        .into_iter()
        .map(|t| TicketView {
            id: t.id,
            title: t.title,
            status_name: t.status_name,
            status_color: t.status_color,
            type_name: t.type_name,
            creator_name: t.creator_name,
            assignee_name: t.assignee_name,
            due_date: t.due_date,
            created_at: t.created_at,
            updated_at: t.updated_at,
        })
        .collect();

    Ok(TicketsListTemplate {
        t,
        user: auth_user,
        project_id,
        project_name: p.name,
        tickets,
    })
}

pub async fn new_page(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
    query: web::Query<TypeSelectQuery>,
    Lang(t): Lang,
) -> Result<impl actix_web::Responder, AppError> {
    let conn = pool.get()?;
    let project_id = path.into_inner();
    let p = check_project_access(&conn, &auth_user, project_id)?;

    let active_type_ids = project::list_active_ticket_type_ids(&conn, project_id);
    let ticket_types: Vec<TypeOption> = ticket_type::list_all(&conn)
        .into_iter()
        .filter(|t| active_type_ids.contains(&t.id))
        .map(|t| TypeOption {
            id: t.id,
            name: t.name,
        })
        .collect();

    let active_status_ids = project::list_active_status_ids(&conn, project_id);
    let statuses: Vec<StatusOption> = status::list_all(&conn)
        .into_iter()
        .filter(|s| active_status_ids.contains(&s.id))
        .map(|s| StatusOption {
            id: s.id,
            name: s.name,
        })
        .collect();

    let fields = if let Some(type_id) = query.type_id {
        ticket_type::list_fields(&conn, type_id)
            .into_iter()
            .map(|f| {
                let value = if f.default_value.is_empty() {
                    String::new()
                } else {
                    f.default_value.clone()
                };
                FieldInput {
                    id: f.id,
                    name: f.name,
                    field_type: f.field_type,
                    is_required: f.is_required,
                    value,
                    num_min: f.num_min,
                    num_max: f.num_max,
                    num_step: f.num_step,
                    placeholder: f.placeholder,
                    default_value: f.default_value,
                }
            })
            .collect()
    } else {
        vec![]
    };

    let project_members = load_project_members(&conn, project_id);
    let project_tickets = load_project_tickets(&conn, project_id);

    Ok(TicketFormTemplate {
        t,
        user: auth_user,
        project_id,
        project_name: p.name,
        edit: None,
        ticket_types,
        statuses,
        fields,
        selected_type_id: query.type_id,
        project_members,
        project_tickets,
    })
}

pub async fn create(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
    form: web::Form<HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let project_id = path.into_inner();
    check_project_access(&conn, &auth_user, project_id)?;

    let params = form.into_inner();

    let title = params.get("title").cloned().unwrap_or_default();
    let text = params.get("text").cloned().unwrap_or_default();
    let ticket_type_id: i64 = params
        .get("ticket_type_id")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::BadRequest("Missing or invalid ticket type".into()))?;
    let status_id: i64 = params
        .get("status_id")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::BadRequest("Missing or invalid status".into()))?;
    let assignee_id: i64 = params
        .get("assignee_id")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::BadRequest("Missing or invalid assignee".into()))?;
    let due_date = params
        .get("due_date")
        .filter(|s| !s.is_empty())
        .cloned()
        .ok_or_else(|| AppError::BadRequest("Missing due date".into()))?;

    let ticket_id = ticket::create(
        &conn,
        project_id,
        ticket_type_id,
        status_id,
        auth_user.id,
        assignee_id,
        &title,
        &text,
        &due_date,
    )?;

    let fields = ticket_type::list_fields(&conn, ticket_type_id);
    for field in &fields {
        let key = format!("field_{}", field.id);
        if let Some(value) = params.get(&key) {
            ticket::set_field_value(&conn, ticket_id, field.id, value)?;
        }
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/projects/{project_id}/tickets/{ticket_id}")))
        .finish())
}

pub async fn detail(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<(i64, i64)>,
    Lang(tr): Lang,
) -> Result<impl actix_web::Responder, AppError> {
    let conn = pool.get()?;
    let (project_id, ticket_id) = path.into_inner();
    let p = check_project_access(&conn, &auth_user, project_id)?;

    let t = ticket::find_by_id(&conn, ticket_id)
        .ok_or(AppError::NotFound("Ticket not found".into()))?;
    if t.is_deleted && !auth_user.is_admin() {
        return Err(AppError::NotFound("Ticket not found".into()));
    }

    let field_values: Vec<FieldDisplay> = ticket::get_field_values(&conn, ticket_id)
        .into_iter()
        .filter(|fv| !fv.value.is_empty())
        .map(|fv| {
            let display_value = match fv.field_type.as_str() {
                "user" => {
                    fv.value.parse::<i64>().ok()
                        .and_then(|uid| user::find_by_id(&conn, uid))
                        .map(|u| u.username)
                        .unwrap_or(fv.value)
                }
                "ticket" => {
                    fv.value.parse::<i64>().ok()
                        .and_then(|tid| ticket::find_by_id(&conn, tid))
                        .map(|t| format!("#{} - {}", t.id, t.title))
                        .unwrap_or(fv.value)
                }
                _ => fv.value,
            };
            FieldDisplay {
                name: fv.field_name,
                value: display_value,
            }
        })
        .collect();

    let active_status_ids = project::list_active_status_ids(&conn, project_id);
    let all_statuses = status::list_all(&conn);
    let transitions: Vec<StatusOption> = all_statuses
        .into_iter()
        .filter(|s| {
            s.id != t.status_id
                && active_status_ids.contains(&s.id)
                && status::has_transition(&conn, t.status_id, s.id)
        })
        .map(|s| StatusOption {
            id: s.id,
            name: s.name,
        })
        .collect();

    let project_role = project::get_member_role(&conn, project_id, auth_user.id);
    let editable = can_edit_ticket(&auth_user, project_role.as_deref(), t.creator_id);
    let can_trans = can_transition_ticket(&auth_user, project_role.as_deref());

    Ok(TicketDetailTemplate {
        t: tr,
        user: auth_user,
        project_id,
        project_name: p.name,
        ticket: TicketDetailView {
            id: t.id,
            title: t.title,
            text: t.text,
            status_name: t.status_name,
            status_color: t.status_color,
            status_id: t.status_id,
            type_name: t.type_name,
            creator_name: t.creator_name,
            creator_id: t.creator_id,
            assignee_name: t.assignee_name,
            due_date: t.due_date,
            created_at: t.created_at,
            updated_at: t.updated_at,
        },
        field_values,
        transitions,
        can_edit: editable,
        can_transition: can_trans,
    })
}

pub async fn edit_page(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<(i64, i64)>,
    Lang(tr): Lang,
) -> Result<impl actix_web::Responder, AppError> {
    let conn = pool.get()?;
    let (project_id, ticket_id) = path.into_inner();
    let p = check_project_access(&conn, &auth_user, project_id)?;
    let t = ticket::find_by_id(&conn, ticket_id)
        .ok_or(AppError::NotFound("Ticket not found".into()))?;

    let project_role = project::get_member_role(&conn, project_id, auth_user.id);
    if !can_edit_ticket(&auth_user, project_role.as_deref(), t.creator_id) {
        return Err(AppError::Forbidden);
    }

    let active_type_ids = project::list_active_ticket_type_ids(&conn, project_id);
    let ticket_types: Vec<TypeOption> = ticket_type::list_all(&conn)
        .into_iter()
        .filter(|tt| active_type_ids.contains(&tt.id))
        .map(|tt| TypeOption {
            id: tt.id,
            name: tt.name,
        })
        .collect();

    let active_status_ids = project::list_active_status_ids(&conn, project_id);
    let statuses: Vec<StatusOption> = status::list_all(&conn)
        .into_iter()
        .filter(|s| active_status_ids.contains(&s.id))
        .map(|s| StatusOption {
            id: s.id,
            name: s.name,
        })
        .collect();

    let field_values = ticket::get_field_values(&conn, ticket_id);
    let fields: Vec<FieldInput> = field_values
        .into_iter()
        .map(|fv| FieldInput {
            id: fv.custom_field_id,
            name: fv.field_name,
            field_type: fv.field_type,
            is_required: fv.is_required,
            value: fv.value,
            num_min: fv.num_min,
            num_max: fv.num_max,
            num_step: fv.num_step,
            placeholder: fv.placeholder,
            default_value: fv.default_value,
        })
        .collect();

    Ok(TicketFormTemplate {
        t: tr,
        user: auth_user,
        project_id,
        project_name: p.name,
        edit: Some(TicketEditView {
            id: t.id,
            title: t.title,
            text: t.text,
            ticket_type_id: t.ticket_type_id,
            status_id: t.status_id,
            assignee_id: t.assignee_id,
            due_date: t.due_date,
        }),
        ticket_types,
        statuses,
        fields,
        selected_type_id: Some(t.ticket_type_id),
        project_members: load_project_members(&conn, project_id),
        project_tickets: load_project_tickets(&conn, project_id),
    })
}

pub async fn edit_submit(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<(i64, i64)>,
    form: web::Form<HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let (project_id, ticket_id) = path.into_inner();
    check_project_access(&conn, &auth_user, project_id)?;

    let t = ticket::find_by_id(&conn, ticket_id)
        .ok_or(AppError::NotFound("Ticket not found".into()))?;
    let project_role = project::get_member_role(&conn, project_id, auth_user.id);
    if !can_edit_ticket(&auth_user, project_role.as_deref(), t.creator_id) {
        return Err(AppError::Forbidden);
    }

    let params = form.into_inner();

    let title = params.get("title").cloned().unwrap_or_default();
    let text = params.get("text").cloned().unwrap_or_default();
    let assignee_id: i64 = params
        .get("assignee_id")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::BadRequest("Missing or invalid assignee".into()))?;
    let due_date = params
        .get("due_date")
        .filter(|s| !s.is_empty())
        .cloned()
        .ok_or_else(|| AppError::BadRequest("Missing due date".into()))?;

    ticket::update(&conn, ticket_id, &title, &text, assignee_id, &due_date)?;

    let fields = ticket_type::list_fields(&conn, t.ticket_type_id);
    for field in &fields {
        let key = format!("field_{}", field.id);
        if let Some(value) = params.get(&key) {
            ticket::set_field_value(&conn, ticket_id, field.id, value)?;
        }
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/projects/{project_id}/tickets/{ticket_id}")))
        .finish())
}

pub async fn transition(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<(i64, i64)>,
    form: web::Form<TransitionForm>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let (project_id, ticket_id) = path.into_inner();
    check_project_access(&conn, &auth_user, project_id)?;

    let project_role = project::get_member_role(&conn, project_id, auth_user.id);
    if !can_transition_ticket(&auth_user, project_role.as_deref()) {
        return Err(AppError::Forbidden);
    }

    let t = ticket::find_by_id(&conn, ticket_id)
        .ok_or(AppError::NotFound("Ticket not found".into()))?;

    if !status::has_transition(&conn, t.status_id, form.status_id) {
        return Err(AppError::BadRequest("Invalid status transition".into()));
    }

    let active = project::list_active_status_ids(&conn, project_id);
    if !active.contains(&form.status_id) {
        return Err(AppError::BadRequest("Target status not active in project".into()));
    }

    ticket::transition_status(&conn, ticket_id, form.status_id)?;

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/projects/{project_id}/tickets/{ticket_id}")))
        .finish())
}

pub async fn delete(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let (project_id, ticket_id) = path.into_inner();
    check_project_access(&conn, &auth_user, project_id)?;

    let t = ticket::find_by_id(&conn, ticket_id)
        .ok_or(AppError::NotFound("Ticket not found".into()))?;
    let project_role = project::get_member_role(&conn, project_id, auth_user.id);
    if !can_edit_ticket(&auth_user, project_role.as_deref(), t.creator_id) {
        return Err(AppError::Forbidden);
    }

    ticket::soft_delete(&conn, ticket_id)?;

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/projects/{project_id}/tickets")))
        .finish())
}
