use actix_web::web;
use actix_web::HttpResponse;
use askama::Template;
use serde::Deserialize;

use ticketsystem_db::DbPool;
use ticketsystem_db::repo::{project, status, ticket_type, user};
use crate::errors::AppError;
use crate::middleware::AuthenticatedUser;

#[derive(Template)]
#[template(path = "projects/list.html")]
struct ProjectsListTemplate {
    user: AuthenticatedUser,
    projects: Vec<ProjectView>,
    can_create: bool,
}

struct ProjectView {
    id: i64,
    name: String,
    description: String,
}

#[derive(Template)]
#[template(path = "projects/form.html")]
struct ProjectFormTemplate {
    user: AuthenticatedUser,
    edit: Option<ProjectView>,
}

#[derive(Template)]
#[template(path = "projects/detail.html")]
struct ProjectDetailTemplate {
    user: AuthenticatedUser,
    project: ProjectView,
    members: Vec<MemberView>,
    all_users: Vec<MemberView>,
    statuses: Vec<StatusToggleView>,
    ticket_types: Vec<TypeToggleView>,
    can_manage: bool,
    can_delete: bool,
}

struct MemberView {
    id: i64,
    username: String,
    role: String,
}

struct StatusToggleView {
    id: i64,
    name: String,
    color: String,
    active: bool,
}

struct TypeToggleView {
    id: i64,
    name: String,
    active: bool,
}

#[derive(Deserialize)]
pub struct ProjectForm {
    name: String,
    description: String,
}

#[derive(Deserialize)]
pub struct MemberForm {
    user_id: i64,
    role: String,
}

#[derive(Deserialize)]
pub struct ToggleStatusForm {
    status_id: i64,
}

#[derive(Deserialize)]
pub struct ToggleTypeForm {
    ticket_type_id: i64,
}

fn can_create_project(user: &AuthenticatedUser) -> bool {
    user.is_admin() || user.is_manager()
}

fn can_manage_project(conn: &ticketsystem_db::rusqlite::Connection, user: &AuthenticatedUser, project_id: i64) -> bool {
    user.is_admin() || project::get_member_role(conn, project_id, user.id).as_deref() == Some("manager")
}

fn can_delete_project(conn: &ticketsystem_db::rusqlite::Connection, user: &AuthenticatedUser, project_id: i64) -> bool {
    if user.is_admin() {
        return true;
    }
    if !user.is_manager() {
        return false;
    }
    project::find_by_id(conn, project_id)
        .map(|p| p.created_by == Some(user.id))
        .unwrap_or(false)
}

pub async fn list(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
) -> Result<impl actix_web::Responder, AppError> {
    let conn = pool.get()?;
    let projects_raw = if auth_user.is_admin() {
        project::list_all(&conn)
    } else {
        project::list_for_user(&conn, auth_user.id)
    };

    let can_create = can_create_project(&auth_user);

    let projects: Vec<ProjectView> = projects_raw
        .into_iter()
        .map(|p| ProjectView {
            id: p.id,
            name: p.name,
            description: p.description,
        })
        .collect();

    Ok(ProjectsListTemplate {
        user: auth_user,
        projects,
        can_create,
    })
}

pub async fn new_page(
    auth_user: AuthenticatedUser,
) -> Result<impl actix_web::Responder, AppError> {
    if !can_create_project(&auth_user) {
        return Err(AppError::Forbidden);
    }
    Ok(ProjectFormTemplate {
        user: auth_user,
        edit: None,
    })
}

pub async fn create(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    form: web::Form<ProjectForm>,
) -> Result<HttpResponse, AppError> {
    if !can_create_project(&auth_user) {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    project::create(&conn, &form.name, &form.description, auth_user.id)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/projects"))
        .finish())
}

pub async fn detail(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
) -> Result<impl actix_web::Responder, AppError> {
    let conn = pool.get()?;
    let id = path.into_inner();
    let p = project::find_by_id(&conn, id).ok_or(AppError::NotFound("Project not found".into()))?;

    if !auth_user.is_admin() && !project::is_member(&conn, id, auth_user.id) {
        return Err(AppError::Forbidden);
    }

    let can_manage = can_manage_project(&conn, &auth_user, id);
    let can_delete = can_delete_project(&conn, &auth_user, id);

    let members: Vec<MemberView> = project::list_members(&conn, id)
        .into_iter()
        .map(|m| MemberView {
            id: m.user_id,
            username: m.username,
            role: m.role,
        })
        .collect();

    let all_users: Vec<MemberView> = user::list_all(&conn)
        .into_iter()
        .map(|u| MemberView {
            id: u.id,
            username: u.username,
            role: String::new(),
        })
        .collect();

    let active_status_ids = project::list_active_status_ids(&conn, id);
    let statuses: Vec<StatusToggleView> = status::list_all(&conn)
        .into_iter()
        .map(|s| StatusToggleView {
            active: active_status_ids.contains(&s.id),
            id: s.id,
            name: s.name,
            color: s.color,
        })
        .collect();

    let active_type_ids = project::list_active_ticket_type_ids(&conn, id);
    let ticket_types: Vec<TypeToggleView> = ticket_type::list_all(&conn)
        .into_iter()
        .map(|t| TypeToggleView {
            active: active_type_ids.contains(&t.id),
            id: t.id,
            name: t.name,
        })
        .collect();

    Ok(ProjectDetailTemplate {
        user: auth_user,
        project: ProjectView {
            id: p.id,
            name: p.name,
            description: p.description,
        },
        members,
        all_users,
        statuses,
        ticket_types,
        can_manage,
        can_delete,
    })
}

pub async fn edit_page(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
) -> Result<impl actix_web::Responder, AppError> {
    let conn = pool.get()?;
    let id = path.into_inner();
    if !can_manage_project(&conn, &auth_user, id) {
        return Err(AppError::Forbidden);
    }
    let p = project::find_by_id(&conn, id).ok_or(AppError::NotFound("Project not found".into()))?;
    Ok(ProjectFormTemplate {
        user: auth_user,
        edit: Some(ProjectView {
            id: p.id,
            name: p.name,
            description: p.description,
        }),
    })
}

pub async fn edit_submit(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
    form: web::Form<ProjectForm>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let id = path.into_inner();
    if !can_manage_project(&conn, &auth_user, id) {
        return Err(AppError::Forbidden);
    }
    project::update(&conn, id, &form.name, &form.description)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/projects/{id}")))
        .finish())
}

pub async fn delete_project(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let id = path.into_inner();
    if !can_delete_project(&conn, &auth_user, id) {
        return Err(AppError::Forbidden);
    }
    project::delete(&conn, id)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/projects"))
        .finish())
}

pub async fn add_member(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
    form: web::Form<MemberForm>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let id = path.into_inner();
    if !can_manage_project(&conn, &auth_user, id) {
        return Err(AppError::Forbidden);
    }
    project::add_member(&conn, id, form.user_id, &form.role)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/projects/{id}")))
        .finish())
}

pub async fn remove_member(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let (project_id, user_id) = path.into_inner();
    if !can_manage_project(&conn, &auth_user, project_id) {
        return Err(AppError::Forbidden);
    }
    project::remove_member(&conn, project_id, user_id)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/projects/{project_id}")))
        .finish())
}

pub async fn toggle_status(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
    form: web::Form<ToggleStatusForm>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let id = path.into_inner();
    if !can_manage_project(&conn, &auth_user, id) {
        return Err(AppError::Forbidden);
    }
    project::toggle_status(&conn, id, form.status_id)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/projects/{id}")))
        .finish())
}

pub async fn toggle_ticket_type(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
    form: web::Form<ToggleTypeForm>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let id = path.into_inner();
    if !can_manage_project(&conn, &auth_user, id) {
        return Err(AppError::Forbidden);
    }
    project::toggle_ticket_type(&conn, id, form.ticket_type_id)?;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/projects/{id}")))
        .finish())
}
