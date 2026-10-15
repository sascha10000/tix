use actix_web::web;
use askama::Template;

use ticketsystem_core::i18n::Translations;
use ticketsystem_db::DbPool;
use ticketsystem_db::repo::{project, status, ticket};
use crate::middleware::{AuthenticatedUser, Lang};
use crate::ticket_filter::{self, FilterQuery, SelectOption};

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    user: AuthenticatedUser,
    projects: Vec<ProjectWithCount>,
    assigned_tickets: Vec<UserTicket>,
    created_tickets: Vec<UserTicket>,
    filter: DashboardFilterView,
    t: &'static Translations,
}

/// Current filter state, for re-rendering the filter form.
struct DashboardFilterView {
    projects: Vec<SelectOption>,
    statuses: Vec<SelectOption>,
    search: String,
    only_open: bool,
    only_overdue: bool,
    active: bool,
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
    assignee_name: String,
    due_date: String,
    updated_at: String,
    overdue: bool,
}

pub async fn index(
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    query: web::Query<FilterQuery>,
    Lang(t): Lang,
) -> impl actix_web::Responder {
    let conn = pool.get().unwrap();
    let filter = query.to_filter();
    let today = ticket_filter::today();

    let projects_raw = if user.is_admin() {
        project::list_all(&conn)
    } else {
        project::list_for_user(&conn, user.id)
    };

    let filter_view = DashboardFilterView {
        projects: SelectOption::list(projects_raw.iter().map(|p| (p.id, p.name.clone())), filter.project_id),
        statuses: SelectOption::list(
            status::list_all(&conn).into_iter().map(|s| (s.id, s.name)),
            filter.status_id,
        ),
        search: query.search_text(),
        only_open: filter.only_open,
        only_overdue: filter.only_overdue,
        active: filter.project_id.is_some()
            || filter.status_id.is_some()
            || filter.query.is_some()
            || filter.only_open
            || filter.only_overdue,
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
        if !filter.matches(&row, today) {
            continue;
        }
        let ut = UserTicket {
            overdue: ticket_filter::is_overdue(&row, today),
            id: row.id,
            project_id: row.project_id,
            project_name,
            title: row.title,
            status_name: row.status_name,
            status_color: row.status_color,
            assignee_name: row.assignee_name,
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

    DashboardTemplate { user, projects, assigned_tickets, created_tickets, filter: filter_view, t }
}
