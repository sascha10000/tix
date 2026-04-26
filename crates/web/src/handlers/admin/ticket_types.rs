use actix_web::web;
use actix_web::HttpResponse;
use askama::Template;
use serde::Deserialize;

use ticketsystem_db::DbPool;
use ticketsystem_db::repo::ticket_type;
use crate::errors::AppError;
use crate::middleware::AuthenticatedUser;

#[derive(Template)]
#[template(path = "admin/ticket_types/list.html")]
struct TicketTypesListTemplate {
    user: AuthenticatedUser,
    ticket_types: Vec<TicketTypeView>,
}

struct TicketTypeView {
    id: i64,
    name: String,
    description: String,
    field_count: usize,
}

#[derive(Template)]
#[template(path = "admin/ticket_types/form.html")]
struct TicketTypeFormTemplate {
    user: AuthenticatedUser,
    edit: Option<TicketTypeEditView>,
    fields: Vec<FieldView>,
    available_templates: Vec<TemplateOption>,
    error: Option<String>,
}

struct TemplateOption {
    id: i64,
    name: String,
}

struct TicketTypeEditView {
    id: i64,
    name: String,
    description: String,
}

struct FieldView {
    id: i64,
    name: String,
    field_type: String,
    is_required: bool,
    position: i64,
    num_min: Option<f64>,
    num_max: Option<f64>,
    num_step: Option<f64>,
}

#[derive(Deserialize)]
pub struct TicketTypeForm {
    name: String,
    description: String,
    template_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct FieldForm {
    name: String,
    field_type: String,
    is_required: Option<String>,
    position: String,
    num_min: Option<String>,
    num_max: Option<String>,
    num_step: Option<String>,
}

pub async fn list(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
) -> Result<impl actix_web::Responder, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let types = ticket_type::list_all(&conn);
    let ticket_types: Vec<TicketTypeView> = types
        .into_iter()
        .map(|t| {
            let fields = ticket_type::list_fields(&conn, t.id);
            TicketTypeView {
                id: t.id,
                name: t.name,
                description: t.description,
                field_count: fields.len(),
            }
        })
        .collect();

    Ok(TicketTypesListTemplate {
        user: auth_user,
        ticket_types,
    })
}

pub async fn new_page(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
) -> Result<impl actix_web::Responder, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let available_templates: Vec<TemplateOption> = ticket_type::list_all(&conn)
        .into_iter()
        .map(|t| TemplateOption { id: t.id, name: t.name })
        .collect();

    Ok(TicketTypeFormTemplate {
        user: auth_user,
        edit: None,
        fields: vec![],
        available_templates,
        error: None,
    })
}

pub async fn create(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    form: web::Form<TicketTypeForm>,
) -> Result<HttpResponse, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let id = match ticket_type::create(&conn, &form.name, &form.description) {
        Ok(id) => id,
        Err(e) => {
            let available_templates: Vec<TemplateOption> = ticket_type::list_all(&conn)
                .into_iter()
                .map(|t| TemplateOption { id: t.id, name: t.name })
                .collect();
            let tmpl = TicketTypeFormTemplate {
                user: auth_user,
                edit: None,
                fields: vec![],
                available_templates,
                error: Some(format!(
                    "A ticket type with the name '{}' already exists. ({})",
                    form.name, e
                )),
            };
            return Ok(HttpResponse::Ok()
                .content_type("text/html")
                .body(tmpl.render().map_err(|e| AppError::Internal(e.to_string()))?));
        }
    };

    if let Some(template_id) = form.template_id {
        if template_id > 0 {
            let template_fields = ticket_type::list_fields(&conn, template_id);
            for f in &template_fields {
                ticket_type::add_field(
                    &conn,
                    id,
                    &f.name,
                    &f.field_type,
                    f.is_required,
                    f.position,
                    f.num_min,
                    f.num_max,
                    f.num_step,
                )?;
            }
        }
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/admin/ticket-types/{id}/edit")))
        .finish())
}

pub async fn edit_page(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
) -> Result<impl actix_web::Responder, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let id = path.into_inner();
    let t = ticket_type::find_by_id(&conn, id)
        .ok_or(AppError::NotFound("Ticket type not found".into()))?;
    let fields_raw = ticket_type::list_fields(&conn, id);
    let fields: Vec<FieldView> = fields_raw
        .into_iter()
        .map(|f| FieldView {
            id: f.id,
            name: f.name,
            field_type: f.field_type,
            is_required: f.is_required,
            position: f.position,
            num_min: f.num_min,
            num_max: f.num_max,
            num_step: f.num_step,
        })
        .collect();

    Ok(TicketTypeFormTemplate {
        user: auth_user,
        edit: Some(TicketTypeEditView {
            id: t.id,
            name: t.name,
            description: t.description,
        }),
        fields,
        available_templates: vec![],
        error: None,
    })
}

pub async fn edit_submit(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
    form: web::Form<TicketTypeForm>,
) -> Result<HttpResponse, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let id = path.into_inner();
    ticket_type::update(&conn, id, &form.name, &form.description)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/admin/ticket-types/{id}/edit")))
        .finish())
}

pub async fn add_field(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
    form: web::Form<FieldForm>,
) -> Result<HttpResponse, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let id = path.into_inner();
    let pos: i64 = form.position.parse().unwrap_or(0);
    let num_min: Option<f64> = form.num_min.as_deref().and_then(|s| s.parse().ok());
    let num_max: Option<f64> = form.num_max.as_deref().and_then(|s| s.parse().ok());
    let num_step: Option<f64> = form.num_step.as_deref().and_then(|s| s.parse().ok());

    ticket_type::add_field(
        &conn,
        id,
        &form.name,
        &form.field_type,
        form.is_required.is_some(),
        pos,
        num_min,
        num_max,
        num_step,
    )?;

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/admin/ticket-types/{id}/edit")))
        .finish())
}

pub async fn delete_field(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let (type_id, field_id) = path.into_inner();
    ticket_type::delete_field(&conn, field_id)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/admin/ticket-types/{type_id}/edit")))
        .finish())
}
