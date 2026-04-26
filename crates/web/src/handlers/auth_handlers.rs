use actix_web::{web, HttpRequest, HttpResponse};
use askama::Template;
use serde::Deserialize;

use ticketsystem_db::DbPool;
use ticketsystem_db::repo::user;

#[derive(Template)]
#[template(path = "auth/login.html")]
struct LoginTemplate {
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/register.html")]
struct RegisterTemplate {
    error: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

#[derive(Deserialize)]
pub struct RegisterForm {
    username: String,
    email: String,
    password: String,
}

pub async fn login_page() -> impl actix_web::Responder {
    LoginTemplate { error: None }
}

pub async fn login_submit(
    pool: web::Data<DbPool>,
    session_hours: web::Data<i64>,
    form: web::Form<LoginForm>,
) -> HttpResponse {
    let conn = pool.get().unwrap();
    let u = user::find_by_username(&conn, &form.username);

    match u {
        Some(u) if u.is_active && ticketsystem_auth::verify_password(&form.password, &u.password_hash) => {
            let session_id = user::create_session(&conn, u.id, **session_hours).unwrap();
            HttpResponse::SeeOther()
                .insert_header(("Location", "/"))
                .cookie(
                    actix_web::cookie::Cookie::build("session_id", session_id)
                        .path("/")
                        .http_only(true)
                        .same_site(actix_web::cookie::SameSite::Strict)
                        .finish(),
                )
                .finish()
        }
        _ => {
            let tmpl = LoginTemplate {
                error: Some("Invalid username or password".to_string()),
            };
            HttpResponse::Ok()
                .content_type("text/html")
                .body(tmpl.render().unwrap())
        }
    }
}

pub async fn register_page() -> impl actix_web::Responder {
    RegisterTemplate { error: None }
}

pub async fn register_submit(
    pool: web::Data<DbPool>,
    form: web::Form<RegisterForm>,
) -> HttpResponse {
    let conn = pool.get().unwrap();
    let hash = match ticketsystem_auth::hash_password(&form.password) {
        Ok(h) => h,
        Err(_) => {
            return HttpResponse::InternalServerError().body("Failed to hash password");
        }
    };

    match user::create(&conn, &form.username, &form.email, &hash, false, false) {
        Ok(_) => HttpResponse::SeeOther()
            .insert_header(("Location", "/login?registered=1"))
            .finish(),
        Err(e) => {
            let tmpl = RegisterTemplate {
                error: Some(format!("Registration failed: {e}")),
            };
            HttpResponse::Ok()
                .content_type("text/html")
                .body(tmpl.render().unwrap())
        }
    }
}

pub async fn logout(req: HttpRequest, pool: web::Data<DbPool>) -> HttpResponse {
    if let Some(cookie) = req.cookie("session_id") {
        let conn = pool.get().unwrap();
        let _ = user::delete_session(&conn, cookie.value());
    }

    HttpResponse::SeeOther()
        .insert_header(("Location", "/login"))
        .cookie(
            actix_web::cookie::Cookie::build("session_id", "")
                .path("/")
                .max_age(actix_web::cookie::time::Duration::ZERO)
                .finish(),
        )
        .finish()
}
