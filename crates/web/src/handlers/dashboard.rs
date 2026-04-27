use actix_web::web;
use askama::Template;

use ticketsystem_core::i18n::Translations;
use ticketsystem_db::DbPool;
use ticketsystem_db::repo::{project, ticket};
use crate::middleware::{AuthenticatedUser, Lang};

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    user: AuthenticatedUser,
    projects: Vec<ProjectWithCount>,
    assigned_tickets: Vec<UserTicket>,
    created_tickets: Vec<UserTicket>,
    t: &'static Translations,
}

struct ProjectWithCount {
    id: i64,
    name: String,
    description: String,
    ticket_count: usize,
}

#[derive(Clone)]
struct UserTicket {
    id: i64,
    project_id: i64,
    project_name: String,
    title: String,
    status_name: String,
    status_color: String,
    due_date: String,
    updated_at: String,
}

pub async fn index(
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    Lang(t): Lang,
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

    let user_tickets = ticket::list_for_user(&conn, user.id);

    let mut assigned_tickets = Vec::new();
    let mut created_tickets = Vec::new();

    for (row, project_name) in user_tickets {
        let ut = UserTicket {
            id: row.id,
            project_id: row.project_id,
            project_name,
            title: row.title,
            status_name: row.status_name,
            status_color: row.status_color,
            due_date: row.due_date,
            updated_at: row.updated_at,
        };
        if row.assignee_id == user.id {
            assigned_tickets.push(ut.clone());
        }
        if row.creator_id == user.id {
            created_tickets.push(ut);
        }
    }

    DashboardTemplate { user, projects, assigned_tickets, created_tickets, t }
}
