use actix_web::cookie::Cookie;
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest, HttpResponse, web};
use std::future::{Ready, ready};

use crate::db::DbPool;

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub is_manager: bool,
}

impl AuthenticatedUser {
    pub fn is_admin(&self) -> bool {
        self.is_admin
    }

    pub fn is_manager(&self) -> bool {
        self.is_manager
    }
}

impl FromRequest for AuthenticatedUser {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let result = extract_user(req);
        match result {
            Some(user) => ready(Ok(user)),
            None => {
                let response = HttpResponse::SeeOther()
                    .insert_header(("Location", "/login"))
                    .finish();
                ready(Err(actix_web::error::InternalError::from_response(
                    "Not authenticated",
                    response,
                )
                .into()))
            }
        }
    }
}

fn extract_user(req: &HttpRequest) -> Option<AuthenticatedUser> {
    let pool = req.app_data::<web::Data<DbPool>>()?;
    let cookie: Cookie = req.cookie("session_id")?;
    let session_id = cookie.value();

    let conn = pool.get().ok()?;
    conn.query_row(
        "SELECT u.id, u.username, u.is_admin, u.is_manager FROM sessions s
         JOIN users u ON u.id = s.user_id
         WHERE s.id = ?1 AND s.expires_at > datetime('now') AND u.is_active = 1",
        [session_id],
        |row| {
            Ok(AuthenticatedUser {
                id: row.get(0)?,
                username: row.get(1)?,
                is_admin: row.get::<_, i64>(2)? != 0,
                is_manager: row.get::<_, i64>(3)? != 0,
            })
        },
    )
    .ok()
}
