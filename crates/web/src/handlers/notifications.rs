use actix_web::{web, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use ticketsystem_db::DbPool;
use ticketsystem_db::repo::notification;
use crate::errors::AppError;
use crate::middleware::AuthenticatedUser;

const MAX_ITEMS: i64 = 20;

#[derive(Deserialize)]
pub struct PollQuery {
    after: Option<i64>,
}

/// Polled by the browser for desktop notifications. Without `after` it only returns
/// the newest id, so enabling notifications doesn't replay the whole history.
pub async fn poll(
    auth_user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    query: web::Query<PollQuery>,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let latest_id = notification::latest_id(&conn, auth_user.id);
    let items: Vec<_> = match query.after {
        Some(after) => notification::list_after(&conn, auth_user.id, after, MAX_ITEMS)
            .into_iter()
            .map(|n| {
                json!({
                    "id": n.id,
                    "ticket_id": n.ticket_id,
                    "project_id": n.project_id,
                    "project": n.project_name,
                    "title": n.ticket_title,
                    "kind": n.kind,
                    "actor": n.actor_name,
                    "created_at": n.created_at,
                })
            })
            .collect(),
        None => Vec::new(),
    };
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(json!({ "latest_id": latest_id, "items": items })))
}
