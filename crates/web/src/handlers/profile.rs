use actix_web::web;
use actix_web::HttpResponse;
use askama::Template;
use serde::Deserialize;

use ticketsystem_db::DbPool;
use ticketsystem_db::repo::user;
use crate::errors::AppError;
use crate::middleware::AuthenticatedUser;

#[derive(Template)]
#[template(path = "profile.html")]
struct ProfileTemplate {
    user: AuthenticatedUser,
    profile: ProfileView,
    success: Option<String>,
    error: Option<String>,
}

struct ProfileView {
    username: String,
    email: String,
}

#[derive(Deserialize)]
pub struct ProfileForm {
    username: String,
    email: String,
}

#[derive(Deserialize)]
pub struct PasswordForm {
    current_password: String,
    new_password: String,
}

pub async fn page(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
) -> Result<impl actix_web::Responder, AppError> {
    let conn = pool.get()?;
    let u = user::find_by_id(&conn, auth_user.id)
        .ok_or(AppError::NotFound("User not found".into()))?;

    Ok(ProfileTemplate {
        user: auth_user,
        profile: ProfileView {
            username: u.username,
            email: u.email,
        },
        success: None,
        error: None,
    })
}

pub async fn update(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    form: web::Form<ProfileForm>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;

    match user::update_profile(&conn, auth_user.id, &form.username, &form.email) {
        Ok(_) => {
            let tmpl = ProfileTemplate {
                user: auth_user,
                profile: ProfileView {
                    username: form.username.clone(),
                    email: form.email.clone(),
                },
                success: Some("Profile updated.".into()),
                error: None,
            };
            Ok(HttpResponse::Ok()
                .content_type("text/html")
                .body(tmpl.render().map_err(|e| AppError::Internal(e.to_string()))?))
        }
        Err(e) => {
            let tmpl = ProfileTemplate {
                user: auth_user,
                profile: ProfileView {
                    username: form.username.clone(),
                    email: form.email.clone(),
                },
                success: None,
                error: Some(format!("Failed to update: {e}")),
            };
            Ok(HttpResponse::Ok()
                .content_type("text/html")
                .body(tmpl.render().map_err(|e| AppError::Internal(e.to_string()))?))
        }
    }
}

pub async fn change_password(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    form: web::Form<PasswordForm>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let u = user::find_by_id(&conn, auth_user.id)
        .ok_or(AppError::NotFound("User not found".into()))?;

    if !ticketsystem_auth::verify_password(&form.current_password, &u.password_hash) {
        let tmpl = ProfileTemplate {
            user: auth_user,
            profile: ProfileView {
                username: u.username,
                email: u.email,
            },
            success: None,
            error: Some("Current password is incorrect.".into()),
        };
        return Ok(HttpResponse::Ok()
            .content_type("text/html")
            .body(tmpl.render().map_err(|e| AppError::Internal(e.to_string()))?));
    }

    let new_hash = ticketsystem_auth::hash_password(&form.new_password)
        .map_err(|_| AppError::Internal("Failed to hash password".into()))?;
    user::update_password(&conn, auth_user.id, &new_hash)?;

    let tmpl = ProfileTemplate {
        user: auth_user,
        profile: ProfileView {
            username: u.username,
            email: u.email,
        },
        success: Some("Password changed.".into()),
        error: None,
    };
    Ok(HttpResponse::Ok()
        .content_type("text/html")
        .body(tmpl.render().map_err(|e| AppError::Internal(e.to_string()))?))
}
