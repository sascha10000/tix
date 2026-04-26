use actix_web::web;
use actix_web::HttpResponse;
use askama::Template;
use serde::Deserialize;

use ticketsystem_db::DbPool;
use ticketsystem_db::repo::user;
use ticketsystem_core::i18n::Translations;
use crate::errors::AppError;
use crate::middleware::{AuthenticatedUser, Lang};

#[derive(Template)]
#[template(path = "admin/users/list.html")]
struct UsersListTemplate {
    user: AuthenticatedUser,
    users: Vec<UserView>,
    t: &'static Translations,
}

#[derive(Template)]
#[template(path = "admin/users/edit.html")]
struct UserEditTemplate {
    user: AuthenticatedUser,
    edit_user: UserView,
    t: &'static Translations,
}

struct UserView {
    id: i64,
    username: String,
    email: String,
    is_admin: bool,
    is_manager: bool,
    is_active: bool,
    created_at: String,
}

#[derive(Template)]
#[template(path = "admin/users/new.html")]
struct UserNewTemplate {
    user: AuthenticatedUser,
    error: Option<String>,
    t: &'static Translations,
}

#[derive(Deserialize)]
pub struct UserCreateForm {
    username: String,
    email: String,
    password: String,
    is_admin: Option<String>,
    is_manager: Option<String>,
}

#[derive(Deserialize)]
pub struct UserEditForm {
    username: String,
    email: String,
    is_admin: Option<String>,
    is_manager: Option<String>,
    is_active: Option<String>,
}

pub async fn list(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    Lang(t): Lang,
) -> Result<impl actix_web::Responder, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let users: Vec<UserView> = user::list_all(&conn)
        .into_iter()
        .map(|u| UserView {
            id: u.id,
            username: u.username,
            email: u.email,
            is_admin: u.is_admin,
            is_manager: u.is_manager,
            is_active: u.is_active,
            created_at: u.created_at,
        })
        .collect();

    Ok(UsersListTemplate {
        user: auth_user,
        users,
        t,
    })
}

pub async fn edit_page(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
    Lang(t): Lang,
) -> Result<impl actix_web::Responder, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let id = path.into_inner();
    let u = user::find_by_id(&conn, id).ok_or(AppError::NotFound("User not found".into()))?;

    Ok(UserEditTemplate {
        user: auth_user,
        edit_user: UserView {
            id: u.id,
            username: u.username,
            email: u.email,
            is_admin: u.is_admin,
            is_manager: u.is_manager,
            is_active: u.is_active,
            created_at: u.created_at,
        },
        t,
    })
}

pub async fn edit_submit(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    path: web::Path<i64>,
    form: web::Form<UserEditForm>,
) -> Result<HttpResponse, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let id = path.into_inner();

    user::update_profile(&conn, id, &form.username, &form.email)?;
    user::set_admin(&conn, id, form.is_admin.is_some())?;
    user::set_manager(&conn, id, form.is_manager.is_some())?;
    user::set_active(&conn, id, form.is_active.is_some())?;

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/admin/users"))
        .finish())
}

pub async fn new_page(auth_user: AuthenticatedUser, Lang(t): Lang) -> Result<impl actix_web::Responder, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    Ok(UserNewTemplate {
        user: auth_user,
        error: None,
        t,
    })
}

pub async fn create(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    form: web::Form<UserCreateForm>,
    Lang(t): Lang,
) -> Result<HttpResponse, AppError> {
    if !auth_user.is_admin() {
        return Err(AppError::Forbidden);
    }
    let conn = pool.get()?;
    let hash = ticketsystem_auth::hash_password(&form.password)
        .map_err(|_| AppError::Internal("Failed to hash password".into()))?;

    match user::create(&conn, &form.username, &form.email, &hash, form.is_admin.is_some(), form.is_manager.is_some()) {
        Ok(_) => Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/admin/users"))
            .finish()),
        Err(e) => {
            let tmpl = UserNewTemplate {
                user: auth_user,
                error: Some(format!("Failed to create user: {e}")),
                t,
            };
            Ok(HttpResponse::Ok()
                .content_type("text/html")
                .body(tmpl.render().map_err(|e| AppError::Internal(e.to_string()))?))
        }
    }
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

    if id == auth_user.id {
        return Err(AppError::BadRequest("Cannot delete yourself".into()));
    }

    user::set_active(&conn, id, false)?;

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/admin/users"))
        .finish())
}
