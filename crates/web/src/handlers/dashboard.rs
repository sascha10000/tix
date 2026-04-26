use actix_web::web;
use askama::Template;

use ticketsystem_db::DbPool;
use ticketsystem_db::repo::{project, ticket};
use crate::middleware::AuthenticatedUser;

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    user: AuthenticatedUser,
    projects: Vec<ProjectWithCount>,
}

struct ProjectWithCount {
    id: i64,
    name: String,
    description: String,
    ticket_count: usize,
}

pub async fn index(
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
) -> impl actix_web::Responder {
    let conn = pool.get().unwrap();

    let projects_raw = if user.is_admin() {
        project::list_all(&conn)
    } else {
        project::list_for_user(&conn, user.id)
    };

    let projects: Vec<ProjectWithCount> = projects_raw
        .into_iter()
        .map(|p| {
            let tickets = ticket::list_for_project(&conn, p.id);
            ProjectWithCount {
                id: p.id,
                name: p.name,
                description: p.description,
                ticket_count: tickets.len(),
            }
        })
        .collect();

    DashboardTemplate { user, projects }
}
