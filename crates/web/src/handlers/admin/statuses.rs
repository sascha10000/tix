use actix_web::web;
use actix_web::HttpResponse;
use askama::Template;
use serde::Deserialize;

use ticketsystem_db::DbPool;
use ticketsystem_db::repo::status;
use crate::errors::AppError;
use crate::middleware::AuthenticatedUser;

#[derive(Template)]
#[template(path = "admin/statuses/list.html")]
struct StatusesListTemplate {
    user: AuthenticatedUser,
    statuses: Vec<StatusView>,
    workflow_matrix: Vec<Vec<bool>>,
}

struct StatusView {
    id: i64,
    name: String,
    color: String,
    position: i64,
}

#[derive(Template)]
#[template(path = "admin/statuses/form.html")]
struct StatusFormTemplate {
    user: AuthenticatedUser,
    edit: Option<StatusView>,
}

#[derive(Deserialize)]
pub struct StatusForm {
    name: String,
    color: String,
    position: String,
}

#[derive(Deserialize)]
pub struct WorkflowToggleForm {
    from_id: i64,
    to_id: i64,
}

pub async fn list(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
) -> Result<impl actix_web::Responder, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let statuses_raw = status::list_all(&conn);
    let workflows = status::list_workflows(&conn);

    let statuses: Vec<StatusView> = statuses_raw
        .iter()
        .map(|s| StatusView {
            id: s.id,
            name: s.name.clone(),
            color: s.color.clone(),
            position: s.position,
        })
        .collect();

    let n = statuses.len();
    let mut matrix = vec![vec![false; n]; n];
    for w in &workflows {
        if let (Some(fi), Some(ti)) = (
            statuses.iter().position(|s| s.id == w.from_status_id),
            statuses.iter().position(|s| s.id == w.to_status_id),
        ) {
            matrix[fi][ti] = true;
        }
    }

    Ok(StatusesListTemplate {
        user: auth_user,
        statuses,
        workflow_matrix: matrix,
    })
}

pub async fn new_page(auth_user: AuthenticatedUser) -> Result<impl actix_web::Responder, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    Ok(StatusFormTemplate {
        user: auth_user,
        edit: None,
    })
}

pub async fn create(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    form: web::Form<StatusForm>,
) -> Result<HttpResponse, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let pos: i64 = form.position.parse().unwrap_or(0);
    status::create(&conn, &form.name, &form.color, pos)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/admin/statuses"))
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
    let s = status::find_by_id(&conn, id).ok_or(AppError::NotFound("Status not found".into()))?;
    Ok(StatusFormTemplate {
        user: auth_user,
        edit: Some(StatusView {
            id: s.id,
            name: s.name,
            color: s.color,
            position: s.position,
        }),
    })
}

pub async fn edit_submit(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
    form: web::Form<StatusForm>,
) -> Result<HttpResponse, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let id = path.into_inner();
    let pos: i64 = form.position.parse().unwrap_or(0);
    status::update(&conn, id, &form.name, &form.color, pos)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/admin/statuses"))
        .finish())
}

pub async fn delete(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let id = path.into_inner();
    status::delete(&conn, id)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/admin/statuses"))
        .finish())
}

pub async fn toggle_workflow(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    form: web::Form<WorkflowToggleForm>,
) -> Result<HttpResponse, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    status::toggle_workflow(&conn, form.from_id, form.to_id)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/admin/statuses"))
        .finish())
}
